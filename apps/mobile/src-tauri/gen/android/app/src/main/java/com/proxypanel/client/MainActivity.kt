package com.proxypanel.client

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    requestNotificationPermissionIfNeeded()
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
