import { invoke } from "@tauri-apps/api/core";
import { toErrorMessage } from "./api";

/**
 * 前端日志捕获：把 `window.onerror`、未处理的 Promise 拒绝与 `console.error/warn`
 * 接入后端 `log_frontend` 命令（`target="frontend"`），与后端日志同走一条管道，
 * 供日志页查看与问题排查。
 *
 * 防递归：IPC 发送一律静默失败（同步 try/catch + `.catch(() => {})`），绝不回到
 * `console.error` 触发包装器自递归。
 *
 * 批量/节流：同一消息 1 秒内去重；待发队列每 500ms 统一发送一次，
 * 避免高频错误打爆 IPC。
 */

/** 同内容消息去重窗口（毫秒）。 */
const DEDUP_WINDOW_MS = 1000;
/** 队列批量发送间隔（毫秒）。 */
const FLUSH_INTERVAL_MS = 500;
/** 去重表上限：超过后清理过期项，防止长期运行内存膨胀。 */
const DEDUP_MAX_ENTRIES = 200;

interface LogItem {
  level: string;
  message: string;
}

let queue: LogItem[] = [];
let flushTimer: ReturnType<typeof setTimeout> | null = null;
/** `level|message` -> 最近入队时间戳（用于 1 秒内去重）。 */
const dedup = new Map<string, number>();

/** 入队一条日志（同步调用），超过去重窗口的重复消息直接丢弃。 */
function enqueue(level: string, message: string): void {
  const now = Date.now();
  // 定期清理过期去重项，保持表有界。
  if (dedup.size >= DEDUP_MAX_ENTRIES) {
    for (const [key, ts] of dedup) {
      if (now - ts >= DEDUP_WINDOW_MS) {
        dedup.delete(key);
      }
    }
  }
  const dedupKey = `${level}|${message}`;
  const last = dedup.get(dedupKey);
  if (last !== undefined && now - last < DEDUP_WINDOW_MS) {
    return;
  }
  dedup.set(dedupKey, now);
  queue.push({ level, message });
  scheduleFlush();
}

/** 安排一次批量发送（500ms 间隔，同时间最多挂一个定时器）。 */
function scheduleFlush(): void {
  if (flushTimer !== null) {
    return;
  }
  flushTimer = setTimeout(() => {
    flushTimer = null;
    flush();
    if (queue.length > 0) {
      scheduleFlush();
    }
  }, FLUSH_INTERVAL_MS);
}

/** 把待发队列整体发送给后端。invoke 失败静默，避免回到 console 触发递归。 */
function flush(): void {
  const batch = queue;
  queue = [];
  for (const item of batch) {
    try {
      void invoke("log_frontend", { level: item.level, message: item.message }).catch(() => {
        // 忽略发送失败（例如后端命令未注册），静默丢弃。
      });
    } catch {
      // 同步抛错同样静默。
    }
  }
}

/** 把 console 参数格式化为单行消息：Error 取 message，对象 JSON 序列化。 */
function formatArgs(args: unknown[]): string {
  return args
    .map((arg) => {
      if (arg instanceof Error) {
        return toErrorMessage(arg);
      }
      if (typeof arg === "string") {
        return arg;
      }
      if (arg === null) {
        return "null";
      }
      if (arg === undefined) {
        return "undefined";
      }
      try {
        const serialized = JSON.stringify(arg);
        return serialized ?? String(arg);
      } catch {
        return String(arg);
      }
    })
    .join(" ");
}

let installed = false;

/** 安装前端日志捕获（幂等，仅执行一次）。在应用入口（main.tsx）调用。 */
export function installLogCapture(): void {
  if (installed) {
    return;
  }
  installed = true;

  window.onerror = (event, source, lineno, _colno, error) => {
    const message = error instanceof Error ? toErrorMessage(error) : `${String(event)} @ ${source}:${lineno}`;
    enqueue("error", `[window.onerror] ${message}`);
  };

  window.addEventListener("unhandledrejection", (event) => {
    enqueue("error", `[unhandledrejection] ${toErrorMessage(event.reason)}`);
  });

  // 劫持 console.warn/error：保留原输出（包装非替换），同步转发到日志管道。
  const originalError = console.error;
  const originalWarn = console.warn;

  console.error = (...args: unknown[]) => {
    originalError(...args);
    enqueue("error", formatArgs(args));
  };

  console.warn = (...args: unknown[]) => {
    originalWarn(...args);
    enqueue("warn", formatArgs(args));
  };
}
