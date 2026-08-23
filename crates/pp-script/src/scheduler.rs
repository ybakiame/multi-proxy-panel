//! cron/event 脚本调度器：管理按 cron 表达式定时执行的脚本任务（QX/Surge/Loon 的
//! task/cron 签到类脚本），支持注册/移除、手动触发、到期批量执行与后台循环。

// tonic::Status is inherently large; these gRPC-facing helpers return it by value.
#![allow(clippy::result_large_err)]

use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::host::ScriptHost;
use crate::types::{ScriptDialect, ScriptKind, ScriptLimits, ScriptOutput};
use crate::worker::ScriptWorker;
use pp_common::{PanelError, PanelResult};

/// 一个待注册的定时脚本任务。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskScript {
    pub name: String,
    pub cron_expr: String,
    pub source: String,
    pub dialect: ScriptDialect,
    pub enabled: bool,
}

/// 任务运行时视图：任务信息 + 下次执行/上次执行/上次错误。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskScriptView {
    pub name: String,
    pub cron_expr: String,
    pub dialect: ScriptDialect,
    pub enabled: bool,
    /// 从当前时刻起的下一次执行时间。
    pub next_run: Option<DateTime<Utc>>,
    /// 上次执行时间（手动触发/到期执行均记录）。
    pub last_run: Option<DateTime<Utc>>,
    /// 上次执行的错误信息（成功为 None）。
    pub last_error: Option<String>,
}

/// 事件类脚本类型。本轮仅做类型预留，未接入触发源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptEvent {
    /// 网络变更（network-change）事件。
    NetworkChange,
}

/// 内部登记的任务：`TaskScript` + 解析后的 `Schedule` + 运行记录。
struct ScheduledTask {
    name: String,
    cron_expr: String,
    schedule: Schedule,
    source: String,
    dialect: ScriptDialect,
    enabled: bool,
    last_run: Option<DateTime<Utc>>,
    last_error: Option<String>,
}

/// cron 脚本调度器。
///
/// 任务注册时解析 cron 表达式并校验；到期判定基于“`(last_run, now]` 区间内是否有
/// 匹配时刻”，无状态可注入 `now` 便于测试。单任务执行统一走 [`ScriptWorker`]
/// （`kind = Cron`），失败仅记录在结果/`last_error`，不影响其他任务。
pub struct ScriptScheduler {
    worker: ScriptWorker,
    tasks: Mutex<Vec<ScheduledTask>>,
    shutdown: watch::Sender<bool>,
}

impl ScriptScheduler {
    pub fn new(host: Arc<ScriptHost>, limits: ScriptLimits) -> Self {
        let (shutdown, _rx) = watch::channel(false);
        let worker = ScriptWorker::new(host, limits);
        Self {
            worker,
            tasks: Mutex::new(Vec::new()),
            shutdown,
        }
    }

    /// 注册任务：校验并解析 cron 表达式；同名任务重复注册报错。
    #[allow(clippy::result_large_err)]
    pub fn add_task(&mut self, task: TaskScript) -> PanelResult<()> {
        let schedule = Schedule::from_str(&task.cron_expr).map_err(|e| {
            PanelError::Validation(format!("invalid cron expression '{}': {e}", task.cron_expr))
        })?;
        let mut tasks = self.lock_tasks();
        if tasks.iter().any(|t| t.name == task.name) {
            return Err(PanelError::Validation(format!(
                "task already registered: {}",
                task.name
            )));
        }
        tasks.push(ScheduledTask {
            name: task.name,
            cron_expr: task.cron_expr,
            schedule,
            source: task.source,
            dialect: task.dialect,
            enabled: task.enabled,
            last_run: None,
            last_error: None,
        });
        Ok(())
    }

    /// 移除任务；返回是否存在并被移除。
    pub fn remove_task(&mut self, name: &str) -> bool {
        let mut tasks = self.lock_tasks();
        let before = tasks.len();
        tasks.retain(|t| t.name != name);
        tasks.len() != before
    }

