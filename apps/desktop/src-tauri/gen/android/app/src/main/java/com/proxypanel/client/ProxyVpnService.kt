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
import android.os.Process
import android.system.OsConstants
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.proxypanel.core.libbox.BoxService
import com.proxypanel.core.libbox.InterfaceUpdateListener
import com.proxypanel.core.libbox.Libbox
import com.proxypanel.core.libbox.LocalDNSTransport
import com.proxypanel.core.libbox.NetworkInterface as LibboxNetworkInterface
import com.proxypanel.core.libbox.NetworkInterfaceIterator
import com.proxypanel.core.libbox.PlatformInterface
import com.proxypanel.core.libbox.RoutePrefix
import com.proxypanel.core.libbox.SetupOptions
import com.proxypanel.core.libbox.StringIterator
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
 * 启动序列（对齐 sing-box experimental/libbox 与 SFA 官方语义）：
 *   1. Libbox.setup(SetupOptions) 设置数据路径（进程内仅一次）
 *   2. Libbox.newService(config, platformInterface) 解析配置并创建服务
 *   3. BoxService.start() 启动核心（openTun 由 [PlatformInterface] 回调，
 *      用 VpnService.Builder.establish() 取得 pfd，保留所有权并返回 `pfd.fd`
 *      原始 fd 号给核心；核心侧 `dup(fd)` 出一份独立 fd 供 tun 使用）
 *
 * 停止序列：先 close pfd（原始 fd），再 BoxService.close()（关闭核心持有的
 * dup fd / tun 接口），随后 VpnService 撤销 VPN。
 *
 * fd 生命周期对齐 SFA：**不调用 detachFd()**。detachFd 会把原始 fd 所有权转交
 * 出去且 Kotlin 侧 `close()` 变成空操作，导致原始 fd 无人关闭（fd 泄漏）并绕过
 * VpnService 的 fd 生命周期跟踪，是真机「file already closed」的诱因。
 *
 * 重复启动防护：`startInProgress` 标志保证同一时刻只有一个 startBox 序列；
 * 重启（服务存活时再次收到 start）在 startBox 内先有序关闭旧实例再启动新实例，
 * 禁止两个 BoxService 并发 establish()（并发 establish 会撤销前一 tun 导致
 * 启动期即报 file already closed）。
 */
class ProxyVpnService : VpnService() {

  companion object {
    private const val TAG = "ProxyVpnService"
    private const val CHANNEL_ID = "vpn"
    private const val NOTIFICATION_ID = 1

    /** Intent extra：sing-box JSON 配置内容。 */
    const val EXTRA_CONFIG = "config"

    /**
     * 显式停止 action：由 [VpnPlugin] 发送。onStartCommand 收到后有序关闭
     * BoxService（后台线程）→ 退前台 → 自停 → 复位 running；裸 stopService 只
     * 走 onDestroy，close 仍阻塞主线程且不记录 lastError。
     */
    const val ACTION_STOP = "com.proxypanel.client.action.STOP"

    /**
     * 服务运行标记，由服务自身真实生命周期维护（startBox 成功后置 true，
     * 失败 / onDestroy / onRevoke 置 false）。VpnPlugin 不乐观设置。
     */
    @Volatile
    var running = false
      private set

    /**
     * 服务实例存活标记（onCreate 置 true，onDestroy 置 false）：启动中的服务
     * `running` 仍为 false 但实例已在前台，VpnPlugin 据此判断能否安全派发
     * ACTION_STOP（避免启动中被 stop 漏关）。
     */
    @Volatile
    var instanceAlive = false
      private set

    /** 最近一次启动失败原因（成功启动后清空；供插件 isRunning 命令与前端轮询读取）。 */
    @Volatile
    var lastError: String? = null
      private set

    /**
     * libbox writeLog 环形缓冲（最近 200 行）：启动失败时附带进 lastError，
     * 让前端 Alert 直接看到 Go 侧完整错误链（如 "query tun name" /
     * "dup tun file descriptor" / "initialize inbound/tun" 前缀），定位异步失败。
     *
     * 每行以 `[RFC3339] ` 时间戳前缀开头（[writeLog] 统一写入，与
     * `logs/libbox.log` 同源），文件超限截断时重建内容仍为可解析的完整行。
     */
    private val libboxLogBuffer = ArrayDeque<String>(200)

    /** `logs/libbox.log` 大小上限（字节），超限时删除重建写入最新缓冲。 */
    private const val LIBBOX_LOG_MAX_BYTES = 1024 * 1024

    /**
     * RFC3339 本地时间格式（毫秒精度，如 `2026-08-02T22:12:30.123+08:00`）。
     * 与 Rust 日志页 `LogEntry.ts` 对齐，供日志查看器 `get_logs` 解析排序。
     * `java.time` 自 API 26 起可用（minSdk 26，无需 desugaring）。
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
     * 当前注册的 BoxService 及其 tun pfd，进程级共享（跨实例）：
     * 重启时新实例的 startBox 需先关闭旧实例残留的 box（「先有序 stop 再 start」），
     * 而旧实例迟到的 stopBox 不得关闭新实例已启动的 box（用 [boxOwner] 判定归属）。
     * 所有访问必须在 [lifecycleLock] 临界区内进行。
     */
    private var boxService: BoxService? = null

    private var tunFd: ParcelFileDescriptor? = null

    /** 当前 box/tun 的持有者实例：仅持有者可关闭，防旧实例迟到的 stop 误关新 box。 */
    private var boxOwner: ProxyVpnService? = null
  }

