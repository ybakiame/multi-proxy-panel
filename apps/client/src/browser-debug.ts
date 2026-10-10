/**
 * 浏览器调试模式（仅 dev 构建生效）。
 *
 * 纯浏览器打开 vite devUrl 时缺少 Tauri IPC 桥，`App` 会渲染「请在客户端内运行」
 * 拦截页。此处用官方 `@tauri-apps/api/mocks` 安装一个 mock IPC 层，使页面可在
 * 普通浏览器中渲染，用于纯 UI / 样式调试（agent-browser 等）。
 *
 * 开启方式（二选一，均持久化到 localStorage，之后免参数）：
 * - URL 参数 `?browser-debug=1`（`?browser-debug=0` 关闭）
 * - DevTools 执行 `localStorage.setItem("pp-browser-debug", "1")` 后刷新
 *
 * 本模块必须在所有 `@pp/client-core` / `App` 导入之前执行：env.ts 的
 * `isTauriEnv` 在模块加载时求值，mock 装晚了就翻不过来。
 */
import { mockIPC } from "@tauri-apps/api/mocks";

const FLAG_KEY = "pp-browser-debug";

function browserDebugEnabled(): boolean {
  if (!import.meta.env.DEV) {
    return false;
  }
  const params = new URLSearchParams(window.location.search);
  if (params.has("browser-debug")) {
    window.localStorage.setItem(FLAG_KEY, params.get("browser-debug") === "0" ? "" : "1");
  }
  return window.localStorage.getItem(FLAG_KEY) === "1";
}

/** 常用命令的最小可用返回值；未列出的命令一律 `null`（查询走空态/错误态即可渲染）。 */
const MOCKS: Record<string, unknown> = {
  get_capabilities: {
    os: "linux",
    is_android: false,
    capabilities: {
      mitm: true,
      system_proxy: true,
      core_management: true,
      tun_toggle: true,
      scripts_remote: true,
      cron_tasks: true,
    },
  },
  platform_info: { os: "linux" },
  local_override_get: { singbox: { rules: [], rule_sets: [] }, custom_rule_sets: [] },
  baseline_view_get: {
    rule_sets: [],
    route_rules: [],
    dns_rules: [],
    outbounds: [
      { tag: "proxy", kind: "selector", description: "手动选择" },
      { tag: "auto", kind: "urltest", description: "自动选择" },
      { tag: "direct", kind: "direct", description: "直连" },
      { tag: "block", kind: "block", description: "拦截" },
    ],
    route_final: "proxy",
  },
  list_profiles: [],
  list_subscriptions: [],
  list_cores: [],
  list_remotes: [],
  proxy_status: null,
  toast_mode_override: null,
};

if (browserDebugEnabled() && !("__TAURI_INTERNALS__" in window)) {
  mockIPC((cmd) => (cmd in MOCKS ? MOCKS[cmd] : null));
  console.info("[browser-debug] mock IPC 已安装，页面数据为占位空态；?browser-debug=0 关闭");
}
