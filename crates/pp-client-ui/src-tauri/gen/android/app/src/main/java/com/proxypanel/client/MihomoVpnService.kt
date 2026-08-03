package com.proxypanel.client

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.PackageManager.NameNotFoundException
import android.content.pm.ServiceInfo
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.proxypanel.core.mihomocore.Callback
import com.proxypanel.core.mihomocore.Mihomocore
import java.io.File
import java.time.OffsetDateTime
import java.time.format.DateTimeFormatter

/**
 * TUN/VPN 前台服务：通过 panelcore.aar（合并 gomobile 绑定，包
 * `com.proxypanel.core.mihomocore`）驱动 mihomo 核心，由 [VpnPlugin] 的
 * start/stop 命令控制启停。
 *
 * P1 spike 最小实现（对齐 ProxyVpnService 结构，但不做接口监控 / per-app /
 * 通知 action），启动序列：
 *   1. `Mihomocore.setup(homeDir, configYAML, callback)`：设置 HomeDir、保存
 *      回调并解析应用 mihomo YAML 配置（TUN 不由配置启用，由 StartTun 管理）
 *   2. `VpnService.Builder.establish()`：建立 TUN，取得 pfd 并保留所有权
 *      （同 sing-box 侧注释语义：不调用 detachFd()，stop 时关闭原始 fd）
 *   3. `Mihomocore.startTun(fd, "mixed", "172.19.0.1/30", "172.19.0.2")`：以
 *      VpnService 提供的 fd 创建 sing_tun listener 并安装 Protect 钩子
 *      （stack/address/dns 与 wrapper 侧 Setup 对应；dns 参数指向 tun 网关地址，
 *      wrapper 拼为 `172.19.0.2:53` 的 DNSHijack 规则，由核心 DNS 模块接管，
 *      对齐 FlClash 做法）
 *
 * 停止序列：`Mihomocore.stop()`（幂等，含 StopTun）→ 关闭 pfd → 复位 running。
 *
 * 回调 [callback]：`protect` 转交 [VpnService.protect]（使出站连接绕过 VPN），
 * `writeLog` 写入 200 行环形缓冲 + `filesDir/logs/mihomo.log`（`[RFC3339] `
 * 前缀、1MB 截断，格式对齐 libbox.log）。
 *
 * 重复启动防护：`startInProgress` 标志保证同一时刻只有一个 startBox 序列；
 * 重启（服务存活时再次收到 start）在 startBox 内先 `Mihomocore.stop()` 关闭
 * 旧核心再启动新实例。
 */
class MihomoVpnService : VpnService() {

  companion object {
    private const val TAG = "MihomoVpnService"
    private const val CHANNEL_ID = "vpn"
    private const val NOTIFICATION_ID = 1

    /** Intent extra：mihomo YAML 配置文本。 */
    const val EXTRA_CONFIG = "config"

    /**
     * 显式停止 action：由 [VpnPlugin] 发送。onStartCommand 收到后有序关闭
     * 核心（后台线程）→ 退前台 → 自停 → 复位 running；裸 stopService 只走
     * onDestroy，close 仍阻塞主线程且不记录 lastError。
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
     * writeLog 环形缓冲（最近 200 行）：启动失败时附带进 lastError，让前端
     * Alert 直接看到 mihomo 侧错误链（如配置解析 / TUN 启动失败）。
     *
     * 每行以 `[RFC3339] ` 时间戳前缀开头（[writeLog] 统一写入，与
     * `logs/mihomo.log` 同源），文件超限截断时重建内容仍为可解析的完整行。
     */
    private val mihomoLogBuffer = ArrayDeque<String>(200)

    /** `logs/mihomo.log` 大小上限（字节），超限时删除重建写入最新缓冲。 */
    private const val MIHOMO_LOG_MAX_BYTES = 1024 * 1024

    /**
     * RFC3339 本地时间格式（毫秒精度，如 `2026-08-02T22:12:30.123+08:00`），
     * 与 ProxyVpnService / Rust 日志页 `LogEntry.ts` 对齐。
     */
    private val LOG_TS_FORMATTER: DateTimeFormatter =
      DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss.SSSXXX")

    /**
     * 生命周期锁：串行化 startBox/stopBox 对核心与 tunFd 的访问（跨实例共享，
     * 重启时旧实例的迟到 stop 与新实例的 start 并发，锁保证互斥）。
     */
    private val lifecycleLock = Any()

    /** 启动序列进行中：防重复 start 并发启动两个核心/TUN。 */
    @Volatile
    private var startInProgress = false

    /**
     * 当前 tun pfd，进程级共享（跨实例）：stopBox 关闭原始 fd（核心持有的
     * dup fd 由 `Mihomocore.stop()` 关闭）。所有访问必须在 [lifecycleLock]
     * 临界区内进行。
     */
    private var tunFd: ParcelFileDescriptor? = null

    /** setup 时提交的回调（Go 侧持有引用，Kotlin 侧留存防止被回收）。 */
    @Volatile
    private var coreCallback: Callback? = null
  }

