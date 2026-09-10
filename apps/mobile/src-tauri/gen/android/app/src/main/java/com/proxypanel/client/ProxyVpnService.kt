package com.proxypanel.client

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.PackageManager.NameNotFoundException
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.IpPrefix
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.system.OsConstants
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.proxypanel.core.libbox.BridgeOptions
import com.proxypanel.core.libbox.BridgeSession
import com.proxypanel.core.libbox.CommandClient
import com.proxypanel.core.libbox.CommandClientHandler
import com.proxypanel.core.libbox.CommandClientOptions
import com.proxypanel.core.libbox.CommandServer
import com.proxypanel.core.libbox.CommandServerHandler
import com.proxypanel.core.libbox.ConnectionEvents
import com.proxypanel.core.libbox.ConnectionOwner
import com.proxypanel.core.libbox.InterfaceUpdateListener
import com.proxypanel.core.libbox.Libbox
import com.proxypanel.core.libbox.LocalDNSTransport
import com.proxypanel.core.libbox.LogIterator
import com.proxypanel.core.libbox.NetworkInterface as LibboxNetworkInterface
import com.proxypanel.core.libbox.NetworkInterfaceIterator
import com.proxypanel.core.libbox.NeighborUpdateListener
import com.proxypanel.core.libbox.OutboundGroupItemIterator
import com.proxypanel.core.libbox.OutboundGroupIterator
import com.proxypanel.core.libbox.OverrideOptions
import com.proxypanel.core.libbox.PlatformInterface
import com.proxypanel.core.libbox.PlatformUser
import com.proxypanel.core.libbox.RoutePrefix
import com.proxypanel.core.libbox.SetupOptions
import com.proxypanel.core.libbox.ShellSession
import com.proxypanel.core.libbox.StatusMessage
import com.proxypanel.core.libbox.StringIterator
import com.proxypanel.core.libbox.SystemProxyStatus
import com.proxypanel.core.libbox.TunOptions
import com.proxypanel.core.libbox.WIFIState
import com.proxypanel.core.libbox.Notification as LibboxNotification
import java.io.File
import java.net.Inet6Address
import java.net.InetAddress
import java.net.InterfaceAddress
import java.net.NetworkInterface
import java.time.OffsetDateTime
import java.time.format.DateTimeFormatter

/**
 * TUN/VPN 前台服务：通过 libbox（sing-box）驱动，由 [VpnPlugin] 的
 * start/stop 命令控制启停。
 *
 * 启动序列（sing-box 1.14 libbox / SFA 官方语义）：
 *   1. Libbox.setup(SetupOptions) 设置数据路径（进程内仅一次）
 *   2. CommandServer(this, platformInterface) 创建核心服务
 *   3. CommandServer.start() + startOrReloadService(config, OverrideOptions)
 *      启动核心（openTun 由 [PlatformInterface] 回调，用
 *      VpnService.Builder.establish() 取得 pfd，保留所有权并返回 `pfd.fd`
 *      原始 fd 号给核心；核心侧 `dup(fd)` 出一份独立 fd 供 tun 使用）
 *   4. 另起 [CommandClient]（CommandLog 流）承接核心日志，重建「环形缓冲 +
 *      logs/libbox.log」行为（1.14 已移除 PlatformInterface.writeLog）
 *
 * 停止序列：先 close pfd（原始 fd），再 CommandServer.closeService()（关闭核心
 * 持有的 dup fd / tun 接口），最后 CommandServer.close()（停止 gRPC listener），
 * 随后 VpnService 撤销 VPN。
 *
 * fd 生命周期对齐 SFA：**不调用 detachFd()**。detachFd 会把原始 fd 所有权转交
 * 出去且 Kotlin 侧 `close()` 变成空操作，导致原始 fd 无人关闭（fd 泄漏）并绕过
 * VpnService 的 fd 生命周期跟踪，是真机「file already closed」的诱因。
 *
 * 重复启动防护：`startInProgress` 标志保证同一时刻只有一个 startBox 序列；
 * 重启（服务存活时再次收到 start）在 startBox 内先有序关闭旧实例再启动新实例，
 * 禁止两个 CommandServer 并发 establish()（并发 establish 会撤销前一 tun 导致
 * 启动期即报 file already closed）。
 */
class ProxyVpnService : VpnService(), CommandServerHandler {

