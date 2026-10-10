package com.proxypanel.client

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    installKeyboardInsets()
    requestNotificationPermissionIfNeeded()
  }

  /**
   * Edge-to-edge 不保证 WebView 随 IME 缩小，旧 WebView 也不会更新 visualViewport。
   * 原生内容容器负责 IME 避让；清零已处理的 IME Insets 后继续传递，避免新版
   * WebView 二次缩小，同时保留系统栏/刘海安全区。adjustNothing 避免系统重复 resize。
   */
  private fun installKeyboardInsets() {
    val content = findViewById<View>(android.R.id.content)
    val left = content.paddingLeft
    val top = content.paddingTop
    val right = content.paddingRight
    val bottom = content.paddingBottom
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val ime = WindowInsetsCompat.Type.ime()
      val keyboardHeight = if (insets.isVisible(ime)) insets.getInsets(ime).bottom else 0
      view.setPadding(left, top, right, bottom + keyboardHeight)
      WindowInsetsCompat.Builder(insets).setInsets(ime, Insets.NONE).build()
    }
    ViewCompat.requestApplyInsets(content)
  }

  /**
   * Android 13+（API 33）POST_NOTIFICATIONS 为运行时权限，minSdk 33 后首次启动
   * 必须动态申请：否则脚本通知（tauri-plugin-notification）与前台服务通知在未授权
   * 时会被系统静默丢弃。用户拒绝仅影响通知展示，不阻塞应用启动。
   */
  private fun requestNotificationPermissionIfNeeded() {
    if (
      ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) !=
        PackageManager.PERMISSION_GRANTED
    ) {
      ActivityCompat.requestPermissions(
        this,
        arrayOf(Manifest.permission.POST_NOTIFICATIONS),
        REQUEST_POST_NOTIFICATIONS,
      )
    }
  }

  private companion object {
    const val REQUEST_POST_NOTIFICATIONS = 1001
  }
}
