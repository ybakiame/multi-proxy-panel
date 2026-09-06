//! Tauri 应用级共享状态（薄层）。

use std::path::PathBuf;

pub use pp_client_tauri::state::AppState;

/// 默认数据目录：`$HOME/.proxy-panel-client`（未设置 HOME 时回退当前目录）。
///
/// 仅桌面端使用；Android 上 HOME 为只读 `/`，该路径不可写，数据目录改由
/// `lib.rs` 的 `resolve_data_dir` 经 Tauri path resolver 解析应用私有目录
/// `app_data_dir()`（见 `lib.rs`），本方法在 Android 构建下不参与编译。
#[cfg(not(target_os = "android"))]
pub fn default_data_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".proxy-panel-client")
}
