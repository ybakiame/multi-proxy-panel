package com.proxypanel.client

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.PackageManager.NameNotFoundException
import android.content.pm.ServiceInfo
import android.net.IpPrefix
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.os.Process
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import io.nekohasekai.libbox.BoxService
import io.nekohasekai.libbox.InterfaceUpdateListener
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.LocalDNSTransport
import io.nekohasekai.libbox.NetworkInterface
import io.nekohasekai.libbox.NetworkInterfaceIterator
import io.nekohasekai.libbox.PlatformInterface
import io.nekohasekai.libbox.RoutePrefix
import io.nekohasekai.libbox.SetupOptions
import io.nekohasekai.libbox.StringIterator
import io.nekohasekai.libbox.TunOptions
import io.nekohasekai.libbox.WIFIState
import io.nekohasekai.libbox.Notification as LibboxNotification
import java.io.File
import java.net.InetAddress
import java.time.OffsetDateTime
import java.time.format.DateTimeFormatter
import java.util.NoSuchElementException

/**
 * TUN/VPN 前台服务：通过 libbox（sing-box）驱动，由 [VpnPlugin] 的
 * start/stop 命令控制启停。
 *
 * 启动序列（对齐 sing-box experimental/libbox）：
 *   1. Libbox.setup(SetupOptions) 设置数据路径
 *   2. Libbox.newService(config, platformInterface) 解析配置并创建服务
 *   3. BoxService.start() 启动核心（openTun 由 [PlatformInterface] 回调，
 *      用 VpnService.Builder.establish() 取得 tun fd 返回给核心）
 *
 * 停止序列：BoxService.close()，随后 VpnService 会撤销本服务持有的 fd。
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
  }

  private var boxService: BoxService? = null
  private var tunFd: ParcelFileDescriptor? = null

  /** 停止请求标记：startBox 在后台线程启动期间到达 stop 时，启动完成后立即有序关闭。 */
  @Volatile
  private var stopRequested = false

  private val mainHandler = Handler(Looper.getMainLooper())

  /**
   * libbox 回调实现。除 openTun 外均为最小实现：
   * 本步只保证启动链路可用，接口监控/连接归属/证书等功能后续按需补齐。
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
      try {
        tunFd = pfd
        return pfd.detachFd()
      } catch (e: Exception) {
        // establish 已成功但后续步骤失败：回收半开 pfd 防 fd 泄漏。detachFd()
        // 成功后 fd 所有权已归 libbox，Kotlin 不再 close（stopBox 对 detached
        // 对象的 close 是无害的，保留原有清理路径）。
        runCatching { pfd.close() }
        throw e
      }
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
      // 无默认接口监控（最小实现）。
    }

    override fun closeDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
      // 无默认接口监控（最小实现）。
    }

    override fun getInterfaces(): NetworkInterfaceIterator = EmptyNetworkInterfaceIterator

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

    // 新的启动序列开始，清除上一轮停止请求标记。
    stopRequested = false

    // newService/start 为阻塞调用，放到后台线程执行。
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
        // openTun（establish 前后）与 startBox 失败都落到这里：附带 libbox 最近
        // 日志，让前端 Alert 直接展示 Go 侧错误链（如 "query tun name" / "dup tun
        // file descriptor"），便于定位真机 file already closed 的完整根因链。
        lastError = withLibboxLogTail(e.message ?: e.javaClass.simpleName)
        stopSelf()
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
        lastError = e.message ?: e.javaClass.simpleName
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
    // 数据路径：basePath 用于日志等，workingPath/tempPath 供核心缓存使用。
    val setup =
      SetupOptions().apply {
        basePath = filesDir.absolutePath
        workingPath = filesDir.absolutePath
        tempPath = cacheDir.absolutePath
        // 不设置 username：Go 侧 os/user.Lookup 在 Android 上可能解析不到
        // "app_<uid>"，留空则回退 os.Getuid()（即当前应用 uid），符合单应用场景。
        isTVOS = false
        fixAndroidStack = true
      }
    setupLibbox(setup)

    val service = Libbox.newService(config, platformInterface)
    // 注册与 start 放同一临界区：stopBox 的 synchronized 会等待 start() 完成再
    // close，避免停止线程在启动期间 close 半启动的服务；start 抛异常时 boxService
    // 已注册，onDestroy 仍能有序关闭释放资源。
    synchronized(this) {
      boxService = service
      service.start()
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
   * 追加 libbox 日志到 `<pkg>/logs/libbox.log`（与 Rust 的 data_dir/logs 同目录，
   * 供日志导出排查 Go 侧错误链）。超过 1MB 时截断：删除重建写入最新缓冲。
   * 任何写入异常一律静默，不得影响 VPN 主流程。
   */
  private fun appendToLibboxLogFile(message: String) {
    try {
      val logDir = File(filesDir.parentFile, "logs")
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
        lastError = e.message ?: e.javaClass.simpleName
      }
    }.start()
  }

  /**
   * 有序关闭核心与 TUN：复位 running → 关闭 BoxService → 关闭 tun fd。
   * 幂等且加锁，避免 stop intent 与 onDestroy 并发双线程重复 close。
   */
  private fun stopBox() {
    synchronized(this) {
      running = false
      val service = boxService ?: return
      boxService = null
      try {
        service.close()
      } catch (e: Exception) {
        Log.e(TAG, "failed to close libbox service", e)
        lastError = e.message ?: e.javaClass.simpleName
      }
      try {
        tunFd?.close()
      } catch (e: Exception) {
        Log.e(TAG, "failed to close tun fd", e)
        lastError = e.message ?: e.javaClass.simpleName
      }
      tunFd = null
    }
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

  private object EmptyNetworkInterfaceIterator : NetworkInterfaceIterator {
    override fun hasNext(): Boolean = false
    override fun next(): NetworkInterface = throw NoSuchElementException("empty iterator")
  }
}
