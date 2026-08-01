//! ScriptWorker actor：把 `!Send` 的 QuickJS 执行统一收敛到单一 OS 线程。
//!
//! rquickjs 的 `AsyncRuntime` 内部含非 `Send` 结构，`QuickJsEngine::run_script`
//! 的 future 无法跨线程 `.await`。历史上调用方被迫
//! `spawn_blocking` + 新建 `current_thread` runtime + `block_on` 绕行
//! （pp-mitm 脚本钩子、pp-script 调度器后台循环、Tauri `run_task`）。
//!
//! 本模块把这三处绕行收敛为一个 actor：专有 OS 线程 + 专用 `current_thread`
//! 运行时，任务经 mpsc 串行化执行，对外暴露 `Send` future。
//! 多个调用方可并发调用 `run_script`（内部串行化）；克隆共享同一 worker。

use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use crate::engine::ScriptEngine;
use crate::engine_quickjs::QuickJsEngine;
use crate::host::ScriptHost;
use crate::types::{ScriptDialect, ScriptKind, ScriptLimits, ScriptOutput};
use pp_common::{PanelError, PanelResult};

/// 一条待执行脚本作业。
struct Job {
    source: String,
    kind: ScriptKind,
    arg: Option<serde_json::Value>,
    dialect: ScriptDialect,
    script_name: String,
    respond: oneshot::Sender<PanelResult<ScriptOutput>>,
}

