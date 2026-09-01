import { Component, useEffect, useMemo, useState, type ErrorInfo, type ReactNode } from "react";
import { ToastProvider, useTheme } from "@heroui/react";
import { HashRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";
import { toastModeOverride } from "./api";
import { Toaster } from "./components/Toaster";
import { setToastMode } from "./toast";
import { useCapabilities } from "./hooks/useCapabilities";
import { DesktopSidebar } from "./layout/DesktopSidebar";
import { MobileTabBar } from "./layout/MobileTabBar";
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

/** 底部 TabBar 显式路由：仅这四个主 Tab 显示底部导航。 */
const MAIN_TAB_PATHS = ["/", "/proxies", "/tools", "/settings"];

/** 判断当前路由是否为主 Tab（底部 TabBar 应显示）。 */
function useIsMainTab(): boolean {
  const { pathname } = useLocation();
  return useMemo(() => MAIN_TAB_PATHS.includes(pathname), [pathname]);
}

/**
 * /mitm route guard: redirects to home on Android (where mitm capability is false).
 */
function MitmGuard() {
  const { data: caps } = useCapabilities();
  if (caps && !caps.capabilities.mitm) {
    return <Navigate to="/" replace />;
  }
  return <Mitm />;
}

/**
 * /scripts route guard: redirects to home on Android (where scripts_remote capability is false).
 */
function ScriptsGuard() {
  const { data: caps } = useCapabilities();
  if (caps && !caps.capabilities.scripts_remote) {
    return <Navigate to="/" replace />;
  }
  return <Scripts />;
}

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
 * 固定为深色主题：HeroUI v3 深色模式由 `<html>` 上的 `dark` / `data-theme="dark"`
 * 驱动（index.html 已静态设置），此处用 `useTheme` 钩子保证持久化后仍为深色。
 */
function ThemeBootstrap() {
  const { setTheme } = useTheme("dark");

  useEffect(() => {
    setTheme("dark");
  }, [setTheme]);

  return null;
}

/**
 * Toast 双态实现（详见 `./toast.ts`）：
 *
 * - 默认 HeroUI 原生 toast：`ToastProvider` 渲染 toast 时调用
 *   `document.startViewTransition()`，在 GPU 正常的桌面环境无问题。
 * - `PP_TOAST_MODE=static`（兼容 `safe`）环境变量强制保持自实现静态
 *   `<Toaster />`：仅供 WSL/WebKitGTK 等特殊环境使用——HeroUI 3.2.2 的
 *   view-transition 在 WebKitGTK 2.52.5 WSL 软渲染下会 SIGSEGV 直接退出进程
 *   （`startViewTransition` 风险详见 toast.ts 背景注释）。
 *
 * 命令返回前 / 命令失败 / 值非 `static`/`safe` 均保持默认（HeroUI 原生路径）。
 * `ToastProvider` 独立挂载（不带 children）——HeroUI 3.2.2 会把 children 当作
 * react-aria UNSTABLE_ToastRegion 的 render-prop 传入，无可见 toast 时 region
 * 返回 null，若用它包裹应用内容会导致整棵 UI 树渲染为空。
 */
/**
 * 应用内容层：依赖 Router context（useLocation），需在 HashRouter 内部渲染。
 *
 * 移动端安全区适配（Tauri Android 已 enableEdgeToEdge）：
 * - 顶部：主 Tab 页面保留原有标题区，子页由 MobileBackHeader 处理；
 *   整体容器加 `env(safe-area-inset-top)` padding（仅移动端 lg 以下）。
 * - 底部：主 Tab 页内容区需留出 TabBar 高度 + `env(safe-area-inset-bottom)`；
 *   子页仅需 safe-area-inset-bottom（无 TabBar）。桌面端不受影响。
 */
function AppContent() {
  const isMainTab = useIsMainTab();

  return (
    <div className="flex h-full min-h-screen bg-background text-foreground">
      <DesktopSidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <main
          className="flex-1 overflow-y-auto p-4 lg:p-6"
          style={{
            paddingTop: "max(1rem, env(safe-area-inset-top))",
            paddingBottom: isMainTab
              ? "calc(5rem + env(safe-area-inset-bottom))"
              : "max(1rem, env(safe-area-inset-bottom))",
          }}
        >
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/proxies" element={<Proxies />} />
            <Route path="/nodes" element={<Nodes />} />
            <Route path="/tools" element={<Tools />} />
            <Route path="/rules" element={<Rules />} />
            <Route path="/connections" element={<Connections />} />
            <Route path="/mitm" element={<MitmGuard />} />
            <Route path="/scripts" element={<ScriptsGuard />} />
            <Route path="/override" element={<Override />} />
            <Route path="/logs" element={<Logs />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </main>
        {isMainTab && <MobileTabBar />}
      </div>
    </div>
  );
}

export default function App() {
  const [heroToastEnabled, setHeroToastEnabled] = useState(true);

  useEffect(() => {
    let cancelled = false;
    toastModeOverride()
      .then((mode) => {
        if (cancelled) {
          return;
        }
        const staticMode =
          (mode ?? "").trim().toLowerCase() === "static" || (mode ?? "").trim().toLowerCase() === "safe";
        setHeroToastEnabled(!staticMode);
        setToastMode(!staticMode);
      })
      .catch(() => {
        // 命令失败保持默认：heroToastEnabled=true + heroMode=true（HeroUI 原生路径）。
      });
    return () => {
      cancelled = true;
    };
  }, []);

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