  companion object {
    private const val TAG = "ProxyVpnService"
    private const val CHANNEL_ID = "vpn"
    private const val NOTIFICATION_ID = 1

    /** Intent extra: sing-box JSON config content. */
    const val EXTRA_CONFIG = "config"
    /** Intent extra: whether to show traffic in notification. */
    const val EXTRA_SHOW_TRAFFIC = "show_traffic"
    /** Intent extra: whether to show proxy selection in notification. */
    const val EXTRA_SHOW_SELECTION = "show_selection"

    /**
     * Explicit stop action: sent by [VpnPlugin]. onStartCommand receives it and orderly shuts down
     * BoxService (background thread) -> exit foreground -> self-stop -> reset running.
     */
    const val ACTION_STOP = "com.proxypanel.client.action.STOP"
    /** Update notification preferences action: sent by [VpnPlugin] when user toggles settings. */
    const val ACTION_UPDATE_PREFS = "com.proxypanel.client.action.UPDATE_PREFS"

    /**
     * Service running flag, maintained by the service's own real lifecycle (set true after startBox succeeds,
     * set false on failure / onDestroy / onRevoke). VpnPlugin does not optimistically set it.
     */
    @Volatile
    var running = false
      private set

    /**
     * Service instance alive flag (set true in onCreate, false in onDestroy): a starting service
     * still has `running` false but instance is already in foreground; VpnPlugin uses this to judge
     * whether ACTION_STOP can be safely dispatched (avoid missing stop during startup).
     */
    @Volatile
    var instanceAlive = false
      private set

    /** Most recent startup failure reason (cleared after successful start; read by plugin isRunning and frontend polling). */
    @Volatile
    var lastError: String? = null
      private set

    /**
     * libbox 日志环形缓冲行数，同时作为核心侧 `SetupOptions.logMaxLines`
     * （CommandClient 连接时按该上限重放已保存日志）。
     */
    private const val LIBBOX_LOG_BUFFER_SIZE = 200

    /**
     * libbox 日志环形缓冲（最近 [LIBBOX_LOG_BUFFER_SIZE] 行）：启动失败时附带进
     * lastError，让前端 Alert 直接看到 Go 侧完整错误链（如 "query tun name" /
     * "dup tun file descriptor" / "initialize inbound/tun" 前缀），定位异步失败。
     *
     * 每行以 `[RFC3339] ` 时间戳前缀开头（[logClientHandler] 统一写入，与
     * `logs/libbox.log` 同源），文件超限截断时重建内容仍为可解析的完整行。
     */
    private val libboxLogBuffer = ArrayDeque<String>(LIBBOX_LOG_BUFFER_SIZE)

    /**
     * 日志文件待重置标记：CommandClient 首次连接/重连时核心会以 Reset 重放
     * 完整日志（[clearLibboxLogBuffer]），下一批 writeLogs 需先清空文件再写入，
     * 避免重放内容与既有文件重复。
     */
    private var libboxLogFileResetPending = false

    /** `logs/libbox.log` 大小上限（字节），超限时删除重建写入最新缓冲。 */
    private const val LIBBOX_LOG_MAX_BYTES = 1024 * 1024

    /**
     * RFC3339 本地时间格式（毫秒精度，如 `2026-08-02T22:12:30.123+08:00`）。
     * 与 Rust 日志页 `LogEntry.ts` 对齐，供日志查看器 `get_logs` 解析排序。
     * `java.time` 自 API 26 起可用（minSdk 33，无需 desugaring）。
     */
    private val LOG_TS_FORMATTER: DateTimeFormatter =
      DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss.SSSXXX")

    /**
     * Libbox.setup 每进程只调一次。重复 setup 会重置 Go 侧全局状态（日志、数据
     * 路径、崩溃处理等），污染正在运行的核心，因此用 setupDone + 锁做一次保护。
     */
    private val setupLock = Any()

    @Volatile
    private var setupDone = false

    /**
     * 生命周期锁：串行化 startBox/stopBox 的 boxService/tunFd 访问，跨实例共享
     * （重启时旧实例的迟到 stop 与新实例的 start 并发，锁保证互斥）。SFA 用
     * CommandServer + status 状态机达成同样效果，这里用锁 + [startInProgress]
     * 标志对齐「同一时刻只允许一个启动序列」的语义。
     */
    private val lifecycleLock = Any()

    /** 启动序列进行中：防重复 start 并发启动两个 BoxService（SFA 的 status != Stopped 守卫）。 */
    @Volatile
    private var startInProgress = false

    /**
     * 当前注册的 CommandServer 及其 tun pfd，进程级共享（跨实例）：
     * 重启时新实例的 startBox 需先关闭旧实例残留的 server（「先有序 stop 再 start」），
     * 而旧实例迟到的 stopBox 不得关闭新实例已启动的 server（用 [boxOwner] 判定归属）。
     * 所有访问必须在 [lifecycleLock] 临界区内进行。
     *
     * sing-box 1.14 起 `Libbox.newService` + `BoxService` 被移除，改由
     * `CommandServer`（`start()` + `startOrReloadService()` / `closeService()` / `close()`）
     * 承载核心生命周期；`CommandServerHandler` 由本服务实现。
     */
    @Volatile
    private var commandServer: CommandServer? = null

    /**
     * 日志通道客户端：sing-box 1.14 移除了 `PlatformInterface.writeLog`，日志经
     * `CommandServer` 的 gRPC 日志流（CommandLog）对外提供。本服务用
     * [CommandClient] 连接自身 server，在 [logClientHandler.writeLogs] 中重建
     * 「环形缓冲 + logs/libbox.log 落盘」的旧行为。
     */
    @Volatile
    private var commandClient: CommandClient? = null

    private var tunFd: ParcelFileDescriptor? = null

    /** 当前 box/tun 的持有者实例：仅持有者可关闭，防旧实例迟到的 stop 误关新 box。 */
    private var boxOwner: ProxyVpnService? = null
  }

  /** Stop request flag: when stop arrives during startBox background thread startup, orderly close after startup completes. */
  @Volatile
  private var stopRequested = false

  /** 最近一次应用的 sing-box JSON 配置，供 CommandServerHandler.serviceReload() 重放。 */
  @Volatile
  private var lastConfig: String? = null

  /** Notification preference flags (read from intent extras or VpnPlugin companion). */
  @Volatile
  private var showTraffic = true

  @Volatile
  private var showSelection = true

  /** Clash API polling state. */
  private var clashApiPort: Int = 9090
  private var clashApiSecret: String = ""
  private var notificationUpdateHandler: Handler? = null
  private var notificationUpdateRunnable: Runnable? = null

  private val mainHandler = Handler(Looper.getMainLooper())

