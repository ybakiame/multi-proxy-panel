//! ProxyPanel Client 桌面壳入口（Tauri 2）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pp_client_ui_lib::run()
}
