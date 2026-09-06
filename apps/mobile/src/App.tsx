import { ToastProvider } from "@heroui/react";
import { HashRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";
import { useEffect, useRef } from "react";
import { isTauriEnv } from "@pp/client-core";
import { TABS, TabBar } from "./components/TabBar";
import Dashboard from "./pages/Dashboard";
import Proxies from "./pages/Proxies";
import Rules from "./pages/Rules";
import Settings from "./pages/Settings";

/** 非 Tauri 环境拦截：浏览器打开 devUrl 无 IPC 桥，任何 invoke 都失败；用原生元素渲染避免轮询失败刷屏（与 desktop 同理）。 */
function TauriRequired() {
  return (
    <div className="flex h-screen w-full flex-col items-center justify-center gap-4 bg-background p-6 text-foreground">
      <h1 className="text-xl font-semibold">请在客户端内运行</h1>
      <p className="max-w-md text-center text-sm text-muted">
        当前页面通过浏览器直接访问，缺少 Tauri 运行环境，所有本地命令不可用。 请使用{" "}
        <code className="rounded bg-default-100 px-1 py-0.5">bun run tauri android dev</code> 启动移动客户端（端口
        1430），或运行已构建的 ProxyPanel 应用。
      </p>
    </div>
  );
}

/**
 * 移动端应用骨架（ADR-0003 M5）：HashRouter + 3 Tab 路由 + 二级页。
 *
 * - 布局：顶部内容滚动区（各页面自行处理 `env(safe-area-inset-top)`）+ 底部 TabBar
 *   （处理 `env(safe-area-inset-bottom)`）；内容区不被 TabBar 遮挡。
 * - 路由：`/` 首页仪表盘、`/rules` 规则管理（占位）、`/settings` 设置（占位）；
 *   `/proxies` 代理选择页为非 Tab 二级页——TabBar 仅在三主 Tab 路径渲染。
 * - 路由切换时滚动区复位到顶部，避免二级页承接首页的滚动位置。
 * - Toast：HeroUI 原生 toast（Android WebView 无 desktop WSL 的 view-transition 限制）。
 * `QueryClientProvider` 保持挂在 main.tsx（不动）。
 */
function AppContent() {
  const location = useLocation();
  const mainRef = useRef<HTMLElement | null>(null);
  const showTabBar = TABS.some((tab) => tab.to === location.pathname);

  useEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
  }, [location.pathname]);

  return (
    <div className="flex h-full min-h-0 flex-col bg-background text-foreground">
      <main ref={mainRef} className="min-h-0 flex-1 overflow-y-auto">
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/proxies" element={<Proxies />} />
          <Route path="/rules" element={<Rules />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </main>
      {showTabBar && <TabBar />}
    </div>
  );
}

export default function App() {
  if (!isTauriEnv) {
    return <TauriRequired />;
  }
  return (
    <HashRouter>
      <ToastProvider placement="top" maxVisibleToasts={3} />
      <AppContent />
    </HashRouter>
  );
}
