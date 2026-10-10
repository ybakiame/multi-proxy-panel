import { Component, type ErrorInfo, type ReactNode } from "react";

export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[ErrorBoundary] 页面渲染异常:", error, info);
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-4 bg-background p-6 text-foreground">
        <h1 className="text-xl font-semibold">页面渲染出错</h1>
        <p className="max-w-md break-all text-center text-sm text-muted">{this.state.error.message}</p>
        <button
          type="button"
          onClick={() => window.location.reload()}
          className="min-h-11 rounded-lg bg-primary px-4 text-white"
        >
          重新加载
        </button>
      </div>
    );
  }
}