  /**
   * libbox 回调实现。openTun / 默认接口监控 / 接口枚举已为真实实现；
   * 连接归属 / 证书等功能保持最小实现，后续按需补齐。
   */
  private val platformInterface = object : PlatformInterface {

    override fun usePlatformAutoDetectInterfaceControl(): Boolean = false

    override fun autoDetectInterfaceControl(fd: Int) {
      // 关闭平台自动检测（见 usePlatformAutoDetectInterfaceControl），此回调不会被调用。
      protect(fd)
    }

    override fun openTun(options: TunOptions): Int {
      if (VpnService.prepare(this@ProxyVpnService) != null) {
        throw IllegalStateException("android: missing vpn permission")
      }

      val builder =
        Builder()
          .setSession(applicationInfo.loadLabel(packageManager).toString())
          .setMtu(options.mtu)

      builder.setMetered(false)

      // 本应用必须排除在 VPN 之外：核心出站（代理连接 / DoH）由本进程发起，若被
      // tun 回环会成环导致「有 VPN 图标但流量不通」。SFA 惯例始终排除自身。
      try {
        builder.addDisallowedApplication(packageName)
      } catch (e: NameNotFoundException) {
        Log.w(TAG, "addDisallowedApplication(self) failed: ${e.message}")
      }

      val inet4Address = options.inet4Address
      while (inet4Address.hasNext()) {
        val address = inet4Address.next()
        builder.addAddress(address.address(), address.prefix())
      }

      val inet6Address = options.inet6Address
      while (inet6Address.hasNext()) {
        val address = inet6Address.next()
        builder.addAddress(address.address(), address.prefix())
      }

      if (options.autoRoute) {
        // DNS 劫持地址由核心按 inet4 首地址推导（需保留一个可用 IP）。
        // sing-box 1.14 起 getDNSServerAddress() 返回 StringIterator（可含多个地址），
        // 逐个注入 Builder（旧版为单值 StringBox.value）。
        try {
          val dnsServers = options.dnsServerAddress
          while (dnsServers.hasNext()) {
            val dnsServer = dnsServers.next()
            if (!dnsServer.isNullOrEmpty()) {
              builder.addDnsServer(dnsServer)
            }
          }
        } catch (e: Exception) {
          Log.w(TAG, "no dns server address available: ${e.message}")
        }

        // minSdk 33：支持精确路由 + excludeRoute。
        val inet4RouteAddress = options.inet4RouteAddress
        if (inet4RouteAddress.hasNext()) {
          while (inet4RouteAddress.hasNext()) {
            val route = inet4RouteAddress.next()
            builder.addRoute(route.address(), route.prefix())
          }
        } else {
          builder.addRoute("0.0.0.0", 0)
        }

        val inet6RouteAddress = options.inet6RouteAddress
        if (inet6RouteAddress.hasNext()) {
          while (inet6RouteAddress.hasNext()) {
            val route = inet6RouteAddress.next()
            builder.addRoute(route.address(), route.prefix())
          }
        } else {
          builder.addRoute("::", 0)
        }

        val inet4RouteExclude = options.inet4RouteExcludeAddress
        while (inet4RouteExclude.hasNext()) {
          addExcludeRoute(builder, inet4RouteExclude.next())
        }

        val inet6RouteExclude = options.inet6RouteExcludeAddress
        while (inet6RouteExclude.hasNext()) {
          addExcludeRoute(builder, inet6RouteExclude.next())
        }

        // 分应用代理（可选）。
        val includePackage = options.includePackage
        while (includePackage.hasNext()) {
          try {
            builder.addAllowedApplication(includePackage.next())
          } catch (e: NameNotFoundException) {
            Log.w(TAG, "addAllowedApplication failed: ${e.message}")
          }
        }

        val excludePackage = options.excludePackage
        while (excludePackage.hasNext()) {
          try {
            builder.addDisallowedApplication(excludePackage.next())
          } catch (e: NameNotFoundException) {
            Log.w(TAG, "addDisallowedApplication failed: ${e.message}")
          }
        }
      }

      val pfd =
        builder.establish()
          ?: throw IllegalStateException(
            "android: 无法建立 VPN 隧道（授权可能已被系统撤销）"
          )
      // SFA 语义（VPNService.kt:184-188）：保留 pfd 所有权，仅返回原始 fd 号，
      // 不调用 detachFd()。核心侧 service.go OpenTun 会对该 fd 做 dup() 取一份
      // 独立 fd 给 tun 使用；Kotlin 侧在 stopBox 关闭 pfd（原始 fd）。若 detachFd，
      // 原始 fd 所有权被转交且 pfd.close() 变空操作，原始 fd 无人关闭（fd 泄漏）
      // 并绕过 VpnService 生命周期跟踪，是真机「file already closed」的诱因。
      tunFd = pfd
      return pfd.fd
    }

    // minSdk 33 后恒 false（/proc 回退仅 API < 29 需要）。
    override fun useProcFS(): Boolean = false

    /**
     * 连接归属查询：panelcore 保持最小实现（不解析 uid/进程），返回
     * `userId = -1` 的 [ConnectionOwner] 表示「未找到」，与旧版返回 -1 的
     * 语义一致（分应用/进程规则不生效，行为不变）。
     *
     * sing-box 1.14 起返回类型由 `Int` 改为 `ConnectionOwner`。
     */
    override fun findConnectionOwner(
      ipProtocol: Int,
      sourceAddress: String,
      sourcePort: Int,
      destinationAddress: String,
      destinationPort: Int,
    ): ConnectionOwner {
      val owner = ConnectionOwner()
      owner.userId = -1
      owner.userName = ""
      owner.processPath = ""
      return owner
    }

    override fun startDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
      // 默认网络接口监控：把物理接口 name/index 推给核心，供
      // route.auto_detect_interface 学习默认出站接口（修复出站拨号
      // no available network interface）。注册/推送逻辑见 [DefaultInterfaceMonitor]。
      DefaultInterfaceMonitor.start(listener, getSystemService(ConnectivityManager::class.java))
    }