    /// 列出全部任务（含下次执行/上次执行/上次错误）。
    pub fn list_tasks(&self) -> Vec<TaskScriptView> {
        let tasks = self.lock_tasks();
        let now = Utc::now();
        tasks
            .iter()
            .map(|t| TaskScriptView {
                name: t.name.clone(),
                cron_expr: t.cron_expr.clone(),
                dialect: t.dialect,
                enabled: t.enabled,
                next_run: t.schedule.after(&now).next(),
                last_run: t.last_run,
                last_error: t.last_error.clone(),
            })
            .collect()
    }

    /// 手动触发指定任务（对应 QX 点击执行 / Surge 手动运行）。
    ///
    /// 不校验 `enabled`：手动执行与定时开关相互独立。
    pub async fn run_now(&self, name: &str) -> PanelResult<ScriptOutput> {
        let (source, dialect) = {
            let tasks = self.lock_tasks();
            let task = tasks
                .iter()
                .find(|t| t.name == name)
                .ok_or_else(|| PanelError::NotFound(format!("task not found: {name}")))?;
            (task.source.clone(), task.dialect)
        };
        let result = self.run_script_once(name, &source, dialect).await;
        self.record_run(name, &result, Utc::now()).await;
        result
    }

    /// 执行所有到期任务（`now` 可注入以便测试）。
    ///
    /// 返回 `(任务名, 执行结果)`；单任务失败不中断其他任务（异常隔离）。
    /// 从未执行的任务只要存在任一匹配时刻早于/等于 `now` 即视为到期（首次注册后补跑）；
    /// 已执行过的任务仅在 `last_run` 之后存在新匹配时刻时触发，避免轮询重复执行。
    pub async fn run_due(&self, now: DateTime<Utc>) -> Vec<(String, PanelResult<ScriptOutput>)> {
        let due: Vec<(String, String, ScriptDialect)> = {
            let tasks = self.lock_tasks();
            let mut due = Vec::new();
            for task in tasks.iter().filter(|t| t.enabled) {
                if Self::is_due(&task.schedule, now, task.last_run) {
                    due.push((task.name.clone(), task.source.clone(), task.dialect));
                }
            }
            due
        };
        let mut results = Vec::with_capacity(due.len());
        for (name, source, dialect) in due {
            let result = self.run_script_once(&name, &source, dialect).await;
            self.record_run(&name, &result, now).await;
            results.push((name, result));
        }
        results
    }

    /// 启动后台循环（公开 API：1s 轮询 tick）。返回可 join 的 handle。
    ///
    /// 循环以 tick 为上限轮询：计算最近一次未来触发时刻并等待（不超过 tick），
    /// 到期后执行全部到期任务；收到 `stop()` 信号后退出。
    pub async fn start(self: Arc<Self>) -> PanelResult<JoinHandle<()>> {
        self.start_with_interval(Duration::from_secs(1)).await
    }

    /// 内部启动：以指定轮询间隔驱动后台循环（测试可注入更短间隔）。
    ///
    /// 脚本执行由 [`ScriptWorker`] 在专有线程驱动（`Send` future），后台循环
    /// 直接在调用方运行时 `tokio::spawn`，不再需要 `spawn_blocking` +
    /// 独立 `current_thread` runtime 的绕行。
    pub(crate) async fn start_with_interval(
        self: Arc<Self>,
        tick: Duration,
    ) -> PanelResult<JoinHandle<()>> {
        let mut shutdown_rx = self.shutdown.subscribe();
        let scheduler = Arc::clone(&self);
        let handle = tokio::spawn(async move {
            loop {
                // stop 已触发（含 start 之前 stop）：立即退出。
                if *shutdown_rx.borrow() {
                    break;
                }
                let sleep_for = {
                    let tasks = scheduler.lock_tasks();
                    let now = Utc::now();
                    let soonest = tasks
                        .iter()
                        .filter(|t| t.enabled)
                        .filter_map(|t| t.schedule.after(&now).next())
                        .min();
                    match soonest {
                        Some(next) => {
                            let delta = (next - now).to_std().unwrap_or(Duration::ZERO);
                            delta.min(tick).max(Duration::from_millis(1))
                        }
                        None => tick,
                    }
                };
                tokio::select! {
                    _ = shutdown_rx.changed() => (),
                    _ = tokio::time::sleep(sleep_for) => {
                        let now = Utc::now();
                        for (name, result) in scheduler.run_due(now).await {
                            if let Err(e) = result {
                                tracing::warn!(task = %name, error = %e, "scheduled task failed");
                            }
                        }
                    }
                }
            }
        });
        Ok(handle)
    }

