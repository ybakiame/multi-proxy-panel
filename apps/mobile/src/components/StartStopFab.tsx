import { PlayIcon, StopIcon } from "@heroicons/react/24/solid";
import { Button, Spinner } from "@heroui/react";

interface StartStopFabProps {
  /** 核心是否运行中：决定形态（未运行主色启动 / 运行中 danger 停止）。 */
  running: boolean;
  /** 启动链路 busy（`startMutation.isPending || vpnAuthMutation.isPending`）。 */
  starting: boolean;
  /** 停止请求进行中（`stopMutation.isPending`）。 */
  stopping: boolean;
  /** 是否存在生效订阅；无生效订阅且未运行时禁用启动。 */
  canStart: boolean;
  /** 点击启动（复用 Dashboard `handleStart`）。 */
  onStart: () => void;
  /** 点击停止（复用 Dashboard `handleStop`）。 */
  onStop: () => void;
}

/**
 * 首页悬浮启停按钮（FAB，ADR-0003 M5）。
 *
 * - `fixed` 定位于右下角：`right-4`，`bottom` 为 TabBar 高度（`min-h-14` = 3.5rem）+
 *   `env(safe-area-inset-bottom)` + 1rem 间距，悬浮于 TabBar 之上且不重叠；
 * - 尺寸 `size-14`（56px）圆形，带阴影；
 * - 状态：未运行主色 + `PlayIcon`（无生效订阅时禁用）；运行中 danger + `StopIcon`；
 * - pending 时显示 `Spinner` 并禁用（`isPending` 同时阻断点击）；
 * - `aria-label` 按状态给出「启动代理」/「停止代理」，触达区 ≥44px。
 *
 * 启停/授权链路逻辑仍由 Dashboard 持有，本组件仅负责呈现与点击转发。
 */
export function StartStopFab({ running, starting, stopping, canStart, onStart, onStop }: StartStopFabProps) {
  const pending = running ? stopping : starting;
  const disabled = running ? starting : !canStart || starting || stopping;

  return (
    <Button
      isIconOnly
      variant={running ? "danger" : "primary"}
      aria-label={running ? "停止代理" : "启动代理"}
      isPending={pending}
      isDisabled={disabled}
      onPress={() => void (running ? onStop() : onStart())}
      className="fixed right-4 z-20 size-14 rounded-full shadow-lg shadow-black/25"
      style={{ bottom: "calc(4.5rem + env(safe-area-inset-bottom))" }}
    >
      {pending ? (
        <Spinner size="sm" color="current" aria-hidden="true" />
      ) : running ? (
        <StopIcon className="size-6" aria-hidden="true" />
      ) : (
        <PlayIcon className="size-6" aria-hidden="true" />
      )}
    </Button>
  );
}
