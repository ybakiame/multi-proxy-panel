use std::path::{Path, PathBuf};
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

/// 单个 scope 的落盘内容：内嵌 scope 字段以便校验与恢复。
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ScopeFile {
    scope: String,
    entries: std::collections::HashMap<String, String>,
}

/// 基于磁盘目录的持久化存储实现（重启不丢）。
///
/// 每个 scope 一个 JSON 文件：文件名 = scope 的 SHA-256 前 16 位 hex + `.json`
/// （固定长度、URL-safe，且文件内嵌 `scope` 字段自校验），write/erase 同步
/// 原子落盘（tempfile + rename）。启动时全量加载到内存，read 走内存命中。
pub struct FilePersistentStore {
    dir: PathBuf,
    inner: MemoryPersistentStore,
}

impl FilePersistentStore {
    /// 创建存储：确保目录存在并加载既有数据。
    ///
    /// 目录下损坏或不可读的 `.json` 文件跳过并记 warning，不 panic。
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let inner = Self::load(&dir);
        Self { dir, inner }
    }

    /// 扫描目录，把每个 scope 文件的 entries 载入内存。
    fn load(dir: &Path) -> MemoryPersistentStore {
        let store = MemoryPersistentStore::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return store;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skip unreadable store file");
                    continue;
                }
            };
            match serde_json::from_str::<ScopeFile>(&text) {
                Ok(file) => {
                    for (key, value) in file.entries {
                        store.write(&file.scope, &key, &value);
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skip corrupted store file");
                }
            }
        }
        store
    }

    /// scope 对应的落盘路径：SHA-256 前 16 位 hex + `.json`。
    fn scope_path(&self, scope: &str) -> PathBuf {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(scope.as_bytes());
        let digest = hasher.finalize();
        let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join(format!("{hex}.json"))
    }

    /// 将 scope 的当前 entries 同步原子落盘；scope 已无 key 时删除文件。
    fn persist(&self, scope: &str) {
        let path = self.scope_path(scope);
        // 从内存收集该 scope 的全部 key → value 快照。
        let mut entries = std::collections::HashMap::new();
        if let Ok(inner) = self.inner.inner.read() {
            for ((s, k), v) in inner.iter() {
                if s == scope {
                    entries.insert(k.clone(), v.clone());
                }
            }
        }
        if entries.is_empty() {
            let _ = std::fs::remove_file(&path);
            return;
        }
        let json = match serde_json::to_vec(&ScopeFile {
            scope: scope.to_string(),
            entries,
        }) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!(scope, error = %e, "store file serialization failed");
                return;
            }
        };
        // tempfile + rename 原子写，避免半写文件被下一次启动读到。
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let tmp = self
            .dir
            .join(format!("{:08x}.{nanos:016x}.tmp", std::process::id()));
        if let Err(e) = std::fs::write(&tmp, &json) {
            tracing::error!(path = %tmp.display(), error = %e, "write temp store file failed");
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, &path) {
            tracing::error!(path = %path.display(), error = %e, "rename store file failed");
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

impl PersistentStore for FilePersistentStore {
    fn read(&self, scope: &str, key: &str) -> Option<String> {
        self.inner.read(scope, key)
    }

    fn write(&self, scope: &str, key: &str, value: &str) {
        self.inner.write(scope, key, value);
        self.persist(scope);
    }

    fn erase(&self, scope: &str, key: &str) {
        self.inner.erase(scope, key);
        self.persist(scope);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 目录下的文件列表（不含子目录）。
    fn json_files(dir: &Path) -> Vec<std::ffi::OsString> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| n.to_string_lossy().ends_with(".json"))
            .collect()
    }

    /// ① write 后新实例（同 dir）read 命中：重启持久性。
    #[test]
    fn file_store_persists_across_instances() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilePersistentStore::new(dir.path().to_path_buf());
        store.write("prefs:demo", "foo", "bar");
        store.write("prefs:demo", "n", "42");
        store.write("pstore:other", "k", "v");
        drop(store);

        let reloaded = FilePersistentStore::new(dir.path().to_path_buf());
        assert_eq!(reloaded.read("prefs:demo", "foo").as_deref(), Some("bar"));
        assert_eq!(reloaded.read("prefs:demo", "n").as_deref(), Some("42"));
        assert_eq!(reloaded.read("pstore:other", "k").as_deref(), Some("v"));
        assert_eq!(reloaded.read("prefs:demo", "missing"), None);
    }

    /// ② erase 后落盘同步删除：文件内容更新，scope 清空后文件删除。
    #[test]
    fn file_store_erase_syncs_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = FilePersistentStore::new(dir.path().to_path_buf());
        store.write("pstore:x", "k", "v");
        store.write("pstore:x", "k2", "v2");
        assert_eq!(json_files(dir.path()).len(), 1, "每个 scope 一个 JSON 文件");

        store.erase("pstore:x", "k");
        // 仍有 k2：文件保留，内容同步。
        let reloaded = FilePersistentStore::new(dir.path().to_path_buf());
        assert_eq!(reloaded.read("pstore:x", "k"), None);
        assert_eq!(reloaded.read("pstore:x", "k2").as_deref(), Some("v2"));

        store.erase("pstore:x", "k2");
        // scope 清空：文件被删除。
        assert!(json_files(dir.path()).is_empty(), "scope 清空后文件应删除");
        let reloaded = FilePersistentStore::new(dir.path().to_path_buf());
        assert_eq!(reloaded.read("pstore:x", "k2"), None);
    }

    /// ③ scope 含特殊字符（`:`、`/`）时文件名安全。
    #[test]
    fn file_store_handles_special_chars_in_scope() {
        let dir = tempfile::tempdir().unwrap();
        let scope = "prefs:http://example.com/脚本?x=1:y";
        let store = FilePersistentStore::new(dir.path().to_path_buf());
        store.write(scope, "k", "v");
        drop(store);

        // 文件名 = 16 位 hex + .json，不含原始 scope 特殊字符。
        let files = json_files(dir.path());
        assert_eq!(files.len(), 1);
        let name = files[0].to_string_lossy();
        assert!(name.ends_with(".json"));
        assert!(
            name[..name.len() - 5]
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );

        let reloaded = FilePersistentStore::new(dir.path().to_path_buf());
        assert_eq!(reloaded.read(scope, "k").as_deref(), Some("v"));
    }

    /// ④ 损坏 JSON 文件：warn 跳过不 panic，有效数据照常加载。
    #[test]
    fn file_store_skips_corrupted_file_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("deadbeef.json"), b"{ not json").unwrap();
        let store = FilePersistentStore::new(dir.path().to_path_buf());
        store.write("pstore:ok", "k", "v");
        drop(store);

        // 损坏文件仍在（未被覆盖），但新实例不 panic，有效数据可读。
        assert_eq!(json_files(dir.path()).len(), 2);
        let reloaded = FilePersistentStore::new(dir.path().to_path_buf());
        assert_eq!(reloaded.read("pstore:ok", "k").as_deref(), Some("v"));
        assert_eq!(reloaded.read("pstore:missing", "k"), None);
    }
}