    override fun closeDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
      DefaultInterfaceMonitor.close(listener)
    }

    override fun getInterfaces(): NetworkInterfaceIterator {
      // 枚举全部已接入网络，组装 libbox NetworkInterface 列表（供核心学习
      // 全部物理接口及 type/flags/metered 等属性）；单个网络失败跳过不中断。
      val connectivity = getSystemService(ConnectivityManager::class.java)
      val networks = connectivity.allNetworks
      val networkInterfaces =
        runCatching { NetworkInterface.getNetworkInterfaces().toList() }.getOrDefault(emptyList())
      val interfaces = mutableListOf<LibboxNetworkInterface>()
      for (network in networks) {
        val boxInterface = LibboxNetworkInterface()
        val linkProperties = connectivity.getLinkProperties(network) ?: continue
        val networkCapabilities = connectivity.getNetworkCapabilities(network) ?: continue
        boxInterface.name = linkProperties.interfaceName
        val networkInterface = networkInterfaces.find { it.name == boxInterface.name } ?: continue
        boxInterface.dnsServer = StringArray(
          linkProperties.dnsServers.mapNotNull { it.hostAddress }.iterator()
        )
        boxInterface.type =
          when {
            networkCapabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ->
              Libbox.InterfaceTypeWIFI
            networkCapabilities.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) ->
              Libbox.InterfaceTypeCellular
            networkCapabilities.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) ->
              Libbox.InterfaceTypeEthernet
            else -> Libbox.InterfaceTypeOther
          }
        boxInterface.index = networkInterface.index
        runCatching { boxInterface.mtu = networkInterface.mtu }
        boxInterface.addresses = StringArray(
          networkInterface.interfaceAddresses.map { it.toPrefix() }.iterator()
        )
        var dumpFlags = 0
        if (networkCapabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)) {
          dumpFlags = OsConstants.IFF_UP or OsConstants.IFF_RUNNING
        }
        if (networkInterface.isLoopback) {
          dumpFlags = dumpFlags or OsConstants.IFF_LOOPBACK
        }
        if (networkInterface.isPointToPoint) {
          dumpFlags = dumpFlags or OsConstants.IFF_POINTOPOINT
        }
        if (networkInterface.supportsMulticast()) {
          dumpFlags = dumpFlags or OsConstants.IFF_MULTICAST
        }
        boxInterface.flags = dumpFlags
        boxInterface.metered =
          !networkCapabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)
        interfaces.add(boxInterface)
      }
      return InterfaceArray(interfaces.iterator())
    }

    override fun underNetworkExtension(): Boolean = false

    override fun includeAllNetworks(): Boolean = false

    override fun readWIFIState(): WIFIState = WIFIState("", "")

    override fun clearDNSCache() {
      // 无本地 DNS 缓存（最小实现）。
    }

    override fun sendNotification(notification: LibboxNotification) {
      // 通知透传留待后续实现。
    }

    override fun cancelNotification(identifier: String, typeID: Int) {
      // panelcore 未使用核心通知通道（通知由本服务的 NotificationCompat 管理）。
    }

    // --- 以下为 sing-box 1.14 PlatformInterface 新增能力，panelcore 均未启用，
    //     保持最小 stub（返回「不支持/空值」），避免核心调用到未实现逻辑。 ---

    override fun startNeighborMonitor(listener: NeighborUpdateListener?) {
      // 未启用邻居表解析（路由规则不使用 neighbor 来源）。
    }

    override fun closeNeighborMonitor(listener: NeighborUpdateListener?) {
      // 未启用邻居表解析。
    }

    override fun registerMyInterface(name: String?) {
      // 未启用核心侧 my-interface 跟踪；默认接口由 [DefaultInterfaceMonitor] 推送。
    }

    override fun usePlatformShell(): Boolean = false

    override fun checkPlatformShell() {
      throw UnsupportedOperationException("android: platform shell not supported")
    }

    override fun openShellSession(
      user: PlatformUser?,
      command: String?,
      environ: StringIterator?,
      term: String?,
      rows: Int,
      cols: Int,
    ): ShellSession {
      throw UnsupportedOperationException("android: platform shell not supported")
    }

    override fun lookupUser(username: String?): PlatformUser {
      throw UnsupportedOperationException("android: platform shell not supported")
    }

    override fun lookupSFTPServer(): String {
      throw UnsupportedOperationException("android: sftp not supported")
    }

    override fun readSystemSSHHostKey(): String {
      throw UnsupportedOperationException("android: system ssh host key not supported")
    }

    override fun tailscaleHostname(): String = ""

    override fun usePlatformBridge(): Boolean = false

    override fun createBridge(options: BridgeOptions?): BridgeSession {
      throw UnsupportedOperationException("android: bridge not supported")
    }

    override fun localDNSTransport(): LocalDNSTransport? = null
  }

  override fun onCreate() {
    super.onCreate()
    instanceAlive = true
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent?.action == ACTION_STOP) {
      handleStop()
      return Service.START_NOT_STICKY
    }

    if (intent?.action == ACTION_UPDATE_PREFS) {
      showTraffic = intent.getBooleanExtra(EXTRA_SHOW_TRAFFIC, showTraffic)
      showSelection = intent.getBooleanExtra(EXTRA_SHOW_SELECTION, showSelection)
      if (running) {
        updateNotificationContent()
      }
      return Service.START_NOT_STICKY
    }

    val config = intent?.getStringExtra(EXTRA_CONFIG)
    if (config.isNullOrBlank()) {
      Log.w(TAG, "start command without config, stopping")
      lastError = "Missing VPN config"
      stopSelf()
      return Service.START_NOT_STICKY
    }

    // Read notification prefs from intent extras (fallback to VpnPlugin companion values).
    showTraffic = intent.getBooleanExtra(EXTRA_SHOW_TRAFFIC, VpnPlugin.notifyShowTraffic)
    showSelection = intent.getBooleanExtra(EXTRA_SHOW_SELECTION, VpnPlugin.notifyShowSelection)

    try {
      startForegroundWithNotification()
    } catch (e: Exception) {
      Log.e(TAG, "failed to start foreground service", e)
      running = false
      lastError = "Foreground service start failed: ${e.message ?: e.javaClass.simpleName}"
      stopSelf()
      return Service.START_NOT_STICKY
    }

    // Dedup start guard: only one startBox sequence at a time (aligns with SFA onStartCommand
    // status != Stopped guard). Ignore this start if another sequence is in progress to avoid
    // concurrent establish() which would revoke previous tun and cause "file already closed".
    if (startInProgress) {
      Log.w(TAG, "start ignored: another start sequence is in progress")
      return Service.START_NOT_STICKY
    }

    // New start sequence begins, clear previous stop request flag.
    stopRequested = false

    // Parse Clash API config from the JSON config for notification polling.
    parseClashApiConfig(config)

    // newService/start are blocking calls, run in background thread (aligns with SFA:
    // onStartCommand returns immediately, start runs asynchronously in IO background thread).
    startInProgress = true
    Thread {
      try {
        startBox(config)
        if (stopRequested) {
          // Stop request arrived during startup: orderly close after startup completes, do not set running.
          Log.i(TAG, "stop requested during startup, closing service")
          stopBox()
        } else {
          running = true
          lastError = null
          Log.i(TAG, "libbox service started")
          // Start notification content polling if needed.
          startNotificationPolling()
        }
      } catch (e: Exception) {
        Log.e(TAG, "failed to start libbox service", e)
        running = false
        // Attach full exception chain + recent libbox logs for frontend Alert display.
        lastError = withLibboxLogTail(buildExceptionChain(e))
        stopSelf()
      } finally {
        startInProgress = false
      }
    }.start()

    return Service.START_NOT_STICKY
  }

  /**
   * Explicit stop: orderly close BoxService in background thread (`close()` blocks, avoid ANR on main thread),
   * then exit foreground and self-stop on main thread; `running` is immediately reset for status polling.
   */
  private fun handleStop() {
    stopRequested = true
    running = false
    stopNotificationPolling()
    Log.i(TAG, "stop requested")
    Thread {
      try {
        stopBox()
      } catch (e: Exception) {
        Log.e(TAG, "failed to stop libbox service", e)
        lastError = buildExceptionChain(e)
      } finally {
        mainHandler.post {
          stopForegroundCompat()
          stopSelf()
        }
      }
    }.start()
  }

  override fun onDestroy() {
    instanceAlive = false
    // 有序关闭放后台线程（close 阻塞，避免主线程 ANR）；stopBox 幂等。
    closeBoxInBackground()
    super.onDestroy()
  }

  override fun onRevoke() {
    // VPN 授权被撤销：停止核心并自杀。
    stopRequested = true
    closeBoxInBackground()
    stopSelf()
    super.onRevoke()
  }

  private fun startForegroundWithNotification() {
    val channel =
      NotificationChannel(CHANNEL_ID, "ProxyPanel VPN", NotificationManager.IMPORTANCE_LOW)
    getSystemService(NotificationManager::class.java).createNotificationChannel(channel)

    val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
    val contentIntent =
      launchIntent?.let {
        PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE)
      }

    val notification =
      NotificationCompat.Builder(this, CHANNEL_ID)
        .setContentTitle("ProxyPanel")
        .setContentText("VPN service is running")
        .setSmallIcon(R.mipmap.ic_launcher)
        .setContentIntent(contentIntent)
        .setOngoing(true)
        .build()

    val type =
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
      } else {
        0
      }
    ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, type)
  }

  /** Parse Clash API port/secret from sing-box JSON config for notification polling. */
  private fun parseClashApiConfig(config: String) {
    try {
      val json = org.json.JSONObject(config)
      // sing-box experimental.clash_api
      val experimental = json.optJSONObject("experimental")
      if (experimental != null) {
        val clashApi = experimental.optJSONObject("clash_api")
        if (clashApi != null) {
          clashApiPort = clashApi.optInt("external_controller_port", 9090)
          clashApiSecret = clashApi.optString("secret", "")
          return
        }
      }
      // Fallback: direct clash_api object (some configs)
      val directClash = json.optJSONObject("clash_api")
      if (directClash != null) {
        clashApiPort = directClash.optInt("external_controller_port", 9090)
        clashApiSecret = directClash.optString("secret", "")
      }
    } catch (e: Exception) {
      Log.w(TAG, "failed to parse clash api config: ${e.message}")
    }
  }

  /** Start periodic notification content update (every 4s) if traffic or selection display is enabled. */
  private fun startNotificationPolling() {
    stopNotificationPolling()
    if (!showTraffic && !showSelection) {
      return
    }
    notificationUpdateHandler = Handler(Looper.getMainLooper())
    val runnable = object : Runnable {
      override fun run() {
        if (!running) return
        updateNotificationContent()
        notificationUpdateHandler?.postDelayed(this, 4000)
      }
    }
    notificationUpdateRunnable = runnable
    notificationUpdateHandler?.postDelayed(runnable, 4000)
  }

  private fun stopNotificationPolling() {
    notificationUpdateRunnable?.let { notificationUpdateHandler?.removeCallbacks(it) }
    notificationUpdateRunnable = null
    notificationUpdateHandler = null
  }

  /** Update notification content from Clash API (traffic + selection). */
  private fun updateNotificationContent() {
    try {
      val lines = mutableListOf<String>()
      if (showSelection) {
        val selection = fetchClashSelection()
        if (selection.isNotEmpty()) {
          lines.add(selection)
        }
      }
      if (showTraffic) {
        val traffic = fetchClashTraffic()
        if (traffic.isNotEmpty()) {
          lines.add(traffic)
        }
      }
      val contentText = if (lines.isEmpty()) "VPN service is running" else lines.joinToString(" · ")
      updateNotification(contentText)
    } catch (e: Exception) {
      Log.w(TAG, "notification update failed: ${e.message}")
    }
  }

  /** Fetch current proxy group selection from Clash API GET /proxies. */
  private fun fetchClashSelection(): String {
    return try {
      val url = java.net.URL("http://127.0.0.1:$clashApiPort/proxies")
      val conn = url.openConnection() as java.net.HttpURLConnection
      conn.connectTimeout = 2000
      conn.readTimeout = 2000
      if (clashApiSecret.isNotEmpty()) {
        conn.setRequestProperty("Authorization", "Bearer $clashApiSecret")
      }
      val text = conn.inputStream.bufferedReader().use { it.readText() }
      conn.disconnect()

      val json = org.json.JSONObject(text)
      val proxies = json.optJSONObject("proxies") ?: return ""
      // Find the first selector group and its now node
      val keys = proxies.keys()
      while (keys.hasNext()) {
        val key = keys.next()
        val proxy = proxies.optJSONObject(key) ?: continue
        if (proxy.optString("type") == "Selector") {
          val now = proxy.optString("now", "")
          if (now.isNotEmpty()) {
            return "$key: $now"
          }
        }
      }
      ""
    } catch (e: Exception) {
      ""
    }
  }

  /** Fetch total upload/download traffic from Clash API GET /connections. */
  private fun fetchClashTraffic(): String {
    return try {
      val url = java.net.URL("http://127.0.0.1:$clashApiPort/connections")
      val conn = url.openConnection() as java.net.HttpURLConnection
      conn.connectTimeout = 2000
      conn.readTimeout = 2000
      if (clashApiSecret.isNotEmpty()) {
        conn.setRequestProperty("Authorization", "Bearer $clashApiSecret")
      }
      val text = conn.inputStream.bufferedReader().use { it.readText() }
      conn.disconnect()

      val json = org.json.JSONObject(text)
      val downloadTotal = json.optLong("downloadTotal", 0)
      val uploadTotal = json.optLong("uploadTotal", 0)
      if (downloadTotal == 0L && uploadTotal == 0L) return ""
      "${formatBytes(downloadTotal)} / ${formatBytes(uploadTotal)}"
    } catch (e: Exception) {
      ""
    }
  }

  private fun formatBytes(bytes: Long): String {
    return when {
      bytes >= 1024 * 1024 * 1024 -> String.format(java.util.Locale.US, "%.2f GB", bytes / (1024.0 * 1024.0 * 1024.0))
      bytes >= 1024 * 1024 -> String.format(java.util.Locale.US, "%.2f MB", bytes / (1024.0 * 1024.0))
      bytes >= 1024 -> String.format(java.util.Locale.US, "%.2f KB", bytes / 1024.0)
      else -> "$bytes B"
    }
  }

  private fun updateNotification(contentText: String) {
    val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
    val contentIntent =
      launchIntent?.let {
        PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE)
      }
    val notification =
      NotificationCompat.Builder(this, CHANNEL_ID)
        .setContentTitle("ProxyPanel")
        .setContentText(contentText)
        .setSmallIcon(R.mipmap.ic_launcher)
        .setContentIntent(contentIntent)
        .setOngoing(true)
        .build()
    val nm = getSystemService(NotificationManager::class.java)
    nm.notify(NOTIFICATION_ID, notification)
  }

  private fun startBox(config: String) {
    // 数据路径（对齐 SFA Application.kt:99-110）：basePath 用内部 filesDir，
    // workingPath 用外部 files 目录（外部存储可用时）供核心缓存使用，tempPath
    // 用 cacheDir；外部目录不可用时回退内部 filesDir。fixAndroidStack 开启以
    // 修复 Android 上 Go 栈回溯问题。logMaxLines 供 CommandClient 连接时重放
    // 已保存日志（1.14 日志通道，替代旧 PlatformInterface.writeLog）。
    val workingDir = getExternalFilesDir(null)
    val setup =
      SetupOptions().apply {
        basePath = filesDir.absolutePath
        workingPath = workingDir?.absolutePath ?: filesDir.absolutePath
        tempPath = cacheDir.absolutePath
        fixAndroidStack = true
        logMaxLines = LIBBOX_LOG_BUFFER_SIZE.toLong()
      }
    setupLibbox(setup)

    lastConfig = config

    // 注册、有序重启与启动放同一临界区（跨实例共享 lifecycleLock）：
    //   - 若存在旧 server（重启：服务存活时再次 start / 旧实例残留），先有序关闭
    //     再启动新实例，杜绝两个 CommandServer 并发 establish()；
    //   - stopBox 的锁会等待启动完成再 close，避免停止线程在启动期间
    //     close 半启动的服务；
    //   - start/startOrReloadService 抛异常时立即 close 该实例并复位状态，
    //     清理半启动痕迹。
    synchronized(lifecycleLock) {
      // 先有序 stop 再 start：关闭上一轮实例持有的 server 与 tun fd。
      stopLogClient()
      commandServer?.let { old ->
        commandServer = null
        boxOwner = null
        try {
          old.closeService()
        } catch (e: Exception) {
          Log.e(TAG, "failed to close previous libbox service", e)
        }
        try {
          old.close()
        } catch (e: Exception) {
          Log.e(TAG, "failed to close previous command server", e)
        }
      }
      tunFd?.let { old ->
        try {
          old.close()
        } catch (e: Exception) {
          Log.e(TAG, "failed to close previous tun fd", e)
        }
      }
      tunFd = null

      val server = CommandServer(this, platformInterface)
      try {
        server.start()
        commandServer = server
        // 尽早连接日志通道：捕获 startOrReloadService 期间（含失败）的核心日志，
        // 供启动失败时 lastError 附带 Go 侧错误链。connect 在后台线程执行，不阻塞。
        startLogClient()
        server.startOrReloadService(config, OverrideOptions())
      } catch (e: Exception) {
        // 半启动清理：server 已创建但 start/应用配置抛异常（可能 openTun 已成功、
        // 后续 inbound 失败），立即 close 该实例、回收 tun pfd 并复位，防 fd 泄漏。
        stopLogClient()
        runCatching { server.closeService() }
        runCatching { server.close() }
        runCatching { tunFd?.close() }
        tunFd = null
        if (commandServer === server) {
          commandServer = null
        }
        throw e
      }
      boxOwner = this
    }
  }

  // --- CommandServerHandler：核心侧回调（sing-box 1.14 新增） ---

  /**
   * 核心请求停止服务（如致命错误）：复用与显式 STOP 相同的后台有序关闭路径，
   * 避免在 Go 回调线程内同步阻塞。
   */
  override fun serviceStop() {
    Log.i(TAG, "libbox requested service stop")
    handleStop()
  }

  /**
   * 核心请求重载：用最近一次配置重放 `startOrReloadService`。panelcore 的配置
   * 由 Rust 侧显式 start 驱动，此回调通常不触发；实现为幂等重放并回收旧 tun fd。
   */
  override fun serviceReload() {
    val config = lastConfig ?: return
    Log.i(TAG, "libbox requested service reload")
    Thread {
      try {
        synchronized(lifecycleLock) {
          val server = commandServer ?: return@synchronized
          runCatching { tunFd?.close() }
          tunFd = null
          server.startOrReloadService(config, OverrideOptions())
        }
      } catch (e: Exception) {
        Log.e(TAG, "libbox service reload failed", e)
        lastError = buildExceptionChain(e)
      }
    }.start()
  }

  /** 系统代理由 Rust 侧 sysproxy 管理，核心侧不参与。 */
  override fun getSystemProxyStatus(): SystemProxyStatus {
    val status = SystemProxyStatus()
    status.available = false
    status.enabled = false
    return status
  }

  override fun setSystemProxyEnabled(isEnabled: Boolean) {
    // panelcore 未启用核心侧系统代理（由 Rust 侧 sysproxy 负责）。
  }

  override fun triggerNativeCrash() {
    Thread {
      Thread.sleep(200)
      throw RuntimeException("debug native crash")
    }.start()
  }

  override fun writeDebugMessage(message: String?) {
    if (message != null) {
      Log.d(TAG, message)
    }
  }

  override fun connectSSHAgent(): Int = -1

  /**
   * 连接本进程 CommandServer 的日志流（CommandLog），重建旧 `writeLog` 的
   * 「环形缓冲 + logs/libbox.log」行为。connect() 阻塞（含重试），放后台线程。
   */
  private fun startLogClient() {
    stopLogClient()
    val options =
      CommandClientOptions().apply {
        addCommand(Libbox.CommandLog)
        statusInterval = 1_000_000_000L
      }
    val client =
      try {
        CommandClient(logClientHandler, options)
      } catch (e: Exception) {
        Log.e(TAG, "failed to create log command client", e)
        return
      }
    commandClient = client
    Thread {
      try {
        client.connect()
      } catch (e: Exception) {
        Log.e(TAG, "log command client connect failed", e)
      }
    }.start()
  }

  /** 断开日志通道客户端（幂等；不阻塞调用线程）。 */
  private fun stopLogClient() {
    val client = commandClient
    commandClient = null
    if (client != null) {
      Thread {
        runCatching { client.disconnect() }
      }.start()
    }
  }

  /**
   * 日志通道回调：把核心日志写入环形缓冲与 `logs/libbox.log`。时间戳前缀与
   * Rust 日志页 `LogEntry.ts` 对齐（`[RFC3339] message`），Rust `get_logs`
   * 按行解析后合并展示。
   */
  private val logClientHandler =
    object : CommandClientHandler {
      override fun connected() {
        Log.i(TAG, "libbox log channel connected")
      }

      override fun disconnected(message: String?) {
        Log.i(TAG, "libbox log channel disconnected: $message")
      }

      override fun setDefaultLogLevel(level: Int) {
        // 核心日志级别由配置 log.level 决定，无需平台调整。
      }

      override fun clearLogs() {
        // 首次连接/重连时核心以 Reset 重放完整日志：清空缓冲并标记文件待重置。
        clearLibboxLogBuffer()
      }

      override fun writeLogs(messageList: LogIterator?) {
        if (messageList == null) {
          return
        }
        val lines = ArrayList<String>()
        while (messageList.hasNext()) {
          val entry = messageList.next() ?: continue
          val message = entry.message ?: ""
          Log.i(TAG, message)
          lines.add("[${timestampNow()}] $message")
        }
        if (lines.isNotEmpty()) {
          appendLibboxLogs(lines)
          appendToLibboxLogFile(lines)
        }
      }

      override fun writeStatus(message: StatusMessage?) {
        // panelcore 未消费状态流。
      }

      override fun writeGroups(message: OutboundGroupIterator?) {
        // panelcore 未消费分组流。
      }

      override fun writeOutbounds(message: OutboundGroupItemIterator?) {
        // panelcore 未消费出站流。
      }

      override fun initializeClashMode(modeList: StringIterator?, currentMode: String?) {
        // panelcore 未消费 Clash 模式流。
      }

      override fun updateClashMode(newMode: String?) {
        // panelcore 未消费 Clash 模式流。
      }

      override fun writeConnectionEvents(events: ConnectionEvents?) {
        // panelcore 未消费连接事件流。
      }
    }

  /**
   * 每进程只调用一次 Libbox.setup：重复 setup 会重置 Go 侧全局状态（日志、数据
   * 路径、崩溃处理等），可能污染正在运行的核心，已初始化则直接跳过。
   */
  private fun setupLibbox(setup: SetupOptions) {
    if (setupDone) {
      return
    }
    synchronized(setupLock) {
      if (setupDone) {
        return
      }
      Libbox.setup(setup)
      setupDone = true
    }
  }

  /** 批量追加 libbox 日志到环形缓冲，超出容量弹出最旧。 */
  private fun appendLibboxLogs(lines: List<String>) {
    synchronized(libboxLogBuffer) {
      for (line in lines) {
        libboxLogBuffer.addLast(line)
        while (libboxLogBuffer.size > LIBBOX_LOG_BUFFER_SIZE) {
          libboxLogBuffer.removeFirst()
        }
      }
    }
  }

  /** 清空日志环形缓冲并标记文件待重置（核心重连重放完整日志前调用）。 */
  private fun clearLibboxLogBuffer() {
    synchronized(libboxLogBuffer) {
      libboxLogBuffer.clear()
      libboxLogFileResetPending = true
    }
  }

  /** 取环形缓冲最近 `lines` 行（空缓冲时返回空串）。 */
  private fun libboxLogTail(lines: Int): String {
    synchronized(libboxLogBuffer) {
      return libboxLogBuffer.takeLast(lines).joinToString("\n")
    }
  }

  /** 当前 RFC3339 本地时间戳（毫秒精度），与 [`LOG_TS_FORMATTER`] 对齐。 */
  private fun timestampNow(): String = LOG_TS_FORMATTER.format(OffsetDateTime.now())

  /**
   * 批量追加 libbox 日志到 `filesDir/logs/libbox.log`（与 Rust 的 data_dir/logs
   * 同目录，供日志导出排查 Go 侧错误链）。重连重放时先清空文件；超过 1MB 时
   * 截断：删除重建写入最新缓冲。任何写入异常一律静默，不得影响 VPN 主流程。
   */
  private fun appendToLibboxLogFile(lines: List<String>) {
    try {
      val logDir = File(filesDir, "logs")
      if (!logDir.isDirectory && !logDir.mkdirs()) {
        return
      }
      val logFile = File(logDir, "libbox.log")
      val reset =
        synchronized(libboxLogBuffer) {
          val pending = libboxLogFileResetPending
          libboxLogFileResetPending = false
          pending
        }
      if (reset) {
        logFile.writeText("")
      } else if (logFile.exists() && logFile.length() > LIBBOX_LOG_MAX_BYTES) {
        logFile.delete()
        logFile.writeText(libboxLogTail(LIBBOX_LOG_BUFFER_SIZE))
      }
      logFile.appendText(lines.joinToString("\n", postfix = "\n"))
    } catch (_: Exception) {
      // 静默：日志写入失败不影响 VPN 主流程。
    }
  }

  /** 失败信息附带最近 15 行 libbox 日志，供前端 Alert 直接展示 Go 侧错误链。 */
  private fun withLibboxLogTail(message: String): String {
    val tail = libboxLogTail(15)
    return if (tail.isEmpty()) message else "$message\n--- libbox 日志 ---\n$tail"
  }

  /** 后台线程执行的有序关闭（stopBox 阻塞 close，防主线程 ANR）。 */
  private fun closeBoxInBackground() {
    Thread {
      try {
        stopBox()
      } catch (e: Exception) {
        Log.e(TAG, "failed to stop libbox service", e)
        lastError = buildExceptionChain(e)
      }
    }.start()
  }

  /**
   * 有序关闭核心与 TUN：复位 running → 关闭 BoxService → 关闭 tun pfd。
   * 幂等且加锁（跨实例共享 lifecycleLock），避免 stop intent 与 onDestroy 并发
   * 双线程重复 close。仅持有者实例可关闭：旧实例迟到的 stop 不得误关新实例已
   * 启动的 box（防「先 stop 再 start」重启时序下误伤新实例）。
   */
  private fun stopBox() {
    synchronized(lifecycleLock) {
      if (boxOwner != null && boxOwner !== this) {
        // 本实例不是当前 box 的持有者（已被重启序列接管），迟到 stop 直接返回。
        Log.i(TAG, "stop ignored: box owned by another instance")
        return
      }
      running = false
      val server = commandServer
      commandServer = null
      boxOwner = null
      stopLogClient()
      // 关闭顺序：先关 pfd（原始 tun fd），再关核心服务（关闭核心持有的 dup fd /
      // tun 接口），最后关 CommandServer（gRPC listener）。
      try {
        tunFd?.close()
      } catch (e: Exception) {
        Log.e(TAG, "failed to close tun fd", e)
        lastError = buildExceptionChain(e)
      }
      tunFd = null
      if (server != null) {
        try {
          server.closeService()
        } catch (e: Exception) {
          Log.e(TAG, "failed to close libbox service", e)
          lastError = buildExceptionChain(e)
        }
        try {
          server.close()
        } catch (e: Exception) {
          Log.e(TAG, "failed to close command server", e)
        }
      }
    }
  }

  /** 拼接完整异常链：e.message + 逐层 e.cause，保留根因（如 gomobile 包装的 Go 错误）。 */
  private fun buildExceptionChain(e: Throwable): String {
    val parts = mutableListOf<String>()
    var current: Throwable? = e
    var depth = 0
    while (current != null && depth < 10) {
      val message = current.message ?: current.javaClass.simpleName
      if (depth == 0) {
        parts.add(message)
      } else {
        parts.add("caused by: $message")
      }
      current = current.cause
      depth++
    }
    return parts.joinToString("\n")
  }

  private fun stopForegroundCompat() {
    try {
      ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
    } catch (e: Exception) {
      Log.w(TAG, "stopForeground failed: ${e.message}")
    }
  }

  private fun addExcludeRoute(builder: Builder, route: RoutePrefix) {
    try {
      builder.excludeRoute(IpPrefix(InetAddress.getByName(route.address()), route.prefix()))
    } catch (e: Exception) {
      Log.w(TAG, "excludeRoute failed for ${route.address()}: ${e.message}")
    }
  }
}

