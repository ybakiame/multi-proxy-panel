/**
 * 运行环境探测：Tauri 运行时向 webview 注入 `__TAURI_INTERNALS__`（`invoke` 依赖它），
 * 直接用浏览器打开 vite devUrl（http://localhost:1420）时没有注入，
 * 任何 invoke 都会抛 "Cannot read properties of undefined (reading 'invoke')"。
 * App 入口据此拦截渲染，避免各页面 Query 反复 invoke 失败、错误 Alert 刷屏。
 */
export const isTauriEnv = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
