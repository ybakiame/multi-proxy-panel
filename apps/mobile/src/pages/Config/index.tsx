import { ArrowsRightLeftIcon, BeakerIcon, GlobeAltIcon, MapIcon, WifiIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { EntryLinkCard } from "../../components/EntryLinkCard";
import { PageShell } from "../../components/PageShell";

/**
 * 配置管理入口页（ADR-0005 §3.3，路由 `/config`，Tab 2）。
 *
 * 入口卡列表：DNS（`/config/dns`）、自定义出站（`/config/outbounds`）、
 * 路由（`/config/route`）、网络（TUN，`/config/network`）、
 * Experimental（`/config/experimental`，含 Clash API 设置）。
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
        <p className="text-sm text-muted">DNS · 出站 · 规则 · 规则集</p>
      </div>

      <p className="text-xs text-muted">内置 CN 分流基线始终生效，自定义配置逐项启用。</p>

      <div className="flex flex-col gap-4">
        <EntryLinkCard
          icon={<GlobeAltIcon className="size-6" aria-hidden="true" />}
          title="DNS"
          description="DNS 服务器与分流规则配置"
          onPress={() => navigate("/config/dns")}
        />
        <EntryLinkCard
          icon={<ArrowsRightLeftIcon className="size-6" aria-hidden="true" />}
          title="自定义出站"
          description="可视化添加与管理自定义出站"
          onPress={() => navigate("/config/outbounds")}
        />
        <EntryLinkCard
          icon={<MapIcon className="size-6" aria-hidden="true" />}
          title="路由"
          description="默认出站、域名解析器与规则管理"
          onPress={() => navigate("/config/route")}
        />
        <EntryLinkCard
          icon={<WifiIcon className="size-6" aria-hidden="true" />}
          title="网络（TUN）"
          description="TUN 始终启用 · 调整本地混合端口"
          onPress={() => navigate("/config/network")}
        />
        <EntryLinkCard
          icon={<BeakerIcon className="size-6" aria-hidden="true" />}
          title="Experimental"
          description="Clash API（端口 / 密钥）与缓存等实验性配置"
          onPress={() => navigate("/config/experimental")}
        />
      </div>
    </PageShell>
  );
}
