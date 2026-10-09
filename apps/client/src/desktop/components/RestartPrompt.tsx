import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Modal } from "@heroui/react";
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

/**
 * 全局配置变更重启提示（桌面端；设计：docs/plans/2026-09-30-mobile-restart-prompt-design.md，
 * 移动端实现见 apps/mobile/src/components/RestartPrompt.tsx）。
 *
 * - 消费 client-core `usePendingRestartStore`（各保存成功路径上报脏顶级配置域）；
 * - 弹窗一次：脏标记非空 + 核心运行中 + 未 dismissed → AlertDialog 列出变更项，
 *   「立即重启」/「稍后」（稍后 = dismiss，收为右下角悬浮按钮）；
 * - 悬浮按钮：点击开 Modal（变更项列表 + 立即重启）；
 * - 重启链路：stopProxy → 轮询确认核心停止 → startProxy，成功 reset + toast；
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

  const [listOpen, setListOpen] = useState(false);
  const [restarting, setRestarting] = useState(false);

  // 核心由运行转停止：脏配置随下次启动生效，提示使命结束，复位。
  const prevRunningRef = useRef(coreRunning);
  useEffect(() => {
    if (prevRunningRef.current && !coreRunning) {
      reset();
      setListOpen(false);
    }
    prevRunningRef.current = coreRunning;
  }, [coreRunning, reset]);

  const dirtyLabels = [...dirtyKeys.values()];
  const hasPending = dirtyLabels.length > 0 && coreRunning;
  const dialogOpen = hasPending && !dismissed;

  /**
   * 等待核心真正停止后再拉起。桌面侧 spawn 子进程停止通常同步返回，但保险起见
   * 仍轮询 `proxy_status` 确认 core_running=false 再 start（对齐移动端踩坑结论），
   * 超时兜底继续走错误提示。
   */
  const waitCoreStopped = async (): Promise<void> => {
    for (let i = 0; i < 20; i++) {
      const current = await proxyStatus();
      if (!current.core_running) {
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 300));
    }
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
      setListOpen(false);
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
      <AlertDialog.Backdrop
        isOpen={dialogOpen}
        onOpenChange={(open) => {
          if (!open) {
            dismiss();
          }
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="warning" />
              <AlertDialog.Heading>配置已变更</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p>以下配置修改需重启代理后生效：</p>
              <ul className="mt-1 list-disc pl-5">
                {dirtyLabels.map((label) => (
                  <li key={label}>{label}</li>
                ))}
              </ul>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={dismiss} isDisabled={restarting}>
                稍后
              </Button>
              <Button variant="primary" onPress={() => void handleRestart()} isPending={restarting}>
                立即重启
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>

      {/* 悬浮按钮（弹窗收起后承接） */}
      {dismissed && (
        <button
          type="button"
          aria-label="查看待生效的配置变更"
          onClick={() => setListOpen(true)}
          className="fixed bottom-6 right-6 z-20 flex h-11 items-center gap-1.5 rounded-full bg-amber-500 px-4 text-sm font-medium text-black shadow-lg shadow-black/25 hover:opacity-90"
        >
          <ArrowPathIcon className="size-5" aria-hidden="true" />
          配置待重启
        </button>
      )}

      {/* 变更项列表 Modal */}
      <Modal.Backdrop
        isOpen={listOpen}
        onOpenChange={(open) => {
          if (!open) {
            setListOpen(false);
          }
        }}
      >
        <Modal.Container>
          <Modal.Dialog className="sm:max-w-[420px]">
            <Modal.CloseTrigger />
            <Modal.Header>
              <Modal.Heading>待生效的配置变更</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="flex flex-col gap-3">
              <p className="text-sm text-muted">以下配置修改需重启代理后生效：</p>
              <ul className="flex flex-col gap-2">
                {dirtyLabels.map((label) => (
                  <li
                    key={label}
                    className="flex items-center gap-2 rounded-lg bg-surface-secondary/60 px-3 py-2.5 text-sm"
                  >
                    <span className="size-1.5 shrink-0 rounded-full bg-amber-500" aria-hidden="true" />
                    {label}
                  </li>
                ))}
              </ul>
            </Modal.Body>
            <Modal.Footer>
              <Button variant="primary" className="w-full" onPress={() => void handleRestart()} isPending={restarting}>
                立即重启
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </>
  );
}
