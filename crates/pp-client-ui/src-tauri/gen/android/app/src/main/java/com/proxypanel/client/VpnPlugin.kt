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

/** start 命令参数：核心配置内容（sing-box JSON / mihomo YAML）与核心类型。 */
@InvokeArg
class StartArgs {
  var config: String? = null

  /** 核心类型：`mihomo` 时走 [MihomoVpnService]，默认/其他值走 [ProxyVpnService]。 */
  var core: String? = null
}

/**
 * Tauri 移动插件 "vpn"：暴露 prepare/start/stop/isRunning/exportLogs 五条命令。
 *
 * 插件注册由 Rust 侧（tauri::plugin::PluginApi::register_android_plugin）完成。
 * prepare 命令经 [VpnService.prepare] 的 Activity 跳转完成系统 VPN 授权，
 * 授权结果由 [prepareCallback]（@ActivityCallback）在用户返回后 resolve/reject。
 */
@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {

  companion object {
    /** 需要用户授权 VPN（Rust 侧透传为 `vpn_not_authorized` 前缀错误）。 */
    const val ERROR_NOT_AUTHORIZED = "vpn_not_authorized"
    /** 缺少 config 参数。 */
    const val ERROR_MISSING_CONFIG = "vpn_missing_config"
    /** 启动服务失败。 */
    const val ERROR_START_FAILED = "vpn_start_failed"
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

    // 核心分派：`core == "mihomo"` 走 MihomoVpnService，默认/其他值走
    // ProxyVpnService（sing-box）。（config 非空校验后 args 已 smart-cast）
    val useMihomo = args.core == "mihomo"
    val serviceClass: Class<*> =
      if (useMihomo) MihomoVpnService::class.java else ProxyVpnService::class.java
    val extraConfig =
      if (useMihomo) MihomoVpnService.EXTRA_CONFIG else ProxyVpnService.EXTRA_CONFIG

    val prepareIntent = VpnService.prepare(activity)
    if (prepareIntent != null) {
      // 未授权：拒绝并携带固定错误码，Rust 侧透传给前端引导先「去授权」。
      invoke.reject("vpn authorization required", ERROR_NOT_AUTHORIZED)
      return
    }

    // 两服务互斥 + 重启：启动前先停掉已运行的服务（自身重启 / 切换核心），
    // 避免两个核心并发启动（running 由服务真实生命周期维护，此处仅读取判断）。
    if (ProxyVpnService.running) {
      activity.stopService(Intent(activity, ProxyVpnService::class.java))
    }
    if (MihomoVpnService.running) {
      activity.stopService(Intent(activity, MihomoVpnService::class.java))
    }

    val intent = Intent(activity, serviceClass).putExtra(extraConfig, config)
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
  fun isRunning(invoke: Invoke) {
    val result = JSObject()
    result.put("running", ProxyVpnService.running || MihomoVpnService.running)
    // last_error：以「当前非 running 且有错误的一方」为准（最近失败方优先）；
    // 两者都无错误时取非空者，顺序与 ProxyVpnService.lastError ?:
    // MihomoVpnService.lastError 反向（mihomo 优先）——P1 阶段 mihomo 是新接入
    // 核心，其错误更值得展示。为 null 时键被 JSONObject 移除，Rust 侧
    // `#[serde(default)]` 回退 None。
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
   * `last_start_config.json`（最终核心配置脱敏快照，uuid/password/server 已打码，
   * 见 Rust 侧 start_services）打包 zip 导出到公共 `Download/ProxyPanel/`：
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
   * `last_start_config.json`（目录不存在 / 无日志时返回空数组）。
   */
  private fun collectLogFiles(): Array<File> {
    val logsDir = File(activity.filesDir, "logs")
    return logsDir.listFiles { f ->
      f.isFile && (f.name.endsWith(".log") || f.name.startsWith("app.log") || f.name == "last_start_config.json")
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
}
