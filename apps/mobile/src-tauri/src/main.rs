//! ProxyPanel Client 移动壳入口（Tauri 2）。
//!
//! Android/iOS 构建不使用本入口：移动端由 `lib.rs` 的 [`tauri::mobile_entry_point`]
//! 注入 JNI 入口（`main` 不参与编译）；host（桌面）构建时由本文件调用 `run()`。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pp_client_mobile_ui_lib::run()
}