  /** 停止请求标记：startBox 在后台线程启动期间到达 stop 时，启动完成后立即有序关闭。 */
  @Volatile
  private var stopRequested = false

  private val mainHandler = Handler(Looper.getMainLooper())

  /** mihomo 回调实现：protect 转交 VpnService，writeLog 进环形缓冲 + 落盘。 */
  private val callback =
    object : Callback {
      override fun protect(fd: Long): Boolean = this@MihomoVpnService.protect(fd.toInt())

      override fun writeLog(level: Long, msg: String) {
        val line = "[${timestampNow()}] $msg"
        Log.i(TAG, msg)
        appendMihomoLog(line)
        appendToMihomoLogFile(line)
      }
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

    // 重复 start 防护：同一时刻只允许一个 startBox 序列，避免两个核心/TUN
    // 并发 setup/establish。
    if (startInProgress) {
      Log.w(TAG, "start ignored: another start sequence is in progress")
      return Service.START_NOT_STICKY
    }

    // 新的启动序列开始，清除上一轮停止请求标记。
    stopRequested = false

    // setup/startTun 为阻塞调用，放到后台线程执行（对齐 ProxyVpnService：
    // onStartCommand 立即返回，start 在 IO 后台线程异步执行）。
    startInProgress = true
    Thread {
      try {
        startBox(config)
        if (stopRequested) {
          // 停止请求在启动期间到达：启动已完成但立即有序关闭，不置 running。
          Log.i(TAG, "stop requested during startup, closing core")
          stopBox()
        } else {
          running = true
          lastError = null
          Log.i(TAG, "mihomo core started")
        }
      } catch (e: Exception) {
        Log.e(TAG, "failed to start mihomo core", e)
        running = false
        // 附带完整异常链 + mihomo 最近日志，让前端 Alert 直接看到错误根因。
        lastError = withMihomoLogTail(buildExceptionChain(e))
        stopSelf()
      } finally {
        startInProgress = false
      }
    }.start()

    return Service.START_NOT_STICKY
  }

