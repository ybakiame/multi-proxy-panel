//! Tauri 应用级共享状态。

use std::path::PathBuf;
use std::sync::Arc;

use pp_client::ClientState;
use tokio::sync::Mutex;

/// Tauri 应用共享状态。
pub struct AppState {
    /// 客户端运行状态机（未启动时为 `None`）。
    ///
    /// 以 `Arc` 持有：多 Tauri 命令共享同一状态机；`ClientState::start` 的
    /// future 为 `Send`（JS 复写经 pp-script `ScriptWorker` 驱动），可直接在
    /// Tauri 命令中 `await`。
    pub client: Arc<Mutex<Option<ClientState>>>,
    /// 数据目录（配置、证书、核心二进制统一存放于此）。
    pub data_dir: PathBuf,
}

impl AppState {
    /// 构造应用状态。
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            data_dir,
        }
    }

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
}
