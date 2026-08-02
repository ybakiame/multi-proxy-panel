package com.proxypanel.client

import android.app.Activity
import android.content.Intent
import android.net.VpnService
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

/** start 命令参数：sing-box JSON 配置内容。 */
@InvokeArg
class StartArgs {
  var config: String? = null
}

/**
 * Tauri 移动插件 "vpn"：暴露 prepare/start/stop/isRunning 四条命令。
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
    val config =
      runCatching { invoke.parseArgs(StartArgs::class.java).config }.getOrNull()

    if (config.isNullOrBlank()) {
      invoke.reject("missing vpn config", ERROR_MISSING_CONFIG)
      return
    }

    val prepareIntent = VpnService.prepare(activity)
    if (prepareIntent != null) {
      // 未授权：拒绝并携带固定错误码，Rust 侧透传给前端引导先「去授权」。
      invoke.reject("vpn authorization required", ERROR_NOT_AUTHORIZED)
      return
    }

    // 服务已在运行时先停掉旧实例，避免新旧 libbox 并发启动（running 由服务
    // 真实生命周期维护，此处仅读取判断）。
    if (ProxyVpnService.running) {
      activity.stopService(Intent(activity, ProxyVpnService::class.java))
    }

    val intent =
      Intent(activity, ProxyVpnService::class.java)
        .putExtra(ProxyVpnService.EXTRA_CONFIG, config)
    try {
      ContextCompat.startForegroundService(activity, intent)
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject("failed to start vpn service: ${e.message}", ERROR_START_FAILED)
    }
  }

  @Command
  fun stop(invoke: Invoke) {
    // 显式 stop intent：服务 onStartCommand 处理 ACTION_STOP 时有序关闭
    // BoxService（后台线程）→ 退前台 → 自停 → running 复位；裸 stopService 只走
    // onDestroy，close 仍阻塞主线程且不记录 lastError。
    //
    // 实例存活（含启动中：running 尚为 false）即可安全派发 ACTION_STOP——服务在
    // 前台，startForegroundService 不会重复创建，仅再次回调 onStartCommand。
    if (ProxyVpnService.instanceAlive) {
      val intent =
        Intent(activity, ProxyVpnService::class.java)
          .setAction(ProxyVpnService.ACTION_STOP)
      try {
        ContextCompat.startForegroundService(activity, intent)
      } catch (e: Exception) {
        // 服务已被系统销毁等异常情况，回退裸 stopService 兜底。
        Log.w("VpnPlugin", "startForegroundService(stop) failed, fallback stopService: ${e.message}")
        activity.stopService(intent)
      }
    } else {
      // 实例已销毁：幂等清理（无副作用）。
      activity.stopService(Intent(activity, ProxyVpnService::class.java))
    }
    invoke.resolve()
  }

  @Command
  fun isRunning(invoke: Invoke) {
    val result = JSObject()
    result.put("running", ProxyVpnService.running)
    // lastError 为 null 时键被 JSONObject 移除，Rust 侧 `#[serde(default)]` 回退 None。
    result.put("last_error", ProxyVpnService.lastError)
    invoke.resolve(result)
  }
}
