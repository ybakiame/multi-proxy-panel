import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { Button, Modal } from "@heroui/react";
import {
  CONFIG_KEY,
  SUBSCRIPTIONS_KEY,
  toErrorMessage,
  toastError,
  toastSuccess,
  toastWarning,
  useSaveConfig,
} from "@pp/client-core";
import type { ClientConfig, SubscriptionView } from "@pp/client-core";

interface SubscriptionSheetProps {
  isOpen: boolean;
  onClose: () => void;
  /** 全部订阅（含停用）；列表只展示已启用的订阅。 */
  subscriptions: SubscriptionView[];
  activeSubscriptionId: string | null;
}

/**
 * 订阅切换底部 Sheet（ADR-0003 M5）。
 *
 * 点击订阅行 → `useSaveConfig` 叠加 `active_subscription_id` 补丁（对齐 desktop Dashboard
 * 的 `persistConfig` 模式：成功 toast + invalidate CONFIG_KEY / SUBSCRIPTIONS_KEY）。
 * 底部「管理订阅」跳转订阅管理页（`/subscriptions`，M5.6 落地）。
 */
export function SubscriptionSheet({ isOpen, onClose, subscriptions, activeSubscriptionId }: SubscriptionSheetProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const saveConfigMutation = useSaveConfig();
  const [selecting, setSelecting] = useState(false);
  const enabledSubs = subscriptions.filter((sub) => sub.enabled);

  const selectSubscription = async (id: string) => {
    // 从 Query 缓存取最新配置叠加补丁（避免闭包旧值；保存期间行按钮已禁用）。
    const current = queryClient.getQueryData<ClientConfig>(CONFIG_KEY);
    if (!current || current.active_subscription_id === id) {
      onClose();
      return;
    }
    setSelecting(true);
    try {
      const { warning } = await saveConfigMutation.mutateAsync({ ...current, active_subscription_id: id });
      if (warning) {
        toastWarning(warning);
      } else {
        toastSuccess("生效订阅已切换");
      }
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
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      isDismissable
    >
      <Modal.Container placement="bottom">
        <Modal.Dialog>
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>选择生效订阅</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="max-h-[55vh] overflow-y-auto">
            <div className="flex flex-col gap-2">
              {enabledSubs.length === 0 ? (
                <div className="flex flex-col gap-1 py-8 text-center">
                  <span className="text-sm text-muted">暂无已启用的订阅</span>
                  <span className="text-xs text-muted/80">请到「管理订阅」添加订阅源并保持启用</span>
                </div>
              ) : (
                enabledSubs.map((sub) => {
                  const isCurrent = sub.id === activeSubscriptionId;
                  return (
                    <button
                      key={sub.id}
                      type="button"
                      disabled={selecting}
                      onClick={() => void selectSubscription(sub.id)}
                      className={`flex w-full items-center justify-between gap-2 rounded-xl border px-4 py-3 text-left transition-colors disabled:opacity-60 ${
                        isCurrent ? "border-primary/50 bg-primary/5" : "border-border/70"
                      }`}
                    >
                      <span className="flex min-w-0 flex-col gap-0.5">
                        <span className="truncate text-sm font-medium text-foreground">{sub.name}</span>
                        <span className="text-xs text-muted">{sub.node_count} 个节点</span>
                      </span>
                      {isCurrent && <span className="shrink-0 text-xs font-medium text-primary">使用中</span>}
                    </button>
                  );
                })
              )}
            </div>
          </Modal.Body>
          <Modal.Footer>
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
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
