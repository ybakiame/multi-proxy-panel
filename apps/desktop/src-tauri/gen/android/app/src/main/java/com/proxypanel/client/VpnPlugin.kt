package com.proxypanel.client

import android.app.Activity
import android.content.ContentValues
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.util.Log
import androidx.activity.result.ActivityResult
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileOutputStream
import java.io.OutputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/** start command args: core config content (sing-box JSON / mihomo YAML), core type, and notification prefs. */
@InvokeArg
class StartArgs {
  var config: String? = null

  /** Core type: `mihomo` -> [MihomoVpnService], default/other -> [ProxyVpnService]. */
  var core: String? = null

  /** Whether to show traffic in the VPN notification. */
  var showTraffic: Boolean = true

  /** Whether to show current proxy group & node in the VPN notification. */
  var showSelection: Boolean = true
}

/** updateNotifyPrefs command args. */
@InvokeArg
class NotifyPrefsArgs {
  var showTraffic: Boolean = true
  var showSelection: Boolean = true
}

/**
 * Tauri mobile plugin "vpn": exposes prepare/start/stop/isRunning/exportLogs/openLogsDir/updateNotifyPrefs commands.
 *
 * Plugin registration is done by Rust side (tauri::plugin::PluginApi::register_android_plugin).
 * prepare command navigates to [VpnService.prepare] authorization page;
 * authorization result is handled by [prepareCallback] (@ActivityCallback) after user returns.
 */
