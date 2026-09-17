import { ArrowsRightLeftIcon, BeakerIcon, GlobeAltIcon, MapIcon, WifiIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { EntryLinkCard } from "../../components/EntryLinkCard";
import { PageShell } from "../../components/PageShell";

/**
 * 配置管理入口页（ADR-0005 §3.3，路由 `/config`，Tab 2）。
 *
 * 入口卡列表（名称对齐 sing-box 顶级字段语义）：DNS 管理（`dns`，`/config/dns`）、
 * 出站管理（`outbounds`，`/config/outbounds`）、路由管理（`route`，`/config/route`）、
 * 入站管理（`inbounds`，`/config/inbounds`）、Experimental（`experimental`，
 * `/config/experimental`，含 Clash API 设置）。
 *
 * 网络（TUN）与 Clash API 为必选切片（ADR-0005 后续决策）：始终启用、无切片级关闭开关，
 * 入口仅提供参数调整（混合端口 / API 端口与密钥）。其余切片无总开关，有内容即生效。
 *
 * 规则管理（`/config/route/rules`）与规则集管理（`/config/route/rulesets`）已迁入路由菜单页。
 */
export default function Config() {
  const navigate = useNavigate();

  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">配置管理</h1>
        <p className="text-sm text-muted">管理 DNS、出站、路由、入站与实验性参数</p>
        <p className="mt-1 text-xs text-muted">内置 CN 分流基线始终生效，自定义配置逐项启用</p>
      </div>

      <div className="flex flex-col gap-4">
        <EntryLinkCard
          icon={<GlobeAltIcon className="size-6" aria-hidden="true" />}
          title="DNS 管理"
          description="dns 顶级字段 · DNS 服务器与分流规则"
          onPress={() => navigate("/config/dns")}
        />
        <EntryLinkCard
          icon={<ArrowsRightLeftIcon className="size-6" aria-hidden="true" />}
          title="出站管理"
          description="outbounds 顶级字段 · 自定义代理节点与分组"
          onPress={() => navigate("/config/outbounds")}
        />
        <EntryLinkCard
          icon={<MapIcon className="size-6" aria-hidden="true" />}
          title="路由管理"
          description="route 顶级字段 · 默认出站、域名解析器与规则"
          onPress={() => navigate("/config/route")}
        />
        <EntryLinkCard
          icon={<WifiIcon className="size-6" aria-hidden="true" />}
          title="入站管理"
          description="inbounds 顶级字段 · 混合入站端口与 TUN 参数"
          onPress={() => navigate("/config/inbounds")}
        />
        <EntryLinkCard
          icon={<BeakerIcon className="size-6" aria-hidden="true" />}
          title="实验性配置"
          description="experimental 顶级字段 · Clash API（端口 / 密钥）与缓存"
          onPress={() => navigate("/config/experimental")}
        />
      </div>
    </PageShell>
  );
}
