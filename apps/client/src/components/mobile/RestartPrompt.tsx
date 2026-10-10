import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ArrowPathIcon } from "@heroicons/react/24/outline";
import {
  PROXY_STATUS_KEY,
  proxyStatus,
  startProxy,
  stopProxy,
  toErrorMessage,
  toastError,
  toastSuccess,
  usePendingRestartStore,
  useProxyStatus,
} from "@pp/client-core";
import { BottomSheet, Button, ConfirmDialog } from "@pp/ui";

/**
 * 全局配置变更重启提示（设计：docs/plans/2026-09-30-mobile-restart-prompt-design.md）。
 *
 * - 消费 client-core `usePendingRestartStore`（各保存成功路径上报脏顶级配置域）；
 * - 弹窗一次：脏标记非空 + 核心运行中 + 未 dismissed → ConfirmDialog 列出变更项，
 *   「立即重启」/「稍后」（稍后 = dismiss，收为悬浮按钮）；
 * - 悬浮按钮：右下角固定（避让启停 FAB 与悬浮 TabBar），点击开 BottomSheet
 *   （变更项列表 + 立即重启）；
 * - 重启链路：stopProxy → startProxy（已授权不再触发 VPN 授权），成功 reset + toast；
 *   失败 toast 并保留脏标记；
 * - 核心由运行转停止（手动停/异常）时 effect 复位脏标记与 dismissed。
 */
export function RestartPrompt() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const coreRunning = status?.core_running ?? false;

  const dirtyKeys = usePendingRestartStore((s) => s.dirtyKeys);
  const dismissed = usePendingRestartStore((s) => s.dismissed);
  const dismiss = usePendingRestartStore((s) => s.dismiss);
  const reset = usePendingRestartStore((s) => s.reset);

  const [sheetOpen, setSheetOpen] = useState(false);
  const [restarting, setRestarting] = useState(false);

  // 核心由运行转停止：脏配置随下次启动生效，提示使命结束，复位。
  const prevRunningRef = useRef(coreRunning);
  useEffect(() => {
    if (prevRunningRef.current && !coreRunning) {
      reset();
      setSheetOpen(false);
    }
    prevRunningRef.current = coreRunning;
  }, [coreRunning, reset]);

  const dirtyLabels = [...dirtyKeys.values()];
  const hasPending = dirtyLabels.length > 0 && coreRunning;
  const dialogOpen = hasPending && !dismissed;

  /**
   * 等待核心真正停止（Kotlin 侧停止为异步：close 在线程中释放 TUN）。
   * stopProxy 返回即 start 会与旧实例抢 TUN 资源导致新核心启动失败，故轮询
   * `proxy_status` 确认停止后再拉起（真机实测踩坑）。
   */
  const waitCoreStopped = async (): Promise<void> => {
    for (let i = 0; i < 20; i++) {
      const status = await proxyStatus();
      if (!status.core_running) {
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 300));
    }
    // 超时仍按已停止继续（start 失败走下方错误提示，不无限等待）。
  };

  const handleRestart = async () => {
    if (restarting) {
      return;
    }
    setRestarting(true);
    try {
      const stopped = await stopProxy();
      queryClient.setQueryData(PROXY_STATUS_KEY, stopped);
      await waitCoreStopped();
      const started = await startProxy();
      queryClient.setQueryData(PROXY_STATUS_KEY, started);
      reset();
      setSheetOpen(false);
      toastSuccess("代理已重启，配置已生效");
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setRestarting(false);
    }
  };

  if (!hasPending) {
    return null;
  }

  return (
    <>
      {/* 弹窗一次（本脏周期内） */}
      <ConfirmDialog
        opened={dialogOpen}
        title="配置已变更"
        confirmText="立即重启"
        cancelText="稍后"
        busy={restarting}
        onConfirm={() => void handleRestart()}
        onClose={dismiss}
      >
        <p>以下配置修改需重启代理后生效：</p>
        <ul className="mt-1 list-disc pl-5">
          {dirtyLabels.map((label) => (
            <li key={label}>{label}</li>
          ))}
        </ul>
      </ConfirmDialog>

      {/* 悬浮按钮（弹窗收起后承接） */}
      {dismissed && (
        <button
          type="button"
          aria-label="查看待生效的配置变更"
          onClick={() => setSheetOpen(true)}
          className="fixed right-4 z-20 flex h-11 items-center gap-1.5 rounded-full bg-amber-500 px-4 text-sm font-medium text-black shadow-lg shadow-black/25 active:opacity-80"
          style={{ bottom: "calc(10.5rem + env(safe-area-inset-bottom))" }}
        >
          <ArrowPathIcon className="size-5" aria-hidden="true" />
          配置待重启
        </button>
      )}

      {/* 变更项列表 BottomSheet */}
      <BottomSheet
        opened={sheetOpen}
        onClose={() => setSheetOpen(false)}
        title="待生效的配置变更"
        footer={
          <Button
            variant="primary"
            className="min-h-12 w-full"
            isPending={restarting}
            onPress={() => void handleRestart()}
          >
            立即重启
          </Button>
        }
      >
        <div className="flex flex-col gap-3">
          <p className="text-sm text-zinc-500 dark:text-zinc-400">以下配置修改需重启代理后生效：</p>
          <ul className="flex flex-col gap-2">
            {dirtyLabels.map((label) => (
              <li
                key={label}
                className="flex items-center gap-2 rounded-lg bg-zinc-100 px-3 py-2.5 text-sm dark:bg-zinc-800"
              >
                <span className="size-1.5 shrink-0 rounded-full bg-amber-500" aria-hidden="true" />
                {label}
              </li>
            ))}
          </ul>
        </div>
      </BottomSheet>
    </>
  );
}
