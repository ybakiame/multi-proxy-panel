import { useEffect, useState } from "react";
import { MinusIcon, Square2StackIcon, StopIcon, XMarkIcon } from "@heroicons/react/24/outline";
import { clientWindow, toastError, toErrorMessage, useCapabilities } from "@pp/client-core";

/** Windows 独占标题栏；系统窗口的 resize/close 语义保留。 */
export function WindowTitlebar() {
  const { data: capabilities } = useCapabilities();
  const [maximized, setMaximized] = useState(false);
  const windows = capabilities?.os === "windows";
  useEffect(() => {
    if (!windows) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const window = clientWindow();
    const update = async () => {
      const value = await window.isMaximized();
      if (!disposed) setMaximized(value);
    };
    void update().catch((error) => toastError(toErrorMessage(error)));
    void window
      .onResized(() => void update().catch((error) => toastError(toErrorMessage(error))))
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error) => toastError(toErrorMessage(error)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [windows]);
  if (!windows) return null;
  const run = (action: () => Promise<void>) => void action().catch((error) => toastError(toErrorMessage(error)));
  return (
    <header className="flex h-10 shrink-0 select-none items-center border-b border-border bg-surface">
      <div data-tauri-drag-region className="flex h-full flex-1 items-center px-4 text-xs font-medium">
        <span className="pointer-events-none">ProxyPanel</span>
      </div>
      <button
        type="button"
        aria-label="最小化"
        className="flex h-full w-12 items-center justify-center hover:bg-surface-secondary focus-visible:outline-2 focus-visible:outline-focus"
        onClick={() => run(() => clientWindow().minimize())}
      >
        <MinusIcon className="size-4" aria-hidden="true" />
      </button>
      <button
        type="button"
        aria-label={maximized ? "还原窗口" : "最大化"}
        className="flex h-full w-12 items-center justify-center hover:bg-surface-secondary focus-visible:outline-2 focus-visible:outline-focus"
        onClick={() => run(() => clientWindow().toggleMaximize())}
      >
        {maximized ? (
          <Square2StackIcon className="size-4" aria-hidden="true" />
        ) : (
          <StopIcon className="size-4" aria-hidden="true" />
        )}
      </button>
      <button
        type="button"
        aria-label="关闭窗口"
        className="flex h-full w-12 items-center justify-center hover:bg-danger hover:text-danger-foreground focus-visible:outline-2 focus-visible:outline-focus"
        onClick={() => run(() => clientWindow().close())}
      >
        <XMarkIcon className="size-4" aria-hidden="true" />
      </button>
    </header>
  );
}
