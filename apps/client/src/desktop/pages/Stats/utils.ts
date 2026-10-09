import { useEffect, useState } from "react";

/** Format bytes to human-readable string (B / KB / MB / GB). */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const k = 1024;
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), units.length - 1);
  return `${(bytes / k ** i).toFixed(i === 0 ? 0 : 2)} ${units[i]}`;
}

/** Unix 秒 → 本地时间 "MM-DD HH:mm:ss"。 */
export function formatTime(unixSeconds: number): string {
  if (!unixSeconds) return "-";
  const d = new Date(unixSeconds * 1000);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** 输入防抖（搜索框 300ms）。 */
export function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(timer);
  }, [value, delayMs]);
  return debounced;
}
