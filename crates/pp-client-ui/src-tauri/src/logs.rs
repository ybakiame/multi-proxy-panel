//! 日志系统：滚动文件持久化 + 内存环形缓冲 + 查询/导出命令。
//!
//! 布局（数据目录 `data_dir`）：
//! - 滚动文件：`data_dir/logs/app.log.<YYYY-MM-DD>`（[`init_logging`] 每日滚动）
//! - 导出文件：`data_dir/logs/export-<YYYYMMDD-HHMMSS>.log`（[`export_logs`] 合并产物）
//!
//! 环形缓冲把每条事件以 [`LogEntry`] 存入全局 [`OnceLock`]，容量 [`RING_CAPACITY`]，
//! 超出弹出最旧；供前端日志页通过 [`get_logs`] 查询，避免日志页请求走磁盘。

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tracing::field::Field;
use tracing::{field, Event, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::EnvFilter;

use crate::state::AppState;

/// 环形缓冲容量：超出后弹出最旧的条目。
pub const RING_CAPACITY: usize = 1000;

/// 未设置 `RUST_LOG` 时的默认过滤规则（本应用相关 crate 提级）。
const DEFAULT_ENV_FILTER: &str = "info,pp_client=debug,pp_mitm=debug,pp_script=debug";

/// 内存环形缓冲中的一条日志。
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    /// RFC3339 本地时间（如 `2026-08-02T21:00:00+08:00`）。
    pub ts: String,
    /// 级别（`ERROR`/`WARN`/`INFO`/`DEBUG`/`TRACE`）。
    pub level: String,
    /// `tracing` target（模块路径，或前端日志的 `frontend`）。
    pub target: String,
    /// 消息（含结构化字段，形如 `key=value`）。
    pub message: String,
}

/// 全局环形缓冲。
static RING_BUFFER: OnceLock<Arc<Mutex<VecDeque<LogEntry>>>> = OnceLock::new();

/// 获取全局环形缓冲（首次调用时惰性初始化）。
fn ring_buffer() -> &'static Arc<Mutex<VecDeque<LogEntry>>> {
    RING_BUFFER.get_or_init(|| Arc::new(Mutex::new(VecDeque::with_capacity(RING_CAPACITY))))
}

/// 日志级别 → 严重度排行（`error` 最高）。未知级别返回 `None`。
fn level_rank(level: &str) -> Option<u8> {
    match level.to_ascii_lowercase().as_str() {
        "error" => Some(5),
        "warn" => Some(4),
        "info" => Some(3),
        "debug" => Some(2),
        "trace" => Some(1),
        _ => None,
    }
}

/// 当前时间，RFC3339 本地时间格式。
fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339()
}

/// 把条目 push 进环形缓冲；容量已满时弹出最旧条目。
fn push_entry(entry: LogEntry) {
    let mut buf = ring_buffer().lock().unwrap_or_else(|e| e.into_inner());
    if buf.len() == RING_CAPACITY {
        buf.pop_front();
    }
    buf.push_back(entry);
}

/// 清空环形缓冲（不影响滚动文件）。
fn clear_ring_buffer() {
    ring_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

/// 把 `tracing` 事件格式化为 [`LogEntry`] 并 push 进环形缓冲的 [`Layer`]。
#[derive(Debug)]
pub struct RingBufferLayer;

impl<S> Layer<S> for RingBufferLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        push_entry(LogEntry {
            ts: now_rfc3339(),
            level: event.metadata().level().to_string(),
            target: event.metadata().target().to_string(),
            message: format_event_message(event),
        });
    }
}

/// 收集事件字段的 visitor（`message` 字段单独提取为消息正文）。
#[derive(Default)]
struct FieldCollector {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl field::Visit for FieldCollector {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            // `fmt::Arguments`（format_args! 消息）经 record_debug 落地。
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields
                .push((field.name().to_string(), format!("{value:?}")));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }
}

/// 把事件格式化为消息正文：`消息内容 key=value ...`（无字段时仅消息本身）。
fn format_event_message(event: &Event<'_>) -> String {
    let mut collector = FieldCollector::default();
    event.record(&mut collector);
    let fields = format_fields(&collector.fields);
    match (collector.message, fields.is_empty()) {
        (Some(message), true) => message,
        (Some(message), false) => format!("{message} {fields}"),
        (None, true) => String::new(),
        (None, false) => fields,
    }
}