@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {

  companion object {
    /** User needs to authorize VPN (Rust side forwards as `vpn_not_authorized` prefix error). */
    const val ERROR_NOT_AUTHORIZED = "vpn_not_authorized"
    /** Missing config parameter. */
    const val ERROR_MISSING_CONFIG = "vpn_missing_config"
    /** Failed to start service. */
    const val ERROR_START_FAILED = "vpn_start_failed"

    /** Shared notification preferences (updated by updateNotifyPrefs, read by services). */
    @Volatile
    var notifyShowTraffic: Boolean = true

    @Volatile
    var notifyShowSelection: Boolean = true
  }

  /**
   * 请求系统 VPN 授权：`VpnService.prepare` 返回非空 intent 时发起
   * Activity 跳转（结果由 [prepareCallback] 处理）；已授权时直接 resolve。
   */
  @Command
  fun prepare(invoke: Invoke) {
    val prepareIntent = VpnService.prepare(activity)
    if (prepareIntent == null) {
      // 已授权，无需跳转。
      invoke.resolve()
      return
    }
    startActivityForResult(invoke, prepareIntent, "prepareCallback")
  }

  /** prepare 的 Activity 结果回调：RESULT_OK = 用户允许创建 VPN。 */
  @ActivityCallback
  fun prepareCallback(invoke: Invoke, result: ActivityResult) {
    if (result.resultCode == Activity.RESULT_OK) {
      invoke.resolve()
    } else {
      invoke.reject("user denied vpn authorization", ERROR_NOT_AUTHORIZED)
    }
  }

  @Command
  fun start(invoke: Invoke) {
    val args = runCatching { invoke.parseArgs(StartArgs::class.java) }.getOrNull()
    val config = args?.config

    if (config.isNullOrBlank()) {
      invoke.reject("missing vpn config", ERROR_MISSING_CONFIG)
      return
    }

    // Core dispatch: `core == "mihomo"` -> MihomoVpnService, default/other -> ProxyVpnService.
    val useMihomo = args.core == "mihomo"
    val serviceClass: Class<*> =
      if (useMihomo) MihomoVpnService::class.java else ProxyVpnService::class.java
    val extraConfig =
      if (useMihomo) MihomoVpnService.EXTRA_CONFIG else ProxyVpnService.EXTRA_CONFIG
    val extraShowTraffic =
      if (useMihomo) MihomoVpnService.EXTRA_SHOW_TRAFFIC else ProxyVpnService.EXTRA_SHOW_TRAFFIC
    val extraShowSelection =
      if (useMihomo) MihomoVpnService.EXTRA_SHOW_SELECTION else ProxyVpnService.EXTRA_SHOW_SELECTION

    val prepareIntent = VpnService.prepare(activity)
    if (prepareIntent != null) {
      // Not authorized: reject with fixed error code, Rust side forwards to frontend for guidance.
      invoke.reject("vpn authorization required", ERROR_NOT_AUTHORIZED)
      return
    }

    // Mutual exclusion + restart: stop any running service before starting new one
    // to avoid concurrent core startup.
    if (ProxyVpnService.running) {
      activity.stopService(Intent(activity, ProxyVpnService::class.java))
    }
    if (MihomoVpnService.running) {
      activity.stopService(Intent(activity, MihomoVpnService::class.java))
    }

    // Persist notification prefs from start args (for initial launch).
    notifyShowTraffic = args.showTraffic
    notifyShowSelection = args.showSelection

    val intent = Intent(activity, serviceClass)
      .putExtra(extraConfig, config)
      .putExtra(extraShowTraffic, args.showTraffic)
      .putExtra(extraShowSelection, args.showSelection)
    try {
      ContextCompat.startForegroundService(activity, intent)
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject("failed to start vpn service: ${e.message}", ERROR_START_FAILED)
    }
  }

  @Command
  fun stop(invoke: Invoke) {
    // 两个服务都处理：谁在运行停谁（幂等——实例已销毁时 stopService 无副作用）。
    stopServiceIfAlive(
      ProxyVpnService::class.java,
      ProxyVpnService.instanceAlive,
      ProxyVpnService.ACTION_STOP,
    )
    stopServiceIfAlive(
      MihomoVpnService::class.java,
      MihomoVpnService.instanceAlive,
      MihomoVpnService.ACTION_STOP,
    )
    invoke.resolve()
  }

  /**
   * 对指定 VPN 服务派发显式 stop：实例存活（含启动中）时发 ACTION_STOP intent，
   * 服务 onStartCommand 处理时有序关闭核心（后台线程）→ 退前台 → 自停 →
   * running 复位；裸 stopService 只走 onDestroy，close 仍阻塞主线程且不记录
   * lastError。实例已销毁时回退裸 stopService 兜底幂等清理。
   */
  private fun stopServiceIfAlive(serviceClass: Class<*>, alive: Boolean, stopAction: String) {
    if (alive) {
      val intent = Intent(activity, serviceClass).setAction(stopAction)
      try {
        ContextCompat.startForegroundService(activity, intent)
      } catch (e: Exception) {
        // 服务已被系统销毁等异常情况，回退裸 stopService 兜底。
        Log.w("VpnPlugin", "startForegroundService(stop) failed, fallback stopService: ${e.message}")
        activity.stopService(intent)
      }
    } else {
      activity.stopService(Intent(activity, serviceClass))
    }
  }

  @Command
  fun updateNotifyPrefs(invoke: Invoke) {
    val args = runCatching { invoke.parseArgs(NotifyPrefsArgs::class.java) }.getOrNull()
    if (args == null) {
      invoke.reject("missing notify prefs args")
      return
    }
    notifyShowTraffic = args.showTraffic
    notifyShowSelection = args.showSelection
    // Forward to running service if alive.
    if (ProxyVpnService.instanceAlive) {
      val intent = Intent(activity, ProxyVpnService::class.java)
        .setAction(ProxyVpnService.ACTION_UPDATE_PREFS)
        .putExtra(ProxyVpnService.EXTRA_SHOW_TRAFFIC, args.showTraffic)
        .putExtra(ProxyVpnService.EXTRA_SHOW_SELECTION, args.showSelection)
      try {
        ContextCompat.startForegroundService(activity, intent)
      } catch (e: Exception) {
        Log.w("VpnPlugin", "forward prefs to ProxyVpnService failed: ${e.message}")
      }
    }
    if (MihomoVpnService.instanceAlive) {
      val intent = Intent(activity, MihomoVpnService::class.java)
        .setAction(MihomoVpnService.ACTION_UPDATE_PREFS)
        .putExtra(MihomoVpnService.EXTRA_SHOW_TRAFFIC, args.showTraffic)
        .putExtra(MihomoVpnService.EXTRA_SHOW_SELECTION, args.showSelection)
      try {
        ContextCompat.startForegroundService(activity, intent)
      } catch (e: Exception) {
        Log.w("VpnPlugin", "forward prefs to MihomoVpnService failed: ${e.message}")
      }
    }
    invoke.resolve()
  }

  @Command
  fun isRunning(invoke: Invoke) {
    val result = JSObject()
    result.put("running", ProxyVpnService.running || MihomoVpnService.running)
    // last_error: prioritize the side that is not running and has an error (most recent failure);
    // when neither has an error, take the non-null one, in reverse order of
    // ProxyVpnService.lastError ?: MihomoVpnService.lastError (mihomo优先).
    // When null, the key is removed by JSONObject, and Rust side `#[serde(default)]` falls back to None.
    val lastError =
      when {
        !MihomoVpnService.running && MihomoVpnService.lastError != null ->
          MihomoVpnService.lastError
        !ProxyVpnService.running && ProxyVpnService.lastError != null ->
          ProxyVpnService.lastError
        else -> MihomoVpnService.lastError ?: ProxyVpnService.lastError
      }
    result.put("last_error", lastError)
    invoke.resolve(result)
  }

  /**
   * 把 `filesDir/logs/` 下 `.log` 文件、`app.log.*` 滚动文件及启动前脱敏落盘的
   * `last_start_config.json` / `last_start_config.yaml`（最终核心配置脱敏快照，
   * uuid/password/server 已打码，见 Rust 侧 start_services；mihomo 为 YAML、
   * sing-box 为 JSON）打包 zip 导出到公共 `Download/ProxyPanel/`：
   * - API 29+：经 [MediaStore.Downloads] 插入（`RELATIVE_PATH=Download/ProxyPanel/`，
   *   无需任何权限）；
   * - API 26-28：旧式直接写公共下载目录（依赖 manifest 中
   *   WRITE_EXTERNAL_STORAGE，maxSdkVersion 28）。
   *
   * 写入成功 resolve `Download/ProxyPanel/<文件名>` 展示路径；无日志文件时
   * reject「暂无可导出的日志」。任何失败 reject 具体原因（异常不抛给调用方）。
   */
  @Command
  fun exportLogs(invoke: Invoke) {
    try {
      val logFiles = collectLogFiles()
      if (logFiles.isEmpty()) {
        invoke.reject("暂无可导出的日志")
        return
      }
      val fileName = "ProxyPanel-logs-${timestampFileName()}.zip"
      val displayPath =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
          exportViaMediaStore(logFiles, fileName)
        } else {
          exportToLegacyPublicDir(logFiles, fileName)
        }
      invoke.resolveObject(displayPath)
    } catch (e: Exception) {
      Log.e("VpnPlugin", "exportLogs failed", e)
      invoke.reject("导出日志失败: ${e.message ?: e.javaClass.simpleName}")
    }
  }

  /**
   * 收集 `filesDir/logs/` 下 `.log` 文件、`app.log.*` 滚动文件及启动前脱敏落盘的
   * `last_start_config.json` / `last_start_config.yaml`（目录不存在 / 无日志时
   * 返回空数组）。
   */
  private fun collectLogFiles(): Array<File> {
    val logsDir = File(activity.filesDir, "logs")
    return logsDir.listFiles { f ->
      f.isFile &&
        (f.name.endsWith(".log") ||
          f.name.startsWith("app.log") ||
          f.name == "last_start_config.json" ||
          f.name == "last_start_config.yaml")
    } ?: emptyArray()
  }

  /** 导出文件名的分钟级时间戳（`yyyyMMdd-HHmmss`）。 */
  private fun timestampFileName(): String =
    SimpleDateFormat("yyyyMMdd-HHmmss", Locale.US).format(Date())

  /** API 29+：经 [MediaStore.Downloads] 写入 `Download/ProxyPanel/`，返回展示路径。 */
  private fun exportViaMediaStore(logFiles: Array<File>, fileName: String): String {
    val values =
      ContentValues().apply {
        put(MediaStore.MediaColumns.DISPLAY_NAME, fileName)
        put(MediaStore.MediaColumns.MIME_TYPE, "application/zip")
        put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/ProxyPanel/")
      }
    val resolver = activity.contentResolver
    val uri =
      resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
        ?: throw IllegalStateException("MediaStore 插入失败")
    try {
      resolver.openOutputStream(uri)?.use { writeZip(logFiles, it) }
        ?: throw IllegalStateException("无法打开 MediaStore 输出流")
    } catch (e: Exception) {
      // 写入失败时清理半成品 MediaStore 条目。
      resolver.delete(uri, null, null)
      throw e
    }
    return "Download/ProxyPanel/$fileName"
  }

  /**
   * API 26-28：旧式直接写公共下载目录 `Download/ProxyPanel/`。
   * `getExternalStoragePublicDirectory` 已废弃，用 `@Suppress` 抑制告警；
   * 写入依赖 manifest 中的 WRITE_EXTERNAL_STORAGE（maxSdkVersion 28）。
   */
  @Suppress("DEPRECATION")
  private fun exportToLegacyPublicDir(logFiles: Array<File>, fileName: String): String {
    val downloadDir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
    val panelDir = File(downloadDir, "ProxyPanel")
    if (!panelDir.isDirectory && !panelDir.mkdirs()) {
      throw IllegalStateException("无法创建导出目录 ${panelDir.absolutePath}")
    }
    val target = File(panelDir, fileName)
    FileOutputStream(target).use { writeZip(logFiles, it) }
    return "Download/ProxyPanel/$fileName"
  }

  /** 把多个日志文件平铺写入 zip（ZipEntry 仅取文件名，不打目录前缀）。 */
  private fun writeZip(logFiles: Array<File>, output: OutputStream) {
    ZipOutputStream(output.buffered()).use { zip ->
      for (file in logFiles) {
        zip.putNextEntry(ZipEntry(file.name))
        file.inputStream().use { it.copyTo(zip) }
        zip.closeEntry()
      }
    }
  }

  /**
   * 打开系统「下载」目录：导出日志 zip 写入公共 `Download/ProxyPanel/` 后，经
   * `DownloadManager.ACTION_VIEW_DOWNLOADS` 直接打开下载目录方便用户取用。
   * 打开失败（无对应 Activity 等）时 reject 具体原因，异常不抛给调用方。
   */
  @Command
  fun openLogsDir(invoke: Invoke) {
    try {
      activity.startActivity(Intent(android.app.DownloadManager.ACTION_VIEW_DOWNLOADS))
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject("无法打开下载目录: ${e.message ?: e.javaClass.simpleName}")
    }
  }
}