  /**
   * 显式停止：在后台线程有序关闭核心（`Mihomocore.stop()` 为阻塞调用，放主
   * 线程会 ANR），完成后经主线程退前台、自停；`running` 立即复位供状态轮询读取。
   */
  private fun handleStop() {
    stopRequested = true
    running = false
    Log.i(TAG, "stop requested")
    Thread {
      try {
        stopBox()
      } catch (e: Exception) {
        Log.e(TAG, "failed to stop mihomo core", e)
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
    // 有序关闭放后台线程（stop 阻塞，避免主线程 ANR）；stopBox 幂等。
    closeCoreInBackground()
    super.onDestroy()
  }

  override fun onRevoke() {
    // VPN 授权被撤销：停止核心并自杀。
    stopRequested = true
    closeCoreInBackground()
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
        .setContentText("ProxyPanel mihomo 运行中")
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

  /**
   * 首启内置 GEO 数据复制：把 APK assets/geo/ 内的 GEO 数据三件套
   * （geoip.metadb / geosite.dat / ASN.mmdb，由
   * scripts/update-android-geodata.sh 生成）复制到 HomeDir（= filesDir），
   * 保证配置含 GEOIP/GEOSITE/ASN 规则时 mihomo 启动无需联网下载 GEO 数据
   * （首次无代理环境下载必败导致启动失败）。
   *
   * 语义：目标文件已存在则跳过不覆盖，为将来应用内更新留路（保留更新后的
   * 新数据）；复制采用流式逐块拷贝（文件 10MB+，禁止 readBytes 全量入内存）。
   * 单文件复制失败仅记 Warn 日志，不中断启动（mihomo 侧仍有下载兜底）。
   *
   * 在 [startBox] 内 `Mihomocore.setup(...)` 调用前执行。
   */
  private fun ensureGeoData() {
    val assetDir = "geo"
    val names =
      try {
        assets.list(assetDir)
      } catch (e: Exception) {
        Log.w(TAG, "failed to list assets/$assetDir: ${e.message}")
        return
      }
      ?: return
    for (name in names) {
      if (name == "VERSION") {
        // VERSION 仅为发布元数据，非 GEO 数据，跳过
        continue
      }
      val target = File(filesDir, name)
      if (target.exists()) {
        // 已存在不覆盖：保留应用内更新后的新数据
        continue
      }
      try {
        assets.open("$assetDir/$name").use { input ->
          target.outputStream().use { output ->
            input.copyTo(output, bufferSize = 64 * 1024)
          }
        }
        Log.i(TAG, "copied GEO asset $name -> ${target.absolutePath}")
      } catch (e: Exception) {
        logWarn("failed to copy GEO asset $name: ${e.message ?: e.javaClass.simpleName}")
        runCatching { target.delete() }
      }
    }
  }

  /** 写一条 Warn 级日志：Log.w + 本文件日志通道（环形缓冲 + mihomo.log，格式对齐 [callback] writeLog）。 */
  private fun logWarn(message: String) {
    Log.w(TAG, message)
    val line = "[${timestampNow()}] $message"
    appendMihomoLog(line)
    appendToMihomoLogFile(line)
  }

  /**
   * 启动序列（锁内执行，跨实例共享 lifecycleLock）：
   *   1. 先 `Mihomocore.stop()` 关闭上一轮实例残留的核心/TUN（幂等），再
   *      setup 应用新配置 —— 重启时避免两个核心并发；
   *   2. VpnService.Builder 建立 TUN（pfd 保留所有权）；
   *   3. startTun 以 pfd 的 fd 创建 sing_tun listener。
   * setup / establish / startTun 任一失败时清理已建立的 pfd 并抛异常。
   */
  private fun startBox(config: String) {
    synchronized(lifecycleLock) {
      // 先有序停旧再启新：关闭上一轮实例残留的核心与 tun fd。
      try {
        Mihomocore.stop()
      } catch (e: Exception) {
        Log.e(TAG, "failed to stop previous mihomo core", e)
      }
      coreCallback = null
      tunFd?.let { old ->
        try {
          old.close()
        } catch (e: Exception) {
          Log.e(TAG, "failed to close previous tun fd", e)
        }
      }
      tunFd = null

      if (VpnService.prepare(this@MihomoVpnService) != null) {
        throw IllegalStateException("android: missing vpn permission")
      }

      // 1. setup：设置 HomeDir、保存回调、解析并应用 mihomo YAML 配置。
      //    先确保 GEO 数据就位（配置含 GEOIP/GEOSITE/ASN 规则时启动需加载，
      //    缺失则 mihomo 尝试联网下载，首次无代理环境必败）。
      ensureGeoData()
      Mihomocore.setup(filesDir.absolutePath, config.toByteArray(Charsets.UTF_8), callback)
      coreCallback = callback

      // 2. 建立 TUN：pfd 保留所有权（不调用 detachFd），stop 时关闭原始 fd。
      //    P1 固定地址段 172.19.0.1/30（与 startTun 的 address 对应）。系统 DNS
      //    指向 tun 网关地址 172.19.0.2（与 startTun 的 dns 参数一致），DNS 包进
      //    tun 后由核心 DNSHijack 接管：若按普通流量路由，UDP 53 会命中代理规则
      //    并经不支持 UDP 转发的协议节点（如 anytls）发出导致 DNS 全灭，对齐
      //    FlClash 做法。
      val builder =
        Builder()
          .setSession(applicationInfo.loadLabel(packageManager).toString())
          .setMtu(9000)
          .addAddress("172.19.0.1", 30)
          .addRoute("0.0.0.0", 0)
          .addDnsServer("172.19.0.2")

      // 本应用必须排除在 VPN 之外：核心出站（代理连接）由本进程发起，若被 tun
      // 回环会成环导致「有 VPN 图标但流量不通」。
      try {
        builder.addDisallowedApplication(packageName)
      } catch (e: NameNotFoundException) {
        Log.w(TAG, "addDisallowedApplication(self) failed: ${e.message}")
      }

      val pfd =
        builder.establish()
          ?: throw IllegalStateException("android: 无法建立 VPN 隧道（授权可能已被系统撤销）")
      tunFd = pfd

      // 3. startTun：以 VpnService 提供的 fd 创建 sing_tun listener（stack 为
      //    "mixed"，address 与 Builder 地址段一致；dns 指向 tun 网关 172.19.0.2，
      //    wrapper 拼为 "172.19.0.2:53" 的 DNSHijack 规则，由核心 DNS 模块解析）。
      try {
        Mihomocore.startTun(pfd.fd.toLong(), "mixed", "172.19.0.1/30", "172.19.0.2")
      } catch (e: Exception) {
        // 半启动清理：startTun 失败（可能 establish 已成功）时回收 tun pfd，
        // 防 fd 泄漏。
        runCatching { tunFd?.close() }
        tunFd = null
        throw e
      }
    }
  }

  /**
   * 有序关闭核心与 TUN：复位 running → `Mihomocore.stop()`（幂等，含 StopTun
   * 与回调清理）→ 关闭 tun pfd。幂等且加锁（跨实例共享 lifecycleLock），避免
   * stop intent 与 onDestroy 并发双线程重复关闭。
   */
  private fun stopBox() {
    synchronized(lifecycleLock) {
      running = false
      try {
        Mihomocore.stop()
      } catch (e: Exception) {
        Log.e(TAG, "failed to stop mihomo core", e)
        lastError = buildExceptionChain(e)
      }
      coreCallback = null
      try {
        tunFd?.close()
      } catch (e: Exception) {
        Log.e(TAG, "failed to close tun fd", e)
        lastError = buildExceptionChain(e)
      }
      tunFd = null
    }
  }

  /** 后台线程执行的有序关闭（stop 阻塞，防主线程 ANR）。 */
  private fun closeCoreInBackground() {
    Thread {
      try {
        stopBox()
      } catch (e: Exception) {
        Log.e(TAG, "failed to stop mihomo core", e)
        lastError = buildExceptionChain(e)
      }
    }.start()
  }

  /** 追加一行 mihomo 日志到环形缓冲，超出容量弹出最旧。 */
  private fun appendMihomoLog(message: String) {
    synchronized(mihomoLogBuffer) {
      mihomoLogBuffer.addLast(message)
      while (mihomoLogBuffer.size > 200) {
        mihomoLogBuffer.removeFirst()
      }
    }
  }

  /** 取环形缓冲最近 `lines` 行（空缓冲时返回空串）。 */
  private fun mihomoLogTail(lines: Int): String {
    synchronized(mihomoLogBuffer) {
      return mihomoLogBuffer.takeLast(lines).joinToString("\n")
    }
  }

  /** 当前 RFC3339 本地时间戳（毫秒精度），与 [`LOG_TS_FORMATTER`] 对齐。 */
  private fun timestampNow(): String = LOG_TS_FORMATTER.format(OffsetDateTime.now())

  /**
   * 追加 mihomo 日志到 `filesDir/logs/mihomo.log`（与 libbox.log 同目录，供
   * 日志导出排查错误链）。超过 1MB 时截断：删除重建写入最新缓冲。
   * 任何写入异常一律静默，不得影响 VPN 主流程。
   */
  private fun appendToMihomoLogFile(message: String) {
    try {
      val logDir = File(filesDir, "logs")
      if (!logDir.isDirectory && !logDir.mkdirs()) {
        return
      }
      val logFile = File(logDir, "mihomo.log")
      if (logFile.exists() && logFile.length() > MIHOMO_LOG_MAX_BYTES) {
        logFile.delete()
        logFile.writeText(mihomoLogTail(200))
      }
      logFile.appendText(message + "\n")
    } catch (_: Exception) {
      // 静默：日志写入失败不影响 VPN 主流程。
    }
  }

  /** 失败信息附带最近 15 行 mihomo 日志，供前端 Alert 直接展示错误链。 */
  private fun withMihomoLogTail(message: String): String {
    val tail = mihomoLogTail(15)
    return if (tail.isEmpty()) message else "$message\n--- mihomo 日志 ---\n$tail"
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
}
