import { isTauriEnv } from "@pp/client-core";

/**
 * 移动端骨架占位首页（M3 第一步纯前端骨架）。
 *
 * 浏览器直接打开 devUrl 时 Tauri 未注入 __TAURI_INTERNALS__，缺少 IPC 桥，
 * 任何 invoke 都会失败；拦截渲染并给出引导（与 desktop App.tsx 同理）。
 * 页面内容刻意使用原生元素 + Tailwind 类，不引入 Router/Query 等本任务用不到的设施。
 * 真正的首页将在 M3.6 实现。
 */
export default function App() {
  if (!isTauriEnv) {
    return (
      <div className="flex h-screen w-full flex-col items-center justify-center gap-4 bg-background p-6 text-foreground">
        <h1 className="text-xl font-semibold">请在客户端内运行</h1>
        <p className="max-w-md text-center text-sm text-muted">
          当前页面通过浏览器直接访问，缺少 Tauri 运行环境，所有本地命令不可用。 请使用{" "}
          <code className="rounded bg-default-100 px-1 py-0.5">bun run tauri dev</code> 启动移动客户端（端口
          1430），或运行已构建的 ProxyPanel 应用。
        </p>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-background p-6 text-center text-foreground">
      <h1 className="text-2xl font-semibold">ProxyPanel Mobile</h1>
      <p className="text-sm text-muted">移动端骨架占位，首页将于 M3.6 实现。</p>
    </div>
  );
}
