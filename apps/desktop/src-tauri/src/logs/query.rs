//! Log query commands: get logs, list log files, read tail, clear.

use crate::logs::{
    clear_ring_buffer, entry_sort_ts, passes_level_filter, read_libbox_entries,
    read_tail_bytes, ring_buffer, validate_log_file_name, LogEntry,
    log_tail_default_lines, log_tail_max_lines,
};
use crate::state::AppState;

/// Tauri command: get logs (newest first), with optional limit and min level filter.
///
/// Merges ring buffer with `libbox.log` tail, sorts by timestamp descending.
#[tauri::command]
pub fn get_logs(
    state: tauri::State<'_, AppState>,
    limit: Option<usize>,
    min_level: Option<String>,
) -> Vec<LogEntry> {
    get_logs_impl(&state.data_dir.join("logs"), limit, min_level)
}

/// Pure logic for `get_logs` (testable with injected logs directory).
pub(crate) fn get_logs_impl(
    logs_dir: &std::path::Path,
    limit: Option<usize>,
    min_level: Option<String>,
) -> Vec<LogEntry> {
    let min_rank = min_level.as_deref().and_then(crate::logs::level_rank);
    let mut entries: Vec<LogEntry> = ring_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|entry| passes_level_filter(entry, min_rank))
        .cloned()
        .collect();
    entries.extend(
        read_libbox_entries(logs_dir)
            .into_iter()
            .filter(|entry| passes_level_filter(entry, min_rank)),
    );
    entries.sort_by_key(entry_sort_ts);
    entries.reverse();
    if let Some(limit) = limit {
        entries.truncate(limit);
    }
    entries
}

/// Tauri command: list viewable log file names in `data_dir/logs`.
#[tauri::command]
pub fn list_log_files(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    list_log_files_impl(&state.data_dir.join("logs"))
}

/// Pure logic for `list_log_files`.
pub(crate) fn list_log_files_impl(logs_dir: &std::path::Path) -> Result<Vec<String>, String> {
    if !logs_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = std::fs::read_dir(logs_dir)
        .map_err(|e| format!("读取日志目录失败：{e}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name.starts_with("app.log")
                || matches!(
                    name.as_str(),
                    "libbox.log"
                        | "mihomo.log"
                        | "last_start_config.json"
                        | "last_start_config.yaml"
                )
        })
        .collect();
    names.sort();
    names.reverse();
    Ok(names)
}

/// Tauri command: read tail of a specified log file.
#[tauri::command]
pub fn read_log_file_tail(
    state: tauri::State<'_, AppState>,
    name: String,
    max_lines: Option<u32>,
) -> Result<String, String> {
    read_log_file_tail_impl(&state.data_dir.join("logs"), &name, max_lines)
}

