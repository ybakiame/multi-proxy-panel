import { Card, Spinner } from "@heroui/react";
import { DocumentTextIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { EntryLinkCard } from "../../components/EntryLinkCard";
import { PageShell } from "../../components/PageShell";
import { AboutCard } from "./AboutCard";
import { ClashApiCard } from "./ClashApiCard";
import { GithubAccessCard } from "./GithubAccessCard";
import { NetworkCard } from "./NetworkCard";
import { VpnNotificationCard } from "./VpnNotificationCard";
import { useSettingsConfig } from "./useSettingsConfig";

/**
 * 设置页（ADR-0003 M5.3，原占位页重写为完整页）。分组卡片自上而下：
 *
 * 1. VPN 通知（移动端专属）：通知栏显示流量 / 节点选择两个 Switch，保存成功后
 *    追加 notifyPrefsChanged 热更新通知栏；
 * 2. Clash API：启用 Switch + 端口 + 可选密钥；
 * 3. 网络：本地混合端口；
 * 4. GitHub 访问：代理前缀；
 * 5. 日志入口卡（点击进 `/logs`）；
 * 6. 关于：应用名 + 一行说明（无版本数据源，不显示版本）。
 *
 * 全部字段经 @pp/client-core 的 useClientConfig / useSaveConfig（patch 叠加 +
 * toast + 失败回滚），数字输入校验端口 1-65535；配置加载完成前字段禁用。
 */
export default function Settings() {
  const navigate = useNavigate();
  const settings = useSettingsConfig();

  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端全局设置 · 修改即时保存</p>
      </div>

      {!settings.ready ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
            <Spinner aria-hidden="true" />
            <span className="text-sm text-muted">正在加载配置…</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-4">
          <VpnNotificationCard settings={settings} />
          <ClashApiCard settings={settings} />
          <NetworkCard settings={settings} />
          <GithubAccessCard settings={settings} />
          <EntryLinkCard
            icon={<DocumentTextIcon className="size-6" aria-hidden="true" />}
            title="日志"
            description="查看运行与内核日志，支持导出与清空"
            onPress={() => navigate("/logs")}
          />
          <AboutCard />
        </div>
      )}
    </PageShell>
  );
}
