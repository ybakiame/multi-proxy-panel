//! Tauri 应用级共享状态。

use std::path::PathBuf;
use std::sync::Arc;

use pp_client::ClientState;
use pp_client::StatsStore;
use tokio::sync::Mutex;
use tracing_appender::non_blocking::WorkerGuard;

/// Tauri 应用共享状态。
///
/// `data_dir` 由壳层解析后注入（桌面壳沿用 `default_data_dir`，移动壳用 Tauri path
/// resolver 解析应用私有目录，见 ADR-0003 §2.1 参数化原则），本类型不做平台分支。
pub struct AppState {
    /// 客户端运行状态机（未启动时为 `None`）。
    ///
    /// 以 `Arc` 持有：多 Tauri 命令共享同一状态机；`ClientState::start` 的
    /// future 为 `Send`（JS 复写经 pp-script `ScriptWorker` 驱动），可直接在
    /// Tauri 命令中 `await`。
    pub client: Arc<Mutex<Option<ClientState>>>,
    /// 数据目录（配置、证书、核心二进制统一存放于此）。
    pub data_dir: PathBuf,
    /// 流量统计存储（懒打开；打开失败时降级为 `None`，统计命令报错、tracker 仅内存态）。
    stats: tokio::sync::OnceCell<Option<Arc<StatsStore>>>,
    /// 日志非阻塞写入线程的保活 guard。
    ///
    /// 被 Drop 后滚动文件停止接收日志（见 [`crate::logs::init_logging`]），因此必须随
    /// `AppState` 存活到进程退出。
    _log_guard: WorkerGuard,
}

impl AppState {
    /// 构造应用状态。
    pub fn new(data_dir: PathBuf, log_guard: WorkerGuard) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            data_dir,
            stats: tokio::sync::OnceCell::new(),
            _log_guard: log_guard,
        }
    }

    /// 获取（首次调用时打开）流量统计存储；打开失败返回 `None` 并记录告警。
    pub async fn stats_store(&self) -> Option<Arc<StatsStore>> {
        self.stats
            .get_or_init(|| async {
                match StatsStore::open(&self.data_dir).await {
                    Ok(store) => Some(Arc::new(store)),
                    Err(e) => {
                        tracing::warn!(error = %e, "流量统计存储打开失败，统计功能降级为不可用");
                        None
                    }
                }
            })
            .await
            .clone()
    }
}