  /** 停止请求标记：startBox 在后台线程启动期间到达 stop 时，启动完成后立即有序关闭。 */
  @Volatile
  private var stopRequested = false

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

      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        builder.setMetered(false)
      }

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
        try {
          builder.addDnsServer(options.dnsServerAddress.value)
        } catch (e: Exception) {
          Log.w(TAG, "no dns server address available: ${e.message}")
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
          // API 33+ 支持精确路由 + excludeRoute。
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
        } else {
          // 旧 API 只能 addRoute，走核心算好的 auto-route 区间。
          val inet4RouteRange = options.inet4RouteRange
          if (inet4RouteRange.hasNext()) {
            while (inet4RouteRange.hasNext()) {
              val route = inet4RouteRange.next()
              builder.addRoute(route.address(), route.prefix())
            }
          }

          val inet6RouteRange = options.inet6RouteRange
          if (inet6RouteRange.hasNext()) {
            while (inet6RouteRange.hasNext()) {
              val route = inet6RouteRange.next()
              builder.addRoute(route.address(), route.prefix())
            }
          }
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

    override fun writeLog(message: String) {
      // 时间戳前缀与 Rust 日志页 `LogEntry.ts` 对齐：环形缓冲与 `logs/libbox.log`
      // 统一存 `[RFC3339] message`，Rust `get_logs` 按行解析后合并展示。
      val line = "[${timestampNow()}] $message"
      Log.i(TAG, message)
      appendLibboxLog(line)
      appendToLibboxLogFile(line)
    }

    override fun useProcFS(): Boolean = Build.VERSION.SDK_INT < Build.VERSION_CODES.Q

    override fun findConnectionOwner(
      ipProtocol: Int,
      sourceAddress: String,
      sourcePort: Int,
      destinationAddress: String,
      destinationPort: Int,
    ): Int = -1

    override fun packageNameByUid(uid: Int): String =
      if (uid == Process.myUid()) this@ProxyVpnService.packageName else ""

    override fun uidByPackageName(packageName: String): Int =
      if (packageName == this@ProxyVpnService.packageName) Process.myUid() else -1

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

    override fun systemCertificates(): StringIterator = EmptyStringIterator

    override fun clearDNSCache() {
      // 无本地 DNS 缓存（最小实现）。
    }

    override fun sendNotification(notification: LibboxNotification) {
      // 通知透传留待后续实现。
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

    val config = intent?.getStringExtra(EXTRA_CONFIG)
    if (config.isNullOrBlank()) {
      Log.w(TAG, "start command without config, stopping")
      lastError = "缺少 VPN 配置"
      stopSelf()
      return Service.START_NOT_STICKY
    }

    try {
      startForegroundWithNotification()
    } catch (e: Exception) {
      Log.e(TAG, "failed to start foreground service", e)
      running = false
      lastError = "前台服务启动失败: ${e.message ?: e.javaClass.simpleName}"
      stopSelf()
      return Service.START_NOT_STICKY
    }

    // 重复 start 防护：同一时刻只允许一个 startBox 序列（对齐 SFA onStartCommand
    // 的 status != Stopped 守卫）。已有启动序列进行中时忽略本次 start，避免两个
    // BoxService 并发 establish()（并发 establish 会撤销前一 tun 导致
    // 启动期即报 file already closed）。
    if (startInProgress) {
      Log.w(TAG, "start ignored: another start sequence is in progress")
      return Service.START_NOT_STICKY
    }

    // 新的启动序列开始，清除上一轮停止请求标记。
    stopRequested = false

    // newService/start 为阻塞调用，放到后台线程执行（对齐 SFA：onStartCommand
    // 立即返回，start 在 IO 后台线程异步执行，不做同步等待）。
    startInProgress = true
    Thread {
      try {
        startBox(config)
        if (stopRequested) {
          // 停止请求在启动期间到达：启动已完成但立即有序关闭，不置 running。
          Log.i(TAG, "stop requested during startup, closing service")
          stopBox()
        } else {
          running = true
          lastError = null
          Log.i(TAG, "libbox service started")
        }
      } catch (e: Exception) {
        Log.e(TAG, "failed to start libbox service", e)
        running = false
        // openTun（establish 前后）与 startBox 失败都落到这里：附带完整异常链
        // （e.message + e.cause 逐层）+ libbox 最近日志，让前端 Alert 直接展示
        // Go 侧错误链（如 "query tun name" / "dup tun file descriptor"），便于
        // 定位真机 file already closed 的完整根因链。
        lastError = withLibboxLogTail(buildExceptionChain(e))
        stopSelf()
      } finally {
        startInProgress = false
      }
    }.start()

    return Service.START_NOT_STICKY
  }

  /**
   * 显式停止：在后台线程有序关闭 BoxService（`close()` 为阻塞调用，放主线程会
   * ANR），完成后经主线程退前台、自停；`running` 立即复位供状态轮询读取。
   */
  private fun handleStop() {
    stopRequested = true
    running = false
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
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      val channel =
        NotificationChannel(CHANNEL_ID, "ProxyPanel VPN", NotificationManager.IMPORTANCE_LOW)
      getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }

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

  private fun startBox(config: String) {
    // 数据路径（对齐 SFA Application.kt:99-110）：basePath 用内部 filesDir，
    // workingPath 用外部 files 目录（外部存储可用时）供核心缓存使用，tempPath
    // 用 cacheDir；外部目录不可用时回退内部 filesDir。fixAndroidStack 开启以
    // 修复 Android 上 Go 栈回溯问题。username 留空回退 os.Getuid()（单应用场景）。
    val workingDir = getExternalFilesDir(null)
    val setup =
      SetupOptions().apply {
        basePath = filesDir.absolutePath
        workingPath = workingDir?.absolutePath ?: filesDir.absolutePath
        tempPath = cacheDir.absolutePath
        isTVOS = false
        fixAndroidStack = true
      }
    setupLibbox(setup)

    val service = Libbox.newService(config, platformInterface)
    // 注册、有序重启与 start 放同一临界区（跨实例共享 lifecycleLock）：
    //   - 若存在旧 box（重启：服务存活时再次 start / 旧实例残留），先有序关闭
    //     再启动新实例，杜绝两个 BoxService 并发 establish()；
    //   - stopBox 的锁会等待 start() 完成再 close，避免停止线程在启动期间
    //     close 半启动的服务；
    //   - start 抛异常时立即 close 该实例并复位状态，清理半启动痕迹。
    synchronized(lifecycleLock) {
      // 先有序 stop 再 start：关闭上一轮实例持有的 box 与 tun fd。
      boxService?.let { old ->
        boxService = null
        boxOwner = null
        try {
          old.close()
        } catch (e: Exception) {
          Log.e(TAG, "failed to close previous libbox service", e)
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

      boxService = service
      boxOwner = this
      try {
        service.start()
      } catch (e: Exception) {
        // 半启动清理：注册了但 start 抛异常（可能 openTun 已成功、后续 inbound
        // 失败），立即 close 该实例、回收 tun pfd 并复位，防 fd 泄漏。
        boxService = null
        boxOwner = null
        runCatching { service.close() }
        runCatching { tunFd?.close() }
        tunFd = null
        throw e
      }
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

  /** 追加一行 libbox 日志到环形缓冲，超出容量弹出最旧。 */
  private fun appendLibboxLog(message: String) {
    synchronized(libboxLogBuffer) {
      libboxLogBuffer.addLast(message)
      while (libboxLogBuffer.size > 200) {
        libboxLogBuffer.removeFirst()
      }
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
   * 追加 libbox 日志到 `filesDir/logs/libbox.log`（与 Rust 的 data_dir/logs 同目录，
   * 供日志导出排查 Go 侧错误链）。超过 1MB 时截断：删除重建写入最新缓冲。
   * 任何写入异常一律静默，不得影响 VPN 主流程。
   */
  private fun appendToLibboxLogFile(message: String) {
    try {
      val logDir = File(filesDir, "logs")
      if (!logDir.isDirectory && !logDir.mkdirs()) {
        return
      }
      val logFile = File(logDir, "libbox.log")
      if (logFile.exists() && logFile.length() > LIBBOX_LOG_MAX_BYTES) {
        logFile.delete()
        logFile.writeText(libboxLogTail(200))
      }
      logFile.appendText(message + "\n")
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
      val service = boxService ?: return
      boxService = null
      boxOwner = null
      // 关闭顺序对齐 SFA（BoxService.kt serviceStop/stopService）：先关 pfd（原始
      // tun fd），再关 BoxService（关闭核心持有的 dup fd / tun 接口）。
      try {
        tunFd?.close()
      } catch (e: Exception) {
        Log.e(TAG, "failed to close tun fd", e)
        lastError = buildExceptionChain(e)
      }
      tunFd = null
      try {
        service.close()
      } catch (e: Exception) {
        Log.e(TAG, "failed to close libbox service", e)
        lastError = buildExceptionChain(e)
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

  private object EmptyStringIterator : StringIterator {
    override fun hasNext(): Boolean = false
    override fun next(): String = ""
    override fun len(): Int = 0
  }
}

/**
 * 默认网络接口监控：跟踪系统默认网络并把其物理接口（name/index）推送给
 * libbox 的 [InterfaceUpdateListener]，sing-box `route.auto_detect_interface`
 * 依赖该推送学习默认出站接口；此前为空实现导致核心学不到任何接口，所有
 * 出站拨号报 `no available network interface`。
 *
 * 除 libbox 接口推送外，另提供轻量 DNS 订阅钩子 [onDefaultNetworkChanged]：
 * 默认网络变化时把新网络的 DNS 服务器列表回调给订阅方（mihomo 用
 * `Mihomocore.updateDns` 刷新核心系统 DNS 与缓存，对齐 FlClash
 * NetworkObserveModule.onUpdateNetwork）；网络丢失时回调空列表。钩子可独立于
 * libbox 监听器工作（listener 传 null 仅订阅 DNS），不影响既有 libbox 语义。
 *
 * 设计对齐 SFA（SagerNet/sing-box-for-android）DefaultNetworkMonitor：
 * - 注册按 API 分级（minSdk 26）：31+ 用 registerBestMatchingNetworkCallback，
 *   28–30 用 requestNetwork（REQUEST 模式；registerDefaultNetworkCallback 自
 *   Android P 起会把 VPN 网络自身报为默认），26–27 用 registerDefaultNetworkCallback；
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

  /**
   * 轻量 DNS 订阅钩子（对齐 FlClash NetworkObserveModule.onUpdateNetwork）：
   * 默认网络变化时回调新网络链路的 DNS 服务器列表（`hostAddress` 文本；网络
   * 丢失回调空列表），供 mihomo 核心刷新系统 DNS 与缓存。
   *
   * 可独立于 libbox 监听器使用（`start(null, ...)` 仅订阅 DNS）；空列表用于
   * 通知订阅方网络丢失，由订阅方决定是否跳过刷新。`@Volatile` 保证跨线程
   * 可见：读取在 [lock] 临界区内进行，赋值由订阅方在自身生命周期锁内完成。
   */
  @Volatile
  var onDefaultNetworkChanged: ((dnsServers: List<String>) -> Unit)? = null

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

  /** 启动监控：注册网络回调并立即推送一次当前默认网络。listener 可为 null（仅订阅 DNS 钩子，见 [onDefaultNetworkChanged]）。 */
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
    when {
      Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
        connectivity.registerBestMatchingNetworkCallback(request, callback, mainHandler)
      Build.VERSION.SDK_INT >= Build.VERSION_CODES.P ->
        // REQUEST 模式：只报告默认网络，避免 P+ registerDefaultNetworkCallback
        // 把 VPN 网络自身报为默认（需 CHANGE_NETWORK_STATE 权限）。
        connectivity.requestNetwork(request, callback, mainHandler)
      else ->
        connectivity.registerDefaultNetworkCallback(callback, mainHandler)
    }
  }

  private fun push() {
    val listener = synchronized(lock) { listener }
    val connectivity = synchronized(lock) { connectivity }
    val network = synchronized(lock) { defaultNetwork }
    val dnsHook = synchronized(lock) { onDefaultNetworkChanged }
    // 无 libbox 监听且无 DNS 订阅钩子时无需推送。
    if (listener == null && dnsHook == null) {
      return
    }
    if (connectivity == null) {
      return
    }
    // 取 LinkProperties / 接口 index 可能需重试（含 sleep），放后台线程，
    // 禁止阻塞回调线程（主线程 / ConnectivityThread）。
    Thread { pushWorker(listener, connectivity, network, dnsHook) }.start()
  }

  private fun pushWorker(
    listener: InterfaceUpdateListener?,
    connectivity: ConnectivityManager,
    network: Network?,
    dnsHook: ((List<String>) -> Unit)?,
  ) {
    if (network == null) {
      // 默认网络丢失：上报空接口（index=-1），让核心感知默认接口消失；
      // DNS 订阅回调空列表（订阅方按空列表跳过刷新）。
      listener?.updateDefaultInterface("", -1, false, false)
      dnsHook?.invoke(emptyList())
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
      // 对齐 FlClash NetworkObserveModule.onUpdateNetwork：默认网络切换时把新
      // 网络的 DNS 服务器列表回调给订阅方（mihomo 用 updateDns 刷新核心系统
      // DNS 与缓存；空列表说明无 DNS 服务器）。
      val dnsServers = linkProperties.dnsServers.mapNotNull { it.hostAddress }
      dnsHook?.invoke(dnsServers)
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
