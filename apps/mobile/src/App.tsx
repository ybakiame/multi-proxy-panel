import { Component, useEffect, useRef, type ErrorInfo, type ReactNode } from "react";
import { ToastProvider } from "@heroui/react";
import { HashRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";
import { isTauriEnv } from "@pp/client-core";
import { TABS, TabBar } from "./components/TabBar";
import Dashboard from "./pages/Dashboard";
import Logs from "./pages/Logs";
import Panel from "./pages/Panel";
import Config from "./pages/Config";
import CustomRulesPage from "./pages/Config/CustomRulesPage";
import DnsPage from "./pages/Config/Dns";
import ExperimentalPage from "./pages/Config/ExperimentalPage";
import NetworkPage from "./pages/Config/NetworkPage";
import OutboundsPage from "./pages/Config/Outbounds";
import RoutePage from "./pages/Config/Route";
import RuleSetMarket from "./pages/Config/RuleSetMarket";
import RuleSetsPage from "./pages/Config/RuleSetsPage";
import Settings from "./pages/Settings";
import AboutPage from "./pages/Settings/AboutPage";
import DevToolsPage from "./pages/Settings/DevToolsPage";
import GithubPage from "./pages/Settings/GithubPage";
import VpnNotifyPage from "./pages/Settings/VpnNotifyPage";
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
            className="min-h-11 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-white hover:opacity-90"
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
 * - 路由：`/` 首页仪表盘、`/config` 配置管理（Tab）、`/settings` 设置（Tab）；
 *   `/panel` Clash 面板页（内嵌 zashboard）、`/logs` 日志页、`/subscriptions` 订阅管理页、
 *   `/config/route` 路由、`/config/route/rules` 规则管理、`/config/route/rulesets` 规则集管理、
 *   `/config/dns`、`/config/outbounds`、
 *   `/config/network`、`/config/experimental` 为非 Tab 二级页——TabBar 仅在三主 Tab 路径渲染；
 *   旧 `/config/clash-api` 重定向到 `/config/experimental`（Clash API 设置已并入）。
 * - 旧 `/rules/*` 路径与旧 `/settings/{network,clash-api}` 路径保留 `<Navigate>` 重定向；
 *   旧 `/config/{rules,rulesets}` 路径重定向到 `/config/route/*`（规则管理迁入路由菜单）；
 *   旧 `/proxies`、`/connections` 路径重定向到 `/panel`（原生页面已由内嵌面板取代，
 *   HashRouter 存量书签兼容）。
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
          <Route path="/panel" element={<Panel />} />
          {/* 原生代理/连接页已由内嵌面板取代：旧路径重定向兼容（HashRouter 存量书签）。 */}
          <Route path="/connections" element={<Navigate to="/panel" replace />} />
          <Route path="/proxies" element={<Navigate to="/panel" replace />} />
          <Route path="/subscriptions" element={<Subscriptions />} />
          <Route path="/config" element={<Config />} />
          <Route path="/config/dns" element={<DnsPage />} />
          <Route path="/config/outbounds" element={<OutboundsPage />} />
          <Route path="/config/network" element={<NetworkPage />} />
          <Route path="/config/experimental" element={<ExperimentalPage />} />
          {/* Clash API 设置已并入 Experimental 页 */}
          <Route path="/config/clash-api" element={<Navigate to="/config/experimental" replace />} />
          <Route path="/config/route" element={<RoutePage />} />
          <Route path="/config/route/rules" element={<CustomRulesPage />} />
          <Route path="/config/route/rulesets" element={<RuleSetsPage />} />
          <Route path="/config/route/rulesets/market" element={<RuleSetMarket />} />
          {/* 规则管理 / 规则集管理迁入路由菜单：旧 /config/{rules,rulesets} 路径重定向兼容。 */}
          <Route path="/config/rules" element={<Navigate to="/config/route/rules" replace />} />
          <Route path="/config/rulesets" element={<Navigate to="/config/route/rulesets" replace />} />
          <Route path="/config/rulesets/market" element={<Navigate to="/config/route/rulesets/market" replace />} />
          {/* 旧 /rules/* 路径重定向：HashRouter 下存量书签 / 导航兼容（ADR-0005 §3.3）。 */}
          <Route path="/rules" element={<Navigate to="/config" replace />} />
          <Route path="/rules/custom" element={<Navigate to="/config/route/rules" replace />} />
          <Route path="/rules/rulesets" element={<Navigate to="/config/route/rulesets" replace />} />
          <Route path="/rules/rulesets/market" element={<Navigate to="/config/route/rulesets/market" replace />} />
          {/* 网络 / Clash API 迁移为配置页必选切片：旧设置页路径重定向兼容。 */}
          <Route path="/settings/network" element={<Navigate to="/config/network" replace />} />
          <Route path="/settings/clash-api" element={<Navigate to="/config/experimental" replace />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/settings/vpn-notify" element={<VpnNotifyPage />} />
          <Route path="/settings/github" element={<GithubPage />} />
          <Route path="/settings/dev-tools" element={<DevToolsPage />} />
          <Route path="/settings/about" element={<AboutPage />} />
          <Route path="/logs" element={<Logs />} />
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
