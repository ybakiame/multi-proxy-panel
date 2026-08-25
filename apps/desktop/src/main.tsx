import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { installLogCapture } from "./logCapture";

// 挂载 React 前先接入日志管道：应用早期（模块初始化/渲染）的 console 错误
// 也能被捕获转发到后端。
installLogCapture();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
