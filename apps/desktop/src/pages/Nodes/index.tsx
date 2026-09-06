import { useCallback, useState } from "react";
import { Button, Card } from "@heroui/react";
import { useQueryClient } from "@tanstack/react-query";
import { CONFIG_KEY } from "@pp/client-core";
import ConfigPreviewModal from "../../components/ConfigPreviewModal";
import { MobileBackHeader } from "../../layout/mobile/MobileBackHeader";
import { useSubscriptionData } from "./useSubscriptionData";
import { SubscriptionTable } from "./SubscriptionTable";
import { SubscriptionAlerts } from "./SubscriptionAlerts";
import { AddSubscriptionModal } from "./AddSubscriptionModal";
import { EditSubscriptionModal } from "./EditSubscriptionModal";

export default function Nodes() {
  const {
    subs,
    profiles,
    busy,
    error,
    result,
    refreshingId,
    refreshSubs,
    handleAdd,
    handleRemove,
    handleToggle,
    handleRefresh,
    handleEditSave,
    setError,
    setResult,
  } = useSubscriptionData();

  const [addOpen, setAddOpen] = useState(false);
  const [editSub, setEditSub] = useState<import("@pp/client-core").SubscriptionView | null>(null);
  const [previewSub, setPreviewSub] = useState<import("@pp/client-core").SubscriptionView | null>(null);

  const queryClient = useQueryClient();
  // toggle 订阅后失效配置缓存触发重读（替代原 store.loadConfig）。
  const reloadConfig = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
  }, [queryClient]);

  const anyEnabled = subs.some((sub) => sub.enabled);

  const onToggle = async (sub: import("@pp/client-core").SubscriptionView) => {
    await handleToggle(sub, reloadConfig);
  };

  const onEditSave = async (
    sub: import("@pp/client-core").SubscriptionView,
    name: string,
    url: string,
    profileId: string | null,
    userAgent?: string,
  ) => {
    await handleEditSave(sub, name, url, profileId, userAgent);
    setEditSub(null);
  };

  return (
    <div className="flex flex-col gap-6">
      <MobileBackHeader title="订阅" />
      <div>
        <h1 className="text-xl font-semibold">订阅</h1>
        <p className="text-sm text-muted">
          多订阅源管理：拉取节点、用量与到期展示，启用后可被首页选择，选择的订阅唯一生效
        </p>
      </div>

      <Card>
        <Card.Header>
          <Card.Title>订阅列表</Card.Title>
          <Card.Description>添加后立即拉取一次；节点数 / 用量 / 到期为最近一次拉取结果</Card.Description>
        </Card.Header>
        <Card.Content>
          <SubscriptionTable
            subs={subs}
            profiles={profiles}
            busy={busy}
            refreshingId={refreshingId}
            onToggle={onToggle}
            onRefresh={handleRefresh}
            onRemove={handleRemove}
            onEdit={(sub) => {
              setError(null);
              setEditSub(sub);
            }}
            onPreview={(sub) => setPreviewSub(sub)}
          />
        </Card.Content>
        <Card.Footer>
          <div className="flex w-full items-center justify-between gap-3">
            <Button variant="secondary" isDisabled={busy} onPress={() => void refreshSubs()}>
              刷新列表
            </Button>
            <Button variant="primary" isDisabled={busy} onPress={() => setAddOpen(true)}>
              添加订阅
            </Button>
          </div>
        </Card.Footer>
      </Card>

      <SubscriptionAlerts anyEnabled={anyEnabled} result={result} error={error} />

      <AddSubscriptionModal
        isOpen={addOpen}
        onClose={() => {
          setAddOpen(false);
          setError(null);
        }}
        busy={busy}
        profiles={profiles}
        onAdd={(name, url, ua, profileId) => {
          setError(null);
          setResult(null);
          void handleAdd(name, url, ua, profileId);
          setAddOpen(false);
        }}
      />

      <EditSubscriptionModal
        sub={editSub}
        busy={busy}
        profiles={profiles}
        onClose={() => setEditSub(null)}
        onSave={onEditSave}
      />

      <ConfigPreviewModal
        isOpen={previewSub !== null}
        onClose={() => setPreviewSub(null)}
        title={previewSub ? `配置预览 — ${previewSub.name}` : ""}
        subscriptionId={previewSub?.id}
      />
    </div>
  );
}