    /// 发送 shutdown 信号，后台循环收到后退出。
    pub async fn stop(&self) {
        let _ = self.shutdown.send(true);
        tokio::task::yield_now().await;
    }

    /// 事件分发入口（事件类脚本本轮仅做类型预留）。
    ///
    /// TODO: 接入 network-change 等事件触发源后，按 `ScriptEvent` 匹配已注册的
    /// event 脚本并执行（复用 `run_script_once`）。
    pub async fn dispatch_event(
        &self,
        event: ScriptEvent,
    ) -> Vec<(String, PanelResult<ScriptOutput>)> {
        tracing::warn!(?event, "event dispatch not implemented yet");
        Vec::new()
    }

    /// 单任务执行：经 [`ScriptWorker`] 串行执行（`kind = Cron`，超时/异常由引擎隔离）。
    async fn run_script_once(
        &self,
        name: &str,
        source: &str,
        dialect: ScriptDialect,
    ) -> PanelResult<ScriptOutput> {
        self.worker
            .run_script(source, ScriptKind::Cron, None, None, dialect, name)
            .await
    }

    /// 记录执行结果（last_run / last_error）。
    async fn record_run(&self, name: &str, result: &PanelResult<ScriptOutput>, at: DateTime<Utc>) {
        let mut tasks = self.lock_tasks();
        if let Some(task) = tasks.iter_mut().find(|t| t.name == name) {
            task.last_run = Some(at);
            match result {
                Ok(_) => task.last_error = None,
                Err(e) => task.last_error = Some(e.to_string()),
            }
        }
    }

    /// 到期判定（无副作用）：
    /// - 已执行过：`last_run` 之后存在不晚于 `now` 的匹配时刻才到期（避免重复触发）；
    /// - 从未执行：`now` 本身命中或存在早于 `now` 的匹配时刻即到期（首次补跑）。
    fn is_due(schedule: &Schedule, now: DateTime<Utc>, last_run: Option<DateTime<Utc>>) -> bool {
        match last_run {
            Some(last) => schedule.after(&last).next().is_some_and(|next| next <= now),
            None => schedule.includes(now) || schedule.after(&now).next_back().is_some(),
        }
    }

    /// 加锁获取任务表（吞掉 poison，避免一处 panic 拖垮调度器）。
    fn lock_tasks(&self) -> MutexGuard<'_, Vec<ScheduledTask>> {
        self.tasks.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{
        MemoryPersistentStore, MockHttpExecutor, PersistentStore, RecordingNotifier,
    };
    use crate::types::HttpResponseData;
    use chrono::Duration as ChronoDuration;
    use chrono::{Datelike, TimeZone};

    fn test_host() -> (
        Arc<ScriptHost>,
        Arc<RecordingNotifier>,
        Arc<MemoryPersistentStore>,
    ) {
        let http = Arc::new(MockHttpExecutor::with_responses(vec![
            HttpResponseData {
                status: 200,
                headers: vec![("content-type".into(), "application/json".into())],
                body: r#"{"code":0,"token":"tok123"}"#.into(),
            },
            HttpResponseData {
                status: 200,
                headers: vec![],
                body: r#"{"code":0}"#.into(),
            },
        ]));
        let store = Arc::new(MemoryPersistentStore::new());
        let notifier = Arc::new(RecordingNotifier::new());
        let host = Arc::new(ScriptHost::new(http, store.clone(), notifier.clone()));
        (host, notifier, store)
    }

