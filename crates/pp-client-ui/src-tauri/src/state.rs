//! Tauri 应用级共享状态。

use std::path::PathBuf;
use std::sync::Arc;

use pp_client::ClientState;
use tokio::sync::Mutex;

/// Tauri 应用共享状态。
pub struct AppState {
    /// 客户端运行状态机（未启动时为 `None`）。
    ///
    /// 以 `Arc` 持有：部分 Tauri 命令需要把状态机移入独立线程驱动
    /// （QuickJS 执行 future 非 `Send`，见 `commands::run_blocking`）。
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
    pub fn default_data_dir() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".proxy-panel-client")
    }
}
