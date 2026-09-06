//! Tauri 应用级共享状态（薄层）。

use std::path::PathBuf;

pub use pp_client_tauri::state::AppState;

/// 默认数据目录：`$HOME/.proxy-panel-client`（未设置 HOME 时回退当前目录）。
pub fn default_data_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".proxy-panel-client")
}