    fn qx_task(name: &str, cron_expr: &str, source: &str) -> TaskScript {
        TaskScript {
            name: name.into(),
            cron_expr: cron_expr.into(),
            source: source.into(),
            dialect: ScriptDialect::QuantumultX,
            enabled: true,
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_task_rejects_invalid_cron() {
        let (host, _n, _s) = test_host();
        let mut scheduler = ScriptScheduler::new(host, ScriptLimits::default());

        // 非法 cron 表达式：报 Validation 错误且不注册
        let err = scheduler
            .add_task(qx_task("bad", "not a cron", "$done();"))
            .unwrap_err();
        assert!(
            matches!(err, PanelError::Validation(_)),
            "expected Validation, got {err:?}"
        );
        assert!(scheduler.list_tasks().is_empty());

        // 合法表达式注册成功
        scheduler
            .add_task(qx_task("ok", "0 * * * * *", "$done();"))
            .unwrap();
        // 同名重复注册报错
        let dup = scheduler
            .add_task(qx_task("ok", "0 0 12 * * *", "$done();"))
            .unwrap_err();
        assert!(matches!(dup, PanelError::Validation(_)));
        assert_eq!(scheduler.list_tasks().len(), 1);

        // remove_task
        assert!(scheduler.remove_task("ok"));
        assert!(!scheduler.remove_task("ok"));
        assert!(scheduler.list_tasks().is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_now_qx_signin_style() {
        let (host, notifier, store) = test_host();
        let mut scheduler = ScriptScheduler::new(host, ScriptLimits::default());
        scheduler
            .add_task(qx_task(
                "qx_signin",
                "0 * * * * *",
                r#"
                    (async () => {
                        const resp = await $task.fetch({url: "https://example.com/api/signin", method: "POST", body: "u=1"});
                        const data = JSON.parse(resp.body);
                        $prefs.setValueForKey(data.token, "token");
                        $notify("签到成功", "token=" + data.token, resp.body);
                        $done(JSON.stringify({code: data.code, token: data.token}));
                    })();
                "#,
            ))
            .unwrap();

        let out = scheduler.run_now("qx_signin").await.unwrap();
        assert_eq!(out.0["code"], 0);
        assert_eq!(out.0["token"], "tok123");
        // store 有值（scope 为 prefs:<script_name>）
        assert_eq!(
            store.read("prefs:qx_signin", "token"),
            Some("tok123".to_string())
        );
        // notifier 记录 1 条
        let calls = notifier.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].title, "签到成功");

        // 手动触发记录 last_run / last_error
        let view = scheduler
            .list_tasks()
            .into_iter()
            .find(|v| v.name == "qx_signin")
            .unwrap();
        assert!(view.last_run.is_some());
        assert!(view.last_error.is_none());

        // 不存在任务报 NotFound
        let err = scheduler.run_now("missing").await.unwrap_err();
        assert!(matches!(err, PanelError::NotFound(_)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_due_only_fires_due_tasks() {
        let (host, _n, _s) = test_host();
        let mut scheduler = ScriptScheduler::new(host, ScriptLimits::default());
        // 每分钟执行（秒固定为 0）
        scheduler
            .add_task(qx_task("every_min", "0 * * * * *", "$done({ok: true});"))
            .unwrap();
        // 下周才会执行的表达式（day/month/year 由测试 now 推导）
        let now = Utc.with_ymd_and_hms(2026, 8, 1, 12, 34, 0).unwrap();
        let next_week = now + ChronoDuration::days(7);
        let later_expr = format!(
            "0 0 12 {} {} ? {}",
            next_week.day(),
            next_week.month(),
            next_week.year()
        );
        scheduler
            .add_task(qx_task("next_week", &later_expr, "$done({ok: true});"))
            .unwrap();

        let results = scheduler.run_due(now).await;
        // 只有到期任务被执行
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "every_min");
        assert!(results[0].1.is_ok());
        // 同一时间点重复调用不重复执行（last_run 去重）
        let again = scheduler.run_due(now).await;
        assert!(again.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn start_stop_smoke() {
        let (host, _n, _s) = test_host();
        let mut scheduler = ScriptScheduler::new(host, ScriptLimits::default());
        scheduler
            .add_task(qx_task("minutely", "0 * * * * *", "$done();"))
            .unwrap();
        let scheduler = Arc::new(scheduler);

        let handle = scheduler
            .clone()
            .start_with_interval(Duration::from_millis(20))
            .await
            .unwrap();
        // 等几个 tick，确认后台循环存活
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!handle.is_finished());

        scheduler.stop().await;
        // stop 后 handle 在超时内退出
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("handle should exit after stop")
            .expect("handle join ok");
    }
}
