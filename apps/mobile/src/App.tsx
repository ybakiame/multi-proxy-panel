import { Component, useEffect, useRef, type ErrorInfo, type ReactNode } from "react";
import { ToastProvider } from "@heroui/react";
import { HashRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";
import { isTauriEnv } from "@pp/client-core";
import { TABS, TabBar } from "./components/TabBar";
import Dashboard from "./pages/Dashboard";
import Proxies from "./pages/Proxies";
import Rules from "./pages/Rules";
import CustomRulesPage from "./pages/Rules/CustomRulesPage";
import RuleSetsPage from "./pages/Rules/RuleSetsPage";
import Settings from "./pages/Settings";
import Subscriptions from "./pages/Subscriptions";

/**
 * 渲染期错误兜底：捕获子组件渲染时的未处理异常，展示错误信息与
 * 「重新加载」按钮，避免页面异常后整页黑屏无法恢复（对齐 desktop App.tsx）。
 *
 * 错误路径刻意不依赖 HeroUI 组件（若异常来自 HeroUI 本身会二次崩溃），
 * 使用原生 button + Tailwind 类渲染；深色背景直接取 `bg-background`/`text-foreground`。
 */
interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[ErrorBoundary] 页面渲染异常:", error, info);
  }

  private handleReload = () => {
    window.location.reload();
  };

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-screen w-full flex-col items-center justify-center gap-4 bg-background p-6 text-foreground">
          <h1 className="text-xl font-semibold">页面渲染出错</h1>
          <p className="max-w-md break-all text-center text-sm text-muted">{this.state.error.message}</p>
          <button
            type="button"
            className="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-white hover:opacity-90"
            onClick={this.handleReload}
          >
            重新加载
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

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
 * - 路由：`/` 首页仪表盘、`/rules` 规则管理（Tab）、`/settings` 设置（Tab）；
 *   `/proxies` 代理选择页、`/subscriptions` 订阅管理页、`/rules/custom` 自定义规则、
 *   `/rules/rulesets` 规则集管理为非 Tab 二级页——TabBar 仅在三主 Tab 路径渲染。
 * - 路由切换时滚动区复位到顶部，避免二级页承接首页的滚动位置。
 * - Toast：HeroUI 原生 toast（Android WebView 无 desktop WSL 的 view-transition 限制）。
 *   edge-to-edge 下状态栏透明，toast region（`placement="top"` 定位于 `top-4`）需额外让出
 *   `env(safe-area-inset-top)`：通过 `ToastProvider` 的 `className`（透传至 region 并参与
 *   slots.region 合并）追加 utilities 层任意值类覆盖其 `top`。
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
          <Route path="/subscriptions" element={<Subscriptions />} />
          <Route path="/rules" element={<Rules />} />
          <Route path="/rules/custom" element={<CustomRulesPage />} />
          <Route path="/rules/rulesets" element={<RuleSetsPage />} />
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
      <ErrorBoundary>
        <ToastProvider placement="top" maxVisibleToasts={3} className="top-[max(1rem,env(safe-area-inset-top))]" />
        <AppContent />
      </ErrorBoundary>
    </HashRouter>
  );
}