/**
 * 默认网络接口监控：跟踪系统默认网络并把其物理接口（name/index）推送给
 * libbox 的 [InterfaceUpdateListener]，sing-box `route.auto_detect_interface`
 * 依赖该推送学习默认出站接口；此前为空实现导致核心学不到任何接口，所有
 * 出站拨号报 `no available network interface`。
 *
 * 设计对齐 SFA（SagerNet/sing-box-for-android）DefaultNetworkMonitor：
 * - 注册：minSdk 33 后统一用 registerBestMatchingNetworkCallback（31+ 分支），
 *   28–30 / 26–27 的低版本分支已不可达；
 * - 防御性过滤：capabilities 含 TRANSPORT_VPN 的网络（含本服务 tun）绝不上报
 *   为默认接口；
 * - 回调只更新状态快照，取 LinkProperties / 接口 index（最多重试 10 次、间隔
 *   100ms）放到后台线程执行，避免阻塞回调线程（SFA 在回调里直接 sleep 是已知
 *   粗糙点，这里改为工作线程重试）。
 */
internal object DefaultInterfaceMonitor {

  private const val TAG = "DefaultInterfaceMonitor"
  private const val MAX_RETRY = 10
  private const val RETRY_INTERVAL_MS = 100L

  private val lock = Any()
  private val mainHandler = Handler(Looper.getMainLooper())

