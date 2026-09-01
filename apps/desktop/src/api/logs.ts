/**
 * Log API types and functions.
 *
 * Aligned with Rust-side `LogEntry`.
 */

import { invoke } from "@tauri-apps/api/core";

/** A log entry (aligned with Rust `LogEntry`: `ts`/`level`/`target`/`message`). */
export interface LogEntry {
  /** RFC3339 local time (e.g. `2026-08-02T21:00:00+08:00`). */
  ts: string;
  /** Level (`ERROR`/`WARN`/`INFO`/`DEBUG`/`TRACE`). */
  level: string;
  /** `tracing` target (module path, or `frontend` for frontend logs). */
  target: string;
  /** Message (with structured fields,形如 `key=value`). */
  message: string;
}

/** Get logs from in-memory ring buffer (newest first), with optional limit and min level. */
export function getLogs(limit?: number, minLevel?: string): Promise<LogEntry[]> {
  return invoke<LogEntry[]>("get_logs", { limit: limit ?? null, minLevel: minLevel ?? null });
}

/** Export all rolling log files under `data_dir/logs/` merged in chronological order, returns exported file path. */
export function exportLogs(): Promise<string> {
  return invoke<string>("export_logs");
}

/** Clear in-memory log ring buffer (does not delete rolling files). */
export function clearLogs(): Promise<void> {
  return invoke<void>("clear_logs");
}

/** Write frontend logs (`target="frontend"`) to backend tracing pipeline, shared with log page and troubleshooting. */
export function logFrontend(level: string, message: string): Promise<void> {
  return invoke<void>("log_frontend", { level, message });
}

/** Open log export directory (Android opens system "Downloads" directory; desktop opens `data_dir/logs`). */
export function openExportDir(): Promise<void> {
  return invoke<void>("open_export_dir");
}

/** List log file names under `data_dir/logs` (`app.log*` rolling files / `libbox.log` / `mihomo.log`), sorted by name descending (newest first). */
export function listLogFiles(): Promise<string[]> {
  return invoke<string[]>("list_log_files");
}

/** Read tail content of specified log file (`maxLines` default/upper limit 1000 lines; large files only read tail bytes to avoid full read). */
export function readLogFileTail(name: string, maxLines?: number): Promise<string> {
  return invoke<string>("read_log_file_tail", { name, maxLines: maxLines ?? null });
}