/// Pure logic for `read_log_file_tail`.
pub(crate) fn read_log_file_tail_impl(
    logs_dir: &std::path::Path,
    name: &str,
    max_lines: Option<u32>,
) -> Result<String, String> {
    validate_log_file_name(name)?;
    let path = logs_dir.join(name);
    let max_lines = max_lines
        .unwrap_or(log_tail_default_lines())
        .clamp(1, log_tail_max_lines()) as usize;
    let bytes = read_tail_bytes(&path)?;
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

/// Tauri command: clear in-memory ring buffer.
#[tauri::command]
pub fn clear_logs() {
    clear_ring_buffer();
}

/// Tauri command: write frontend log into tracing pipeline.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logs::test_entry;

    fn temp_logs_dir(tag: &str) -> std::path::PathBuf {
        let tmp = std::env::temp_dir().join(format!("pp-log-{tag}-{}", std::process::id()));
        let logs_dir = tmp.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        logs_dir
    }

    fn remove_temp_logs_dir(logs_dir: &std::path::Path) {
        std::fs::remove_dir_all(logs_dir.parent().unwrap()).ok();
    }

    static RING_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn get_logs_filters_reverses_and_limits() {
        let _guard = RING_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_ring_buffer();
        let logs_dir = temp_logs_dir("filter");
        push_entry(test_entry(0, "INFO"));
        push_entry(test_entry(1, "WARN"));
        push_entry(test_entry(2, "ERROR"));
        push_entry(test_entry(3, "DEBUG"));

        let all = get_logs_impl(&logs_dir, None, None);
        assert_eq!(
            all.iter().map(|e| e.message.clone()).collect::<Vec<_>>(),
            vec!["message 3", "message 2", "message 1", "message 0"]
        );
        assert!(!all.iter().any(|e| e.target == "libbox"));

        let filtered = get_logs_impl(&logs_dir, None, Some("warn".to_string()));
        assert_eq!(
            filtered.iter().map(|e| e.message.clone()).collect::<Vec<_>>(),
            vec!["message 2", "message 1"]
        );

        let limited = get_logs_impl(&logs_dir, Some(2), None);
        assert_eq!(
            limited.iter().map(|e| e.message.clone()).collect::<Vec<_>>(),
            vec!["message 3", "message 2"]
        );
        clear_ring_buffer();
        remove_temp_logs_dir(&logs_dir);
    }

    #[test]
    fn get_logs_merges_libbox_log_sorted_desc() {
        let _guard = RING_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_ring_buffer();
        let logs_dir = temp_logs_dir("merge");
        push_entry(LogEntry {
            ts: "2026-08-02T22:12:29.000+08:00".to_string(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "ring 1".to_string(),
        });
        push_entry(LogEntry {
            ts: "2026-08-02T22:12:31.000+08:00".to_string(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "ring 2".to_string(),
        });
        std::fs::write(
            logs_dir.join("libbox.log"),
            "[2026-08-02T22:12:30.123+08:00] box line 1\n\
             [not-a-timestamp] skipped line\n\
             [2026-08-02T22:12:32.456+08:00] box line 2\n",
        )
        .unwrap();

        let all = get_logs_impl(&logs_dir, None, None);
        assert_eq!(
            all.iter().map(|e| e.ts.as_str()).collect::<Vec<_>>(),
            vec![
                "2026-08-02T22:12:32.456+08:00",
                "2026-08-02T22:12:31.000+08:00",
                "2026-08-02T22:12:30.123+08:00",
                "2026-08-02T22:12:29.000+08:00",
            ]
        );
        let box_entries: Vec<&LogEntry> = all.iter().filter(|e| e.target == "libbox").collect();
        assert_eq!(box_entries.len(), 2);
        assert_eq!(box_entries[0].message, "box line 2");
        assert_eq!(box_entries[1].message, "box line 1");
        assert_eq!(box_entries[0].level, "INFO");

        clear_ring_buffer();
        remove_temp_logs_dir(&logs_dir);
    }

    #[test]
    fn list_log_files_sorts_desc_and_filters() {
        let tmp = std::env::temp_dir().join(format!("pp-log-listfiles-{}", std::process::id()));
        let logs_dir = tmp.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        std::fs::write(logs_dir.join("app.log"), "current\n").unwrap();
        std::fs::write(logs_dir.join("app.log.2026-08-01"), "day1\n").unwrap();
        std::fs::write(logs_dir.join("app.log.2026-08-02"), "day2\n").unwrap();
        std::fs::write(logs_dir.join("libbox.log"), "box\n").unwrap();
        std::fs::write(logs_dir.join("mihomo.log"), "mihomo\n").unwrap();
        std::fs::write(logs_dir.join("last_start_config.json"), "{}\n").unwrap();
        std::fs::write(logs_dir.join("last_start_config.yaml"), "---\n").unwrap();
        std::fs::write(logs_dir.join("export-20260101-000000.log"), "old\n").unwrap();
        std::fs::write(logs_dir.join("README.txt"), "ignore\n").unwrap();

        let names = list_log_files_impl(&logs_dir).unwrap();
        assert_eq!(
            names,
            vec![
                "mihomo.log",
                "libbox.log",
                "last_start_config.yaml",
                "last_start_config.json",
                "app.log.2026-08-02",
                "app.log.2026-08-01",
                "app.log",
            ]
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn list_log_files_missing_dir_returns_empty() {
        let tmp = std::env::temp_dir().join(format!("pp-log-nodir-{}", std::process::id()));
        let logs_dir = tmp.join("logs");
        assert_eq!(
            list_log_files_impl(&logs_dir).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn read_log_file_tail_returns_last_lines() {
        let tmp = std::env::temp_dir().join(format!("pp-log-tail-{}", std::process::id()));
        let logs_dir = tmp.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let content: String = (0..10).map(|i| format!("line {i}\n")).collect();
        std::fs::write(logs_dir.join("app.log"), &content).unwrap();

        assert_eq!(
            read_log_file_tail_impl(&logs_dir, "app.log", Some(3)).unwrap(),
            "line 7\nline 8\nline 9"
        );
        assert_eq!(
            read_log_file_tail_impl(&logs_dir, "app.log", Some(100)).unwrap(),
            content.trim_end()
        );
        assert_eq!(
            read_log_file_tail_impl(&logs_dir, "app.log", None).unwrap(),
            content.trim_end()
        );
        assert_eq!(
            read_log_file_tail_impl(&logs_dir, "app.log", Some(5000)).unwrap(),
            content.trim_end()
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_log_file_tail_validates_name() {
        let tmp = std::env::temp_dir().join(format!("pp-log-valid-{}", std::process::id()));
        let logs_dir = tmp.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        std::fs::write(logs_dir.join("app.log"), "x\n").unwrap();

        assert!(validate_log_file_name("app.log").is_ok());
        assert!(validate_log_file_name("app.log.2026-08-01").is_ok());
        assert!(validate_log_file_name("libbox.log").is_ok());
        assert!(validate_log_file_name("mihomo.log").is_ok());
        assert!(validate_log_file_name("last_start_config.json").is_ok());
        assert!(validate_log_file_name("last_start_config.yaml").is_ok());

        assert!(validate_log_file_name("").is_err());
        assert!(validate_log_file_name("../app.log").is_err());
        assert!(validate_log_file_name("a/../app.log").is_err());
        assert!(validate_log_file_name("evil.log").is_err());

        assert!(read_log_file_tail_impl(&logs_dir, "../app.log", None).is_err());
        assert!(read_log_file_tail_impl(&logs_dir, "evil.log", None).is_err());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_log_file_tail_large_file_reads_tail_only() {
        let tmp = std::env::temp_dir().join(format!("pp-log-large-{}", std::process::id()));
        let logs_dir = tmp.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let mut content = String::with_capacity(crate::logs::LOG_TAIL_MAX_BYTES_VAL as usize + 32);
        content.push_str("UNIQUE_HEAD_MARKER\n");
        content.push_str(&"x".repeat(crate::logs::LOG_TAIL_MAX_BYTES_VAL as usize));
        std::fs::write(logs_dir.join("mihomo.log"), &content).unwrap();

        let tail = read_log_file_tail_impl(&logs_dir, "mihomo.log", None).unwrap();
        assert!(!tail.contains("UNIQUE_HEAD_MARKER"));
        assert!(!tail.is_empty());
        std::fs::remove_dir_all(&tmp).ok();
    }
}
