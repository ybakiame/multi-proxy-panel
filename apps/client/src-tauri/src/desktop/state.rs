//! Tauri 应用级共享状态（薄层）。

use std::path::PathBuf;

pub use pp_client_tauri::state::AppState;

/// 默认数据目录：用户主目录下的 `.proxy-panel-client`。
///
/// 主目录经 [`dirs::home_dir`] 解析——Windows 上 `HOME` 环境变量通常不存在
/// （此前直接读 `HOME`，未设置时回退为相对路径 `.`，数据目录落到安装目录
/// 或启动目录下，且核心子进程因 `-D` 切换工作目录后读不到相对路径配置）。
/// 保底回退为进程当前目录的**绝对**路径，本函数绝不返回相对路径。
pub fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".proxy-panel-client")
}