/// 把 `(key, value)` 字段序列化为 `key=value` 空格分隔串。
fn format_fields(fields: &[(String, String)]) -> String {
    fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 初始化全局日志：stderr（保留原有控制台行为）+ 每日滚动文件 + 内存环形缓冲。
///
/// 返回的 [`WorkerGuard`] **必须**由调用方持有到进程退出（存入 `AppState`）：
/// guard 被 Drop 后非阻塞写入线程即关闭，滚动文件停止接收日志；正常退出时
/// guard 的 Drop 会冲刷剩余日志行。
///
/// 过滤规则：`RUST_LOG` 环境变量优先；未设置时默认 [`DEFAULT_ENV_FILTER`]。
pub fn init_logging(data_dir: &Path) -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily(data_dir.join("logs"), "app.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = std::env::var("RUST_LOG")
        .map(EnvFilter::new)
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_ENV_FILTER));

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .with(RingBufferLayer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to set global tracing subscriber");

    guard
}

/// Tauri 命令：从环形缓冲取日志（最新在前），可按数量截断、按最小级别过滤。
///
/// `min_level` 取 `error`/`warn`/`info`/`debug`/`trace`，过滤掉级别更低的条目；
/// 非法值等同不设过滤。
#[tauri::command]
pub fn get_logs(limit: Option<usize>, min_level: Option<String>) -> Vec<LogEntry> {
    let min_rank = min_level.as_deref().and_then(level_rank);
    let mut entries: Vec<LogEntry> = ring_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|entry| match min_rank {
            Some(min) => level_rank(&entry.level).is_some_and(|rank| rank >= min),
            None => true,
        })
        .rev()
        .cloned()
        .collect();
    if let Some(limit) = limit {
        entries.truncate(limit);
    }
    entries
}

/// Tauri 命令：把 `data_dir/logs/` 下全部滚动文件按时间序合并导出，返回导出文件路径。
#[tauri::command]
pub fn export_logs(state: tauri::State<'_, AppState>) -> Result<String, String> {
    write_export(&state.data_dir.join("logs")).map(|path| path.to_string_lossy().to_string())
}

/// Tauri 命令：清空内存环形缓冲（不删除滚动文件）。
#[tauri::command]
pub fn clear_logs() {
    clear_ring_buffer();
}

/// Tauri 命令：把前端日志以 `target="frontend"` 写入 tracing 管道。
///
/// 与后端日志同走一条管道：环形缓冲与滚动文件同时收到，供日志页与排查共用。
#[tauri::command]
pub fn log_frontend(level: String, message: String) {
    match level.to_ascii_lowercase().as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warn" | "warning" => tracing::warn!(target: "frontend", "{message}"),
        "debug" => tracing::debug!(target: "frontend", "{message}"),
        "trace" => tracing::trace!(target: "frontend", "{message}"),
        _ => tracing::info!(target: "frontend", "{message}"),
    }
}

/// 列出 `logs_dir` 下全部日志文件。
///
/// 规则：目录下所有以 `.log` 结尾的文件（覆盖 Kotlin 侧 `libbox.log`），
/// 并保留 `app.log.<YYYY-MM-DD>` 滚动文件（`tracing_appender` 以日期结尾命名，
/// 不以 `.log` 结尾）；排除本功能导出的 `export-*.log` 合并产物，避免重复累加。
/// 按文件名字典序排序——`*.log.<YYYY-MM-DD>` 日期零填充，字典序即时间序。
fn list_daily_files(logs_dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !logs_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(logs_dir)
        .map_err(|e| format!("读取日志目录失败：{e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| {
                    !name.starts_with("export-")
                        && (name.starts_with("app.log") || name.ends_with(".log"))
                })
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    Ok(files)
}

