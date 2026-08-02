package com.proxypanel.client

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import androidx.core.content.ContextCompat
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
 * Tauri 移动插件 "vpn"：暴露 start/stop/isRunning 三条命令。
 *
 * 插件注册由 Rust 侧（tauri::plugin::PluginApi::register_android_plugin）
 * 完成——本步 Rust 桥接尚未实现，仅保证 Kotlin 侧类可编译、
 * 构造签名（Activity）符合 tauri 2.11 反射实例化要求。
 *
 * 授权流程（VpnService.prepare 的 Activity 跳转）属 P1c-2，本步在需要
 * 授权时以固定错误码拒绝，由前端据此提示。
 */
@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {

  companion object {
    /** 需要用户授权 VPN（下一步跳转授权页）。 */
    const val ERROR_NOT_AUTHORIZED = "vpn_not_authorized"
    /** 缺少 config 参数。 */
    const val ERROR_MISSING_CONFIG = "vpn_missing_config"
    /** 启动服务失败。 */
    const val ERROR_START_FAILED = "vpn_start_failed"
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
      // 授权流程（P1c-2）：以 startActivityForResult 发起 prepare intent。
      invoke.reject("vpn authorization required", ERROR_NOT_AUTHORIZED)
      return
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
    activity.stopService(Intent(activity, ProxyVpnService::class.java))
    invoke.resolve()
  }

  @Command
  fun isRunning(invoke: Invoke) {
    val result = JSObject()
    result.put("running", ProxyVpnService.isRunning)
    invoke.resolve(result)
  }
}
