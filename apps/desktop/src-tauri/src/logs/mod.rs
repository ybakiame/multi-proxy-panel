//! Log system: rolling file persistence + in-memory ring buffer + query/export commands.
//!
//! Layout (`data_dir`):
//! - Rolling files: `data_dir/logs/app.log.<YYYY-MM-DD>` (daily rotation by `init_logging`)
//! - libbox log: `data_dir/logs/libbox.log` (written by Kotlin side)
//! - Export file: `data_dir/logs/export-<YYYYMMDD-HHMMSS>.log` (desktop merged output)

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use tracing::field::Field;
use tracing::{field, Event, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::EnvFilter;

pub mod export;
pub mod query;

pub use export::*;
pub use query::*;

/// Ring buffer capacity: evicts oldest entries when full.
pub const RING_CAPACITY: usize = 1000;

/// Max tail bytes to read from `logs/libbox.log` (Kotlin-side libbox log).
const LIBBOX_LOG_MAX_TAIL_BYTES: usize = 256 * 1024;

/// Max bytes per file for `read_log_file_tail` (seeks to tail when exceeded).
const LOG_TAIL_MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Default lines returned by `read_log_file_tail`.
const LOG_TAIL_DEFAULT_LINES: u32 = 1000;
/// Max lines returned by `read_log_file_tail`.
const LOG_TAIL_MAX_LINES: u32 = 1000;

/// Default env filter when `RUST_LOG` is not set.
const DEFAULT_ENV_FILTER: &str = "info,pp_client=debug,pp_mitm=debug,pp_script=debug";

/// A single log entry in the in-memory ring buffer.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    /// RFC3339 local time (e.g. `2026-08-02T21:00:00+08:00`).
    pub ts: String,
    /// Level (`ERROR`/`WARN`/`INFO`/`DEBUG`/`TRACE`).
    pub level: String,
    /// `tracing` target (module path, or `frontend` for frontend logs).
    pub target: String,
    /// Message (with structured fields like `key=value`).
    pub message: String,
}

/// Global ring buffer.
static RING_BUFFER: OnceLock<Arc<Mutex<VecDeque<LogEntry>>>> = OnceLock::new();

/// Get or initialize the global ring buffer.
fn ring_buffer() -> &'static Arc<Mutex<VecDeque<LogEntry>>> {
    RING_BUFFER.get_or_init(|| Arc::new(Mutex::new(VecDeque::with_capacity(RING_CAPACITY))))
}

/// Level to severity rank (`error` highest). Returns `None` for unknown levels.
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

/// Current time as RFC3339 local time.
fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339()
}

/// Push an entry into the ring buffer; evicts oldest when at capacity.
pub(crate) fn push_entry(entry: LogEntry) {
    let mut buf = ring_buffer().lock().unwrap_or_else(|e| e.into_inner());
    if buf.len() == RING_CAPACITY {
        buf.pop_front();
    }
    buf.push_back(entry);
}

/// Clear the ring buffer (does not affect rolling files).
pub(crate) fn clear_ring_buffer() {
    ring_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

/// `tracing` Layer that formats events into `LogEntry` and pushes to ring buffer.
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

/// Visitor that collects event fields (`message` extracted separately).
#[derive(Default)]
struct FieldCollector {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl field::Visit for FieldCollector {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
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

/// Format an event into message body: `message key=value ...`.
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

/// Serialize `(key, value)` fields into `key=value` space-separated string.
fn format_fields(fields: &[(String, String)]) -> String {
    fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Initialize global logging: stderr + daily rolling file + ring buffer.
///
/// The returned `WorkerGuard` must be held by the caller until process exit.
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

/// Check if entry passes minimum level filter.
pub(crate) fn passes_level_filter(entry: &LogEntry, min_rank: Option<u8>) -> bool {
    match min_rank {
        Some(min) => level_rank(&entry.level).is_some_and(|rank| rank >= min),
        None => true,
    }
}

/// Parse entry timestamp for sorting; returns `None` on failure (sorted to end).
pub(crate) fn entry_sort_ts(entry: &LogEntry) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(&entry.ts).ok()
}

/// Read `libbox.log` tail and parse into `LogEntry` items.
pub(crate) fn read_libbox_entries(logs_dir: &Path) -> Vec<LogEntry> {
    let bytes = match std::fs::read(logs_dir.join("libbox.log")) {
        Ok(bytes) => bytes,
        Err(_) => return Vec::new(),
    };
    let tail = if bytes.len() > LIBBOX_LOG_MAX_TAIL_BYTES {
        let start = bytes.len() - LIBBOX_LOG_MAX_TAIL_BYTES;
        let line_start = bytes[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(start, |i| start + i + 1);
        &bytes[line_start..]
    } else {
        &bytes[..]
    };
    String::from_utf8_lossy(tail)
        .lines()
        .filter_map(parse_libbox_line)
        .collect()
}

/// Parse a `[ts] message` line from libbox log.
fn parse_libbox_line(line: &str) -> Option<LogEntry> {
    let rest = line.trim_end().strip_prefix('[')?;
    let (ts, message) = rest.split_once("] ")?;
    DateTime::parse_from_rfc3339(ts).ok()?;
    Some(LogEntry {
        ts: ts.to_string(),
        level: "INFO".to_string(),
        target: "libbox".to_string(),
        message: message.to_string(),
    })
}

/// Validate log file name for security.
pub(crate) fn validate_log_file_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("非法的日志文件名".to_string());
    }
    if name.starts_with("app.log")
        || matches!(
            name,
            "libbox.log" | "mihomo.log" | "last_start_config.json" | "last_start_config.yaml"
        )
    {
        Ok(())
    } else {
        Err(format!("非法的日志文件名：{name}"))
    }
}

/// Read file content; seeks to tail when exceeding `LOG_TAIL_MAX_BYTES`.
pub(crate) fn read_tail_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("读取日志文件失败：{e}"))?
        .len();
    if size <= LOG_TAIL_MAX_BYTES {
        return std::fs::read(path).map_err(|e| format!("读取日志文件失败：{e}"));
    }
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).map_err(|e| format!("读取日志文件失败：{e}"))?;
    file.seek(SeekFrom::End(-(LOG_TAIL_MAX_BYTES as i64)))
        .map_err(|e| format!("读取日志文件失败：{e}"))?;
    let mut buf = vec![0u8; LOG_TAIL_MAX_BYTES as usize];
    file.read_exact(&mut buf)
        .map_err(|e| format!("读取日志文件失败：{e}"))?;
    let line_start = buf.iter().position(|&b| b == b'\n').map_or(0, |i| i + 1);
    buf.drain(..line_start);
    Ok(buf)
}

/// Default lines for `read_log_file_tail`.
pub(crate) fn log_tail_default_lines() -> u32 {
    LOG_TAIL_DEFAULT_LINES
}

/// Max lines for `read_log_file_tail`.
pub(crate) fn log_tail_max_lines() -> u32 {
    LOG_TAIL_MAX_LINES
}

/// Max bytes for `read_log_file_tail`.
pub(crate) const LOG_TAIL_MAX_BYTES_VAL: u64 = LOG_TAIL_MAX_BYTES;

/// Construct a test log entry (message carries sequence number for assertions).
#[cfg(test)]
pub(crate) fn test_entry(id: usize, level: &str) -> LogEntry {
    LogEntry {
        ts: format!("2026-08-02T00:00:{id:02}+08:00"),
        level: level.to_string(),
        target: "test".to_string(),
        message: format!("message {id}"),
    }
}
