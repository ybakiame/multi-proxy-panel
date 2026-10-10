import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { Button, BottomSheet } from "@pp/ui";
import {
  CONFIG_KEY,
  SUBSCRIPTIONS_KEY,
  markRestartRequired,
  toErrorMessage,
  toastError,
  toastSuccess,
  useProxyStatus,
  activateSubscription,
} from "@pp/client-core";
import type { SubscriptionView } from "@pp/client-core";

interface SubscriptionSheetProps {
  isOpen: boolean;
  onClose: () => void;
  /** 全部订阅；旧版停用字段在用户选择时恢复。 */
  subscriptions: SubscriptionView[];
  activeSubscriptionId: string | null;
}

/** 选择生效订阅；切换后标记重启并刷新双端共享查询。 */
export function SubscriptionSheet({ isOpen, onClose, subscriptions, activeSubscriptionId }: SubscriptionSheetProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const coreRunning = status?.core_running ?? false;
  const [selecting, setSelecting] = useState(false);

  const selectSubscription = async (sub: SubscriptionView) => {
    if (sub.id === activeSubscriptionId && sub.enabled) {
      onClose();
      return;
    }
    setSelecting(true);
    try {
      await activateSubscription(sub);
      // 核心运行中切换生效订阅：节点集变更需重启核心才生效，上报全局重启提示。
      markRestartRequired("subscription", coreRunning);
      toastSuccess("生效订阅已切换");
      onClose();
      void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
    } catch (err) {
      const message = toErrorMessage(err);
      toastError(message);
      // 保存失败配置未落盘：失效 CONFIG_KEY 回滚重读，避免缓存停留在补丁值。
      void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
    } finally {
      setSelecting(false);
    }
  };

  return (
    <BottomSheet
      opened={isOpen}
      onClose={onClose}
      title="选择生效订阅"
      footer={
        <Button
          variant="secondary"
          className="min-h-11 w-full"
          onPress={() => {
            onClose();
            navigate("/subscriptions");
          }}
        >
          管理订阅
        </Button>
      }
    >
      <div className="flex max-h-[62vh] flex-col gap-2 overflow-y-auto">
        {subscriptions.length === 0 ? (
          <div className="flex flex-col gap-1 py-8 text-center">
            <span className="text-sm text-zinc-500 dark:text-zinc-400">暂无订阅</span>
            <span className="text-xs text-zinc-400 dark:text-zinc-500">请到「管理订阅」添加订阅源</span>
          </div>
        ) : (
          subscriptions.map((sub) => {
            const isCurrent = sub.id === activeSubscriptionId;
            return (
              <button
                key={sub.id}
                type="button"
                disabled={selecting}
                onClick={() => void selectSubscription(sub)}
                className={`flex w-full items-center justify-between gap-2 rounded-xl border px-4 py-3 text-left transition-colors disabled:opacity-60 ${
                  isCurrent ? "border-primary/50 bg-primary/5" : "border-zinc-200 dark:border-zinc-700"
                }`}
              >
                <span className="flex min-w-0 flex-col gap-0.5">
                  <span className="truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">{sub.name}</span>
                  <span className="text-xs text-zinc-500 dark:text-zinc-400">{sub.node_count} 个节点</span>
                </span>
                {isCurrent && <span className="shrink-0 text-xs font-medium text-primary">使用中</span>}
              </button>
            );
          })
        )}
      </div>
    </BottomSheet>
  );
}
