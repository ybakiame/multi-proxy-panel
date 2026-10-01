import { Component, useEffect, useState, type ErrorInfo, type ReactNode } from "react";
import { ToastProvider, useTheme, toast as heroToast } from "@heroui/react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { toastModeOverride } from "@pp/client-core";
import { Toaster } from "./components/Toaster";
import { RestartPrompt } from "./components/RestartPrompt";
import { isTauriEnv } from "@pp/client-core";
import { setToastHandler } from "@pp/client-core";
import { DesktopSidebar } from "./layout/desktop/DesktopSidebar";
import Dashboard from "./pages/Dashboard";
import Logs from "./pages/Logs";
import Mitm from "./pages/Mitm";
import Nodes from "./pages/Nodes";
import Override from "./pages/Override";
import Proxies from "./pages/Proxies";
import Rules from "./pages/Rules";
import Scripts from "./pages/Scripts";
import Settings from "./pages/Settings";
import Tools from "./pages/Tools";
import Connections from "./pages/Connections";
import Stats from "./pages/Stats";
import Config from "./pages/Config";
import DnsPage from "./pages/Config/Dns";
import OutboundsPage from "./pages/Config/Outbounds";
import RoutePage from "./pages/Config/Route";
import ExperimentalPage from "./pages/Config/Experimental";

/**
 * 渲染期错误兜底：捕获子组件渲染时的未处理异常，展示错误信息与
 * 「重新加载」按钮，避免页面异常后整页黑屏/白屏无法恢复。
 *
 * 错误路径刻意不依赖 HeroUI 组件（若异常来自 HeroUI 本身会二次崩溃），
 * 使用原生 button + Tailwind 类渲染。
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

/**
 * 主题同步（浅色 / 深色 / 跟随系统）：HeroUI v3 `useTheme` 读取 localStorage
 * `heroui-theme`（默认 `system`），监听系统深浅色变化并同步 `<html>` 的
 * class / `data-theme`；index.html 预置脚本负责首帧前就位，此组件挂载后接管。
 */
function ThemeBootstrap() {
  useTheme("system");

  return null;
}

/**
 * Toast 双态实现（详见 `@pp/client-core` 的 toast.ts 适配器模式）：
 *
 * - 默认 HeroUI 原生 toast：启动时经 `setToastHandler` 注册 `heroToast` 接管全部
 *   toast 输出；`ToastProvider` 渲染 toast 时调用 `document.startViewTransition()`，
 *   在 GPU 正常的桌面环境无问题。
 * - `PP_TOAST_MODE=static`（兼容 `safe`）环境变量下不注册 handler，toast 落入
 *   client-core 的 zustand 静态 store，由自实现 `<Toaster />` 消费：仅供
 *   WSL/WebKitGTK 等特殊环境使用——HeroUI 3.2.2 的 view-transition 在 WebKitGTK
 *   2.52.5 WSL 软渲染下会 SIGSEGV 直接退出进程（背景详见 client-core toast.ts）。
 *
 * 命令返回前 / 命令失败 / 值非 `static`/`safe` 均保持默认（HeroUI 原生路径）。
 * `ToastProvider` 独立挂载（不带 children）——HeroUI 3.2.2 会把 children 当作
 * react-aria UNSTABLE_ToastRegion 的 render-prop 传入，无可见 toast 时 region
 * 返回 null，若用它包裹应用内容会导致整棵 UI 树渲染为空。
 */
/**
 * 应用内容层：依赖 Router context（useLocation），需在 HashRouter 内部渲染。
 * 桌面端统一由侧边栏主导航，主内容区使用常规内边距布局。
 */
function AppContent() {
  return (
    <div className="flex h-full min-h-screen bg-background text-foreground">
      <DesktopSidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <main className="flex-1 overflow-y-auto p-4 lg:p-6">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/proxies" element={<Proxies />} />
            <Route path="/nodes" element={<Nodes />} />
            <Route path="/tools" element={<Tools />} />
            <Route path="/rules" element={<Rules />} />
            <Route path="/connections" element={<Connections />} />
            <Route path="/stats" element={<Stats />} />
            <Route path="/mitm" element={<Mitm />} />
            <Route path="/scripts" element={<Scripts />} />
            <Route path="/override" element={<Override />} />
            <Route path="/config" element={<Config />} />
            <Route path="/config/dns" element={<DnsPage />} />
            <Route path="/config/outbounds" element={<OutboundsPage />} />
            <Route path="/config/route" element={<RoutePage />} />
            <Route path="/config/experimental" element={<ExperimentalPage />} />
            <Route path="/logs" element={<Logs />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </main>
        <RestartPrompt />
      </div>
    </div>
  );
}

export default function App() {
  const [heroToastEnabled, setHeroToastEnabled] = useState(true);

  useEffect(() => {
    // 非 Tauri 环境（浏览器直接打开 devUrl）无 IPC 桥，跳过命令调用。
    if (!isTauriEnv) {
      return;
    }
    let cancelled = false;
    toastModeOverride()
      .then((mode) => {
        if (cancelled) {
          return;
        }
        const staticMode =
          (mode ?? "").trim().toLowerCase() === "static" || (mode ?? "").trim().toLowerCase() === "safe";
        setHeroToastEnabled(!staticMode);
        // 静态模式不注册 handler，toast 落入 client-core 静态 store 由 <Toaster /> 消费。
        setToastHandler(staticMode ? null : (kind, message) => heroToast[kind](message));
      })
      .catch(() => {
        // 命令失败保持默认：heroToastEnabled=true + 注册 HeroUI 适配器（原生路径）。
        if (!cancelled) {
          setToastHandler((kind, message) => heroToast[kind](message));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 浏览器直接打开 devUrl 时 Tauri 未注入 __TAURI_INTERNALS__，任何 invoke 都会抛
  // "Cannot read properties of undefined (reading 'invoke')"；拦截渲染并给出引导，
  // 避免各页面 Query 轮询反复失败、错误 Alert 刷屏。刻意用原生元素渲染（与 ErrorBoundary 同理）。
  if (!isTauriEnv) {
    return (
      <div className="flex h-screen w-full flex-col items-center justify-center gap-4 bg-background p-6 text-foreground">
        <h1 className="text-xl font-semibold">请在客户端内运行</h1>
        <p className="max-w-md text-center text-sm text-muted">
          当前页面通过浏览器直接访问，缺少 Tauri 运行环境，所有本地命令不可用。 请使用{" "}
          <code className="rounded bg-default-100 px-1 py-0.5">bun run tauri dev</code> 启动桌面客户端，或运行已构建的
          ProxyPanel 应用。
        </p>
      </div>
    );
  }

  return (
    <HashRouter>
      <ThemeBootstrap />
      <ErrorBoundary>
        {heroToastEnabled ? <ToastProvider placement="bottom end" maxVisibleToasts={3} /> : <Toaster />}
        <AppContent />
      </ErrorBoundary>
    </HashRouter>
  );
}
