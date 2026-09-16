import { useState } from "react";
import { Card } from "@heroui/react";
import { CodeBracketIcon, DocumentTextIcon, SignalIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { useClientConfig } from "@pp/client-core";
import { ConfigPreviewModal } from "../../components/ConfigPreviewModal";
import { EntryLinkCard } from "../../components/EntryLinkCard";

/**
 * 开发者工具分组（设置页「关于」分组上方）。
 *
 * - 「连通性诊断」入口：点击进 `/settings/dev-tools/diagnose`（分步诊断目标域名的
 *   DNS / 直连 / 核心链路 / 代理出站）；
 * - 「日志」入口：点击进 `/logs`（自设置页原独立入口迁入）；
 * - 「当前配置」入口：打开配置预览 Modal（`ConfigPreviewModal` 自首页迁移至此，
 *   按当前生效订阅合成只读配置，`previewCoreConfig` 调用不变）。
 */
export function DeveloperToolsCard() {
  const navigate = useNavigate();
  const { data: config } = useClientConfig();
  const [previewOpen, setPreviewOpen] = useState(false);

  return (
    <>
      <Card>
        <Card.Header>
          <Card.Title>开发者工具</Card.Title>
          <Card.Description>开发与排障辅助</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-3">
          <EntryLinkCard
            icon={<DocumentTextIcon className="size-6" aria-hidden="true" />}
            title="日志"
            description="查看运行与内核日志，支持导出与清空"
            onPress={() => navigate("/logs")}
          />
          <EntryLinkCard
            icon={<SignalIcon className="size-6" aria-hidden="true" />}
            title="连通性诊断"
            description="对目标域名分步检查 DNS / 直连 / 核心链路 / 代理出站"
            onPress={() => navigate("/settings/dev-tools/diagnose")}
          />
          <EntryLinkCard
            icon={<CodeBracketIcon className="size-6" aria-hidden="true" />}
            title="当前配置"
            description="预览当前生效订阅合成的核心配置"
            onPress={() => setPreviewOpen(true)}
          />
        </Card.Content>
      </Card>
      <ConfigPreviewModal
        isOpen={previewOpen}
        onClose={() => setPreviewOpen(false)}
        subscriptionId={config?.active_subscription_id}
      />
    </>
  );
}
