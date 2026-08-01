use std::sync::Arc;

use crate::types::{HttpRequestSpec, HttpResponseData};
use pp_common::{PanelError, PanelResult};

/// 执行 HTTP 请求的能力（网络仅经由此 trait，后续可审计/代理）。
#[async_trait::async_trait]
pub trait HttpExecutor: Send + Sync {
    async fn execute(&self, req: HttpRequestSpec) -> PanelResult<HttpResponseData>;
}

/// 持久化存储能力。key 维度 = (scope, key)。
pub trait PersistentStore: Send + Sync {
    fn read(&self, scope: &str, key: &str) -> Option<String>;
    fn write(&self, scope: &str, key: &str, value: &str);
    fn erase(&self, scope: &str, key: &str);
}

/// 通知能力。
pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, subtitle: &str, body: &str, options: Option<serde_json::Value>);
}

/// 脚本宿主：脚本运行时可用的全部外部能力集合。
pub struct ScriptHost {
    pub http: Arc<dyn HttpExecutor>,
    pub store: Arc<dyn PersistentStore>,
    pub notifier: Arc<dyn Notifier>,
}

/// 一条 setTimeout 定时记录：到期时间 + 回调（Persistent 化，不持有 'js 引用）。
pub type TimerEntry = (
    std::time::Instant,
    rquickjs::Persistent<rquickjs::Function<'static>>,
);

/// setTimeout 定时器注册表。
///
/// 回调以 `Persistent` 保存，避免 JS 函数对象持有 `Ctx` 形成引用循环；
/// 由引擎在等待 `$done` 期间驱动到期回调，并在 runtime 释放前清空。
/// 仅在单线程 runtime 锁内访问（`Persistent` 非 Send），故用 `Rc<RefCell>`。
pub type TimerRegistry = std::rc::Rc<std::cell::RefCell<Vec<TimerEntry>>>;

impl ScriptHost {
    pub fn new(
        http: Arc<dyn HttpExecutor>,
        store: Arc<dyn PersistentStore>,
        notifier: Arc<dyn Notifier>,
    ) -> Self {
        Self {
            http,
            store,
            notifier,
        }
    }
}

/// 基于内存 HashMap 的持久化存储实现（测试/默认用途）。
#[derive(Debug, Default)]
pub struct MemoryPersistentStore {
    inner: std::sync::RwLock<std::collections::HashMap<(String, String), String>>,
}

impl MemoryPersistentStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PersistentStore for MemoryPersistentStore {
    fn read(&self, scope: &str, key: &str) -> Option<String> {
        let inner = self.inner.read().ok()?;
        inner.get(&(scope.to_string(), key.to_string())).cloned()
    }

    fn write(&self, scope: &str, key: &str, value: &str) {
        if let Ok(mut inner) = self.inner.write() {
            inner.insert((scope.to_string(), key.to_string()), value.to_string());
        }
    }

    fn erase(&self, scope: &str, key: &str) {
        if let Ok(mut inner) = self.inner.write() {
            inner.remove(&(scope.to_string(), key.to_string()));
        }
    }
}

/// 记录所有通知调用历史的 Notifier（测试钩子）。
#[derive(Debug, Default)]
pub struct RecordingNotifier {
    pub calls: std::sync::Mutex<Vec<Notification>>,
}

/// 一次通知记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub options: Option<serde_json::Value>,
}

impl RecordingNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// 读取通知调用历史（测试钩子）。
    pub fn calls(&self) -> Vec<Notification> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

impl Notifier for RecordingNotifier {
    fn notify(&self, title: &str, subtitle: &str, body: &str, options: Option<serde_json::Value>) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(Notification {
                title: title.to_string(),
                subtitle: subtitle.to_string(),
                body: body.to_string(),
                options,
            });
        }
    }
}

/// 预设响应队列的 HttpExecutor（测试用，不做真实网络请求）。
#[derive(Debug)]
pub struct MockHttpExecutor {
    queue: tokio::sync::Mutex<std::collections::VecDeque<HttpResponseData>>,
}

impl MockHttpExecutor {
    /// 按顺序出队响应；队列为空时返回 404 空响应。
    pub fn with_responses(responses: Vec<HttpResponseData>) -> Self {
        Self {
            queue: tokio::sync::Mutex::new(responses.into()),
        }
    }
}

#[async_trait::async_trait]
impl HttpExecutor for MockHttpExecutor {
    async fn execute(&self, _req: HttpRequestSpec) -> PanelResult<HttpResponseData> {
        let mut queue = self.queue.lock().await;
        match queue.pop_front() {
            Some(resp) => Ok(resp),
            None => Err(PanelError::Internal("mock http queue empty".to_string())),
        }
    }
}