  private var connectivity: ConnectivityManager? = null
  private var listener: InterfaceUpdateListener? = null
  private var defaultNetwork: Network? = null
  private var registered = false

  private val request =
    NetworkRequest.Builder()
      .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
      .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_RESTRICTED)
      .build()

  /** 网络回调（运行在主线程）：只更新状态快照，推送交给后台线程。 */
  private val callback =
    object : ConnectivityManager.NetworkCallback() {
      override fun onAvailable(network: Network) {
        setNetwork(network)
      }

      override fun onCapabilitiesChanged(
        network: Network,
        networkCapabilities: NetworkCapabilities,
      ) {
        if (networkCapabilities.hasTransport(NetworkCapabilities.TRANSPORT_VPN)) {
          // 防御性过滤：VPN 隧道网络（含本服务 tun）绝不作为默认接口推送。
          clearNetwork(network)
        } else {
          setNetwork(network)
        }
      }

      override fun onLost(network: Network) {
        clearNetwork(network)
      }
    }

  /** 启动监控：注册网络回调并立即推送一次当前默认网络。 */
  fun start(listener: InterfaceUpdateListener?, connectivity: ConnectivityManager) {
    synchronized(lock) {
      this.connectivity = connectivity
      this.listener = listener
      if (!registered) {
        register(connectivity)
        registered = true
      }
      // 立即推送一次当前默认网络；若当前默认恰为 VPN 网络则留空，等待回调纠正。
      val active = connectivity.activeNetwork
      defaultNetwork = if (active != null && isVpnNetwork(connectivity, active)) null else active
    }
    push()
  }

  fun close(listener: InterfaceUpdateListener?) {
    synchronized(lock) {
      if (this.listener !== listener) {
        // 监听器已被新一轮 start 替换，迟到 close 不做任何事。
        return
      }
      this.listener = null
      defaultNetwork = null
      if (registered) {
        runCatching { connectivity?.unregisterNetworkCallback(callback) }
        registered = false
      }
    }
  }

  private fun setNetwork(network: Network) {
    synchronized(lock) {
      defaultNetwork = network
    }
    push()
  }

  private fun clearNetwork(network: Network) {
    synchronized(lock) {
      if (network == defaultNetwork) {
        defaultNetwork = null
      }
    }
    push()
  }

  private fun isVpnNetwork(connectivity: ConnectivityManager, network: Network): Boolean =
    connectivity.getNetworkCapabilities(network)
      ?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true

  private fun register(connectivity: ConnectivityManager) {
    // minSdk 33：恒走 31+ 的 registerBestMatchingNetworkCallback（低版本分支已不可达）。
    connectivity.registerBestMatchingNetworkCallback(request, callback, mainHandler)
  }

  private fun push() {
    val listener = synchronized(lock) { listener }
    val connectivity = synchronized(lock) { connectivity }
    val network = synchronized(lock) { defaultNetwork }
    if (listener == null) {
      return
    }
    if (connectivity == null) {
      return
    }
    // 取 LinkProperties / 接口 index 可能需重试（含 sleep），放后台线程，
    // 禁止阻塞回调线程（主线程 / ConnectivityThread）。
    Thread { pushWorker(listener, connectivity, network) }.start()
  }

  private fun pushWorker(
    listener: InterfaceUpdateListener?,
    connectivity: ConnectivityManager,
    network: Network?,
  ) {
    if (network == null) {
      // 默认网络丢失：上报空接口（index=-1），让核心感知默认接口消失。
      listener?.updateDefaultInterface("", -1, false, false)
      return
    }
    for (attempt in 0 until MAX_RETRY) {
      // 等待期间默认网络已切换/丢失：放弃本轮，新回调会再触发推送。
      if (synchronized(lock) { defaultNetwork } !== network) {
        return
      }
      val linkProperties = connectivity.getLinkProperties(network)
      val name = linkProperties?.interfaceName
      val index =
        name?.let { runCatching { NetworkInterface.getByName(it).index }.getOrNull() }
      if (name.isNullOrEmpty() || index == null) {
        Thread.sleep(RETRY_INTERVAL_MS)
        continue
      }
      // isExpensive/isConstrained：对齐 SFA 当前实现传 false（核心另有
      // getInterfaces 上报的 metered 属性），本参数仅影响默认接口的
      // expensive/constrained 标记规则匹配。
      listener?.updateDefaultInterface(name, index, false, false)
      return
    }
    // 重试耗尽仍取不到接口（罕见）：记录日志，等下一次网络回调再试。
    Log.w(TAG, "failed to resolve default interface after $MAX_RETRY attempts")
  }
}

/** [StringIterator] 实现：适配 Kotlin [Iterator]（`len` 核心不使用，返回 0）。 */
private class StringArray(private val iterator: Iterator<String>) : StringIterator {
  override fun len(): Int = 0
  override fun hasNext(): Boolean = iterator.hasNext()
  override fun next(): String = iterator.next()
}

/** [NetworkInterfaceIterator] 实现：适配 Kotlin [Iterator]。 */
private class InterfaceArray(
  private val iterator: Iterator<LibboxNetworkInterface>,
) : NetworkInterfaceIterator {
  override fun hasNext(): Boolean = iterator.hasNext()
  override fun next(): LibboxNetworkInterface = iterator.next()
}

/** [InterfaceAddress] → `ip/prefix` 文本（IPv6 去掉 scope id 后拼接）。 */
private fun InterfaceAddress.toPrefix(): String =
  if (address is Inet6Address) {
    "${Inet6Address.getByAddress(address.address).hostAddress}/$networkPrefixLength"
  } else {
    "${address.hostAddress}/$networkPrefixLength"
  }
