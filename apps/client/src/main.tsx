// 浏览器调试 mock 必须先于所有 @pp/client-core / App 导入执行（env.ts 的
// isTauriEnv 在模块加载时求值），故保持为首行 import。
import "./browser-debug";
import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { installLogCapture } from "@pp/client-core";
import App from "./App";
import "./index.css";

// 挂载 React 前先接入日志管道：应用早期（模块初始化/渲染）的 console 错误
// 也能被捕获转发到后端。
installLogCapture();

// QueryClient 模块级单例（双端一致）：默认配置 retry 1 / 失焦不重拉 / 30s stale，
// 供各页面 Query 轮询共享同一缓存。
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
      staleTime: 30_000,
    },
  },
});

// 页面与路由共用；平台呈现由 @pp/ui 的编译期常量决定。
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
