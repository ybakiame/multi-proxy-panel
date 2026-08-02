import { Component, useEffect, useState, type ErrorInfo, type ReactNode } from "react";
import { ToastProvider, useTheme } from "@heroui/react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { gpuAcceleration } from "./api";
import { Toaster } from "./components/Toaster";
import { setToastMode } from "./toast";
import { MobileNav } from "./layout/MobileNav";
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

/**
 * Toast 按渲染能力选择实现（详见 `./toast.ts`）：
 *
 * - 有 GPU 加速（`gpu_acceleration` 命令返回 `true`）：挂载 HeroUI `ToastProvider`，
 *   用原生动画 toast。
 * - 无 GPU（WSL 无 WSLg 直通或强制软渲染）：挂载自实现 `<Toaster />`（fallback，
 *   保留不删）。背景：HeroUI 3.2.2 的 ToastProvider 渲染 toast 时调用
 *   `document.startViewTransition()`，WebKitGTK 2.52.5 在 WSL 软渲染下执行
 *   view-transition 会 SIGSEGV 直接退出进程。
 *
 * 默认 `false`（自实现）为保守安全：命令返回前不挂载 HeroUI toast，命令失败
 * 时保持自实现路径。`ToastProvider` 独立挂载（不带 children）——HeroUI 3.2.2
 * 会把 children 当作 react-aria UNSTABLE_ToastRegion 的 render-prop 传入，无可见
 * toast 时 region 返回 null，若用它包裹应用内容会导致整棵 UI 树渲染为空。
 */
export default function App() {
  const [heroToastEnabled, setHeroToastEnabled] = useState(false);

  useEffect(() => {
    let cancelled = false;
    gpuAcceleration()
      .then((hasGpu) => {
        if (cancelled) {
          return;
        }
        setHeroToastEnabled(hasGpu);
        setToastMode(hasGpu);
      })
      .catch(() => {
        // 命令失败保持默认：heroToastEnabled=false + heroMode=false（自实现路径）。
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
        <div className="flex h-full min-h-screen bg-background text-foreground">
          <Sidebar />
          <div className="flex min-w-0 flex-1 flex-col">
            <MobileNav />
            <main className="flex-1 overflow-y-auto p-4 lg:p-6">
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
        </div>
      </ErrorBoundary>
    </HashRouter>
  );
}
