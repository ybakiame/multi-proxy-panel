import { BellAlertIcon, CodeBracketIcon, GlobeAltIcon, InformationCircleIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { EntryLinkCard } from "../../components/EntryLinkCard";
import { PageShell } from "../../components/PageShell";
import { AppearanceCard } from "./AppearanceCard";

/**
 * 设置主页（ADR-0003 M5.3，列表化重构）。Tab 页（非二级页），保留 PageShell 标题。
 *
 * 「外观」为直挂设置卡（主题切换高频操作，不入二级页）；其余功能分组为
 * `EntryLinkCard` 列表入口，卡片内容抽离到对应二级子页：VPN 通知 / GitHub 访问 /
 * 开发者工具 / 关于。表单状态由各子页独立实例化 `useSettingsConfig` 消费
 * （保存链路经共享 CONFIG_KEY 缓存）。
 *
 * 入站管理与 Clash API 已迁移为配置页必选切片入口（`/config/inbounds`、`/config/experimental`）。
 */
export default function Settings() {
  const navigate = useNavigate();

  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端全局设置 · 修改即时保存</p>
      </div>

      <AppearanceCard />

      <div className="flex flex-col gap-4">
        <EntryLinkCard
          icon={<BellAlertIcon className="size-6" aria-hidden="true" />}
          title="VPN 通知"
          description="通知栏显示流量与节点选择"
          onPress={() => navigate("/settings/vpn-notify")}
        />
        <EntryLinkCard
          icon={<GlobeAltIcon className="size-6" aria-hidden="true" />}
          title="GitHub 访问"
          description="代理前缀加速"
          onPress={() => navigate("/settings/github")}
        />
        <EntryLinkCard
          icon={<CodeBracketIcon className="size-6" aria-hidden="true" />}
          title="开发者工具"
          description="日志与当前配置"
          onPress={() => navigate("/settings/dev-tools")}
        />
        <EntryLinkCard
          icon={<InformationCircleIcon className="size-6" aria-hidden="true" />}
          title="关于"
          description="应用与核心信息"
          onPress={() => navigate("/settings/about")}
        />
      </div>
    </PageShell>
  );
}
