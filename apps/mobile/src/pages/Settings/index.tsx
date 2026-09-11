import {
  BellAlertIcon,
  BoltIcon,
  CodeBracketIcon,
  GlobeAltIcon,
  InformationCircleIcon,
  WifiIcon,
} from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { EntryLinkCard } from "../../components/EntryLinkCard";
import { PageShell } from "../../components/PageShell";

/**
 * 设置主页（ADR-0003 M5.3，列表化重构）。Tab 页（非二级页），保留 PageShell 标题。
 *
 * 各功能分组由原分组卡片改为 `EntryLinkCard` 列表入口，卡片内容抽离到对应二级子页：
 * VPN 通知 / Clash API / 网络 / GitHub 访问 / 开发者工具 / 关于。表单状态由各子页
 * 独立实例化 `useSettingsConfig` 消费（保存链路经共享 CONFIG_KEY 缓存）。
 */
export default function Settings() {
  const navigate = useNavigate();

  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端全局设置 · 修改即时保存</p>
      </div>

      <div className="flex flex-col gap-4">
        <EntryLinkCard
          icon={<BellAlertIcon className="size-6" aria-hidden="true" />}
          title="VPN 通知"
          description="通知栏显示流量与节点选择"
          onPress={() => navigate("/settings/vpn-notify")}
        />
        <EntryLinkCard
          icon={<BoltIcon className="size-6" aria-hidden="true" />}
          title="Clash API"
          description="面板 API 开关与端口（流量统计/模式切换依赖）"
          onPress={() => navigate("/settings/clash-api")}
        />
        <EntryLinkCard
          icon={<WifiIcon className="size-6" aria-hidden="true" />}
          title="网络"
          description="本地混合端口配置"
          onPress={() => navigate("/settings/network")}
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