/// 脚本执行 actor：专有 OS 线程 + 专用 current_thread tokio runtime。
///
/// `run_script` 返回 `Send` future：任务经 mpsc 发送到 worker 线程串行执行，
/// 结果经 oneshot 回传。worker 线程崩溃后（Rust panic）所有在途与后续调用
/// 快速返回 [`PanelError::Script`]（"worker died"），不会挂死。
#[derive(Clone)]
pub struct ScriptWorker {
    tx: mpsc::Sender<Job>,
    close_tx: mpsc::Sender<()>,
    handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl ScriptWorker {
    /// 生成一个 worker：`thread::spawn` → 建 `current_thread` tokio runtime →
    /// `block_on` 消息循环（收 Job → 新建 `QuickJsEngine` → `run_script` →
    /// `respond.send`）。
    pub fn new(host: Arc<ScriptHost>, limits: ScriptLimits) -> Self {
        let (tx, mut rx) = mpsc::channel::<Job>(64);
        let (close_tx, mut close_rx) = mpsc::channel::<()>(1);
        let (handle, tx, close_tx) =
            match std::thread::Builder::new()
                .name("script-worker".to_string())
                .spawn(move || {
                    let rt = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt,
                        Err(e) => {
                            tracing::error!(error = %e, "script worker: build current_thread runtime failed");
                            return;
                        }
                    };
                    // 线程级 panic 隔离：job 中的 Rust panic 会终止本线程
                    // （mpsc 收端与未决 respond 随之 drop），调用方收到
                    // "worker died" 而非挂死。
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        rt.block_on(async move {
                            loop {
                                tokio::select! {
                                    maybe_job = rx.recv() => {
                                        let Some(Job {
                                            source,
                                            kind,
                                            arg,
                                            dialect,
                                            script_name,
                                            respond,
                                        }) = maybe_job
                                        else {
                                            break;
                                        };
                                        let result = execute(
                                            &host,
                                            limits,
                                            &source,
                                            kind,
                                            arg,
                                            dialect,
                                            &script_name,
                                        )
                                        .await;
                                        let _ = respond.send(result);
                                    }
                                    close = close_rx.recv() => {
                                        if close.is_some() {
                                            break;
                                        }
                                    }
                                }
                            }
                        });
                    }));
                })
            {
                Ok(h) => (Some(h), tx, close_tx),
                Err(e) => {
                    // 线程未启动：收端被 drop，worker 处于死亡态，
                    // run_script 立即返回 "worker died"。
                    tracing::error!(error = %e, "script worker: spawn thread failed");
                    (None, tx, close_tx)
                }
            };
        Self {
            tx,
            close_tx,
            handle: Arc::new(Mutex::new(handle)),
        }
    }

    /// 执行一段脚本（`Send` future）。
    ///
    /// 参数含义与 [`ScriptEngine::run_script`] 一致，另注入 `dialect` 与
    /// `script_name`（与 [`QuickJsEngine::new`] 的构造签名对齐）。
    pub async fn run_script(
        &self,
        source: &str,
        kind: ScriptKind,
        arg: Option<serde_json::Value>,
        dialect: ScriptDialect,
        script_name: &str,
    ) -> PanelResult<ScriptOutput> {
        let (respond, rx) = oneshot::channel();
        let job = Job {
            source: source.to_string(),
            kind,
            arg,
            dialect,
            script_name: script_name.to_string(),
            respond,
        };
        self.tx
            .send(job)
            .await
            .map_err(|_| PanelError::Script("worker died".into()))?;
        rx.await
            .map_err(|_| PanelError::Script("worker died".into()))?
    }

    /// 关闭 worker：发送关闭消息并 join 线程。
    ///
    /// 已有克隆时任一克隆调用都会关闭底层线程；重复调用安全。
    pub async fn shutdown(&self) {
        let _ = self.close_tx.send(()).await;
        let handle = {
            let mut guard = self.handle.lock().unwrap_or_else(|e| e.into_inner());
            guard.take()
        };
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

/// 在 worker 线程内执行单个 job：新建引擎 + `run_script`。
async fn execute(
    host: &Arc<ScriptHost>,
    limits: ScriptLimits,
    source: &str,
    kind: ScriptKind,
    arg: Option<serde_json::Value>,
    dialect: ScriptDialect,
    script_name: &str,
) -> PanelResult<ScriptOutput> {
    let mut engine =
        QuickJsEngine::new(Arc::clone(host), dialect, limits, script_name.to_string())?;
    engine.run_script(source, kind, arg).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{MemoryPersistentStore, MockHttpExecutor, RecordingNotifier};
    use std::time::Duration;

    fn test_host() -> Arc<ScriptHost> {
        let http = Arc::new(MockHttpExecutor::with_responses(vec![]));
        let store = Arc::new(MemoryPersistentStore::new());
        let notifier = Arc::new(RecordingNotifier::new());
        Arc::new(ScriptHost::new(http, store, notifier))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_run_script_from_multiple_tasks() {
        // 多 tokio task 并发调用（multi_thread runtime）：全部正确返回，
        // 证明 run_script 的 future 为 Send 且 worker 内部串行化正确。
        let worker = ScriptWorker::new(test_host(), ScriptLimits::default());
        let mut handles = Vec::new();
        for i in 0..8u32 {
            let worker = worker.clone();
            handles.push(tokio::spawn(async move {
                let source = format!("$done({{i: {i}}});");
                let out = worker
                    .run_script(
                        &source,
                        ScriptKind::Generic,
                        None,
                        ScriptDialect::QuantumultX,
                        "concurrent",
                    )
                    .await
                    .expect("concurrent run_script should succeed");
                assert_eq!(out.0["i"], serde_json::json!(i));
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        worker.shutdown().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn thrown_error_propagates_and_worker_survives() {
        let worker = ScriptWorker::new(test_host(), ScriptLimits::default());
        let err = worker
            .run_script(
                "throw new Error('boom');",
                ScriptKind::Generic,
                None,
                ScriptDialect::QuantumultX,
                "err1",
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, PanelError::Script(_)),
            "expected Script error, got {err:?}"
        );
        // worker 存活：后续调用仍正常。
        let out = worker
            .run_script(
                "$done({ok: true});",
                ScriptKind::Generic,
                None,
                ScriptDialect::QuantumultX,
                "ok2",
            )
            .await
            .expect("worker should survive a script error");
        assert_eq!(out.0["ok"], true);
        worker.shutdown().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shutdown_then_run_script_errors_quickly() {
        let worker = ScriptWorker::new(test_host(), ScriptLimits::default());
        worker.shutdown().await;
        let bounded = tokio::time::timeout(
            Duration::from_secs(2),
            worker.run_script(
                "$done();",
                ScriptKind::Generic,
                None,
                ScriptDialect::QuantumultX,
                "after",
            ),
        )
        .await
        .expect("run_script after shutdown should not hang");
        let err = bounded.unwrap_err();
        assert!(
            matches!(err, PanelError::Script(_)),
            "expected Script error, got {err:?}"
        );
    }
}