/// 把全部滚动文件按时间序合并写入 `logs_dir/export-<YYYYMMDD-HHMMSS>.log`。
fn write_export(logs_dir: &Path) -> Result<PathBuf, String> {
    if !logs_dir.is_dir() {
        return Err(format!("日志目录不存在：{}", logs_dir.display()));
    }
    let files = list_daily_files(logs_dir)?;
    let export_path = logs_dir.join(format!(
        "export-{}.log",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let mut out =
        std::fs::File::create(&export_path).map_err(|e| format!("创建导出文件失败：{e}"))?;
    for file in &files {
        let content = std::fs::read(file).map_err(|e| format!("读取日志文件失败：{e}"))?;
        std::io::Write::write_all(&mut out, &content)
            .map_err(|e| format!("写入导出文件失败：{e}"))?;
    }
    Ok(export_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试条目（`message` 带序号便于断言滚动/排序）。
    fn test_entry(id: usize, level: &str) -> LogEntry {
        LogEntry {
            ts: format!("2026-08-02T00:00:{id:02}+08:00"),
            level: level.to_string(),
            target: "test".to_string(),
            message: format!("message {id}"),
        }
    }

    fn entry_messages(entries: &[LogEntry]) -> Vec<String> {
        entries.iter().map(|e| e.message.clone()).collect()
    }

    #[test]
    fn ring_buffer_rolls_capacity() {
        clear_ring_buffer();
        for id in 0..(RING_CAPACITY + 5) {
            push_entry(test_entry(id, "INFO"));
        }
        let entries: Vec<LogEntry> = ring_buffer()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();
        assert_eq!(entries.len(), RING_CAPACITY);
        // 最旧的 5 条（0..5）被弹出，剩余从 5 开始到 1004。
        assert_eq!(entries.first().unwrap().message, "message 5");
        assert_eq!(
            entries.last().unwrap().message,
            format!("message {}", RING_CAPACITY + 4)
        );
        clear_ring_buffer();
    }

    #[test]
    fn level_rank_scores() {
        assert_eq!(level_rank("ERROR"), Some(5));
        assert_eq!(level_rank("warn"), Some(4));
        assert_eq!(level_rank("Info"), Some(3));
        assert_eq!(level_rank("DEBUG"), Some(2));
        assert_eq!(level_rank("trace"), Some(1));
        assert_eq!(level_rank("unknown"), None);
        assert_eq!(level_rank(""), None);
    }

    #[test]
    fn get_logs_filters_reverses_and_limits() {
        clear_ring_buffer();
        push_entry(test_entry(0, "INFO"));
        push_entry(test_entry(1, "WARN"));
        push_entry(test_entry(2, "ERROR"));
        push_entry(test_entry(3, "DEBUG"));

        // 倒序取全部：3,2,1,0。
        let all = get_logs(None, None);
        assert_eq!(
            entry_messages(&all),
            vec!["message 3", "message 2", "message 1", "message 0"]
        );

        // min_level=warn：只剩 ERROR/WARN，倒序。
        let filtered = get_logs(None, Some("warn".to_string()));
        assert_eq!(entry_messages(&filtered), vec!["message 2", "message 1"]);

        // limit 截断。
        let limited = get_logs(Some(2), None);
        assert_eq!(entry_messages(&limited), vec!["message 3", "message 2"]);
        clear_ring_buffer();
    }

    #[test]
    fn export_merges_in_time_order() {
        let tmp = std::env::temp_dir().join(format!("pp-log-test-{}", std::process::id()));
        let logs_dir = tmp.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        // 乱序写入三个滚动文件，另放一个此前导出的文件验证被排除，
        // 再放 Kotlin 侧 libbox.log（固定文件名）验证纳入导出。
        std::fs::write(logs_dir.join("app.log.2026-08-02"), "day2\n").unwrap();
        std::fs::write(logs_dir.join("app.log.2026-08-01"), "day1\n").unwrap();
        std::fs::write(logs_dir.join("app.log.2026-07-31"), "day0\n").unwrap();
        std::fs::write(logs_dir.join("libbox.log"), "box\n").unwrap();
        std::fs::write(logs_dir.join("export-20260101-000000.log"), "old export\n").unwrap();

        let export_path = write_export(&logs_dir).unwrap();
        let merged = std::fs::read_to_string(&export_path).unwrap();
        // 按文件名字典序：app.* 滚动文件按日期升序，随后 libbox.log。
        assert_eq!(merged, "day0\nday1\nday2\nbox\n");
        assert!(export_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("export-"));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn list_daily_files_collects_all_log_files() {
        let tmp = std::env::temp_dir().join(format!("pp-log-list-{}", std::process::id()));
        let logs_dir = tmp.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        std::fs::write(logs_dir.join("app.log"), "current\n").unwrap();
        std::fs::write(logs_dir.join("app.log.2026-08-01"), "day1\n").unwrap();
        std::fs::write(logs_dir.join("libbox.log"), "box\n").unwrap();
        std::fs::write(logs_dir.join("export-20260101-000000.log"), "old\n").unwrap();
        std::fs::write(logs_dir.join("README.txt"), "ignore\n").unwrap();

        let files = list_daily_files(&logs_dir).unwrap();
        let names: Vec<String> = files
            .iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str().map(String::from))
            .collect();
        // app.log 滚动（当前 + 历史）与 Kotlin 侧 libbox.log 均纳入，
        // export-*.log 与无关文件被排除。
        assert_eq!(names, vec!["app.log", "app.log.2026-08-01", "libbox.log"]);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
