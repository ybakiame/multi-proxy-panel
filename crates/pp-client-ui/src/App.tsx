import { Component, useEffect, type ErrorInfo, type ReactNode } from "react";
import { useTheme } from "@heroui/react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { Toaster } from "./components/Toaster";
import { Sidebar } from "./layout/Sidebar";
import Dashboard from "./pages/Dashboard";
import Mitm from "./pages/Mitm";
import Nodes from "./pages/Nodes";
import Override from "./pages/Override";
import Scripts from "./pages/Scripts";
import Settings from "./pages/Settings";

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

export default function App() {
  return (
    <HashRouter>
      <ThemeBootstrap />
      <ErrorBoundary>
        {/*
          Toast 通知自实现（见 `./toast.ts` / `<Toaster />`）：HeroUI 3.2.2 的
          `ToastProvider` 渲染 toast 时调用 `document.startViewTransition()`，
          WebKitGTK 2.52.5 在 WSL 软渲染下会 SIGSEGV 直接退出进程，故弃用。
          Toaster 为纯静态渲染，独立挂载在应用内容之外。
        */}
        <Toaster />
        <div className="flex h-full min-h-screen bg-background text-foreground">
          <Sidebar />
          <main className="flex-1 overflow-y-auto p-6">
            <Routes>
              <Route path="/" element={<Dashboard />} />
              <Route path="/nodes" element={<Nodes />} />
              <Route path="/mitm" element={<Mitm />} />
              <Route path="/scripts" element={<Scripts />} />
              <Route path="/override" element={<Override />} />
              <Route path="/settings" element={<Settings />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </main>
        </div>
      </ErrorBoundary>
    </HashRouter>
  );
}
