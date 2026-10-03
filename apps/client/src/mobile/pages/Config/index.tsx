import { ArrowsRightLeftIcon, BeakerIcon, GlobeAltIcon, MapIcon, WifiIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { EntryLinkList } from "../../components/EntryLinkCard";
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
      </div>

      <EntryLinkList
        entries={[
          {
            icon: <GlobeAltIcon className="size-5" aria-hidden="true" />,
            title: "DNS 管理",
            description: "DNS 中的服务器与分流规则",
            onPress: () => navigate("/config/dns"),
          },
          {
            icon: <ArrowsRightLeftIcon className="size-5" aria-hidden="true" />,
            title: "出站管理",
            description: "Outbounds 中的代理节点与分组",
            onPress: () => navigate("/config/outbounds"),
          },
          {
            icon: <MapIcon className="size-5" aria-hidden="true" />,
            title: "路由管理",
            description: "Route 中的规则集和路由规则",
            onPress: () => navigate("/config/route"),
          },
          {
            icon: <WifiIcon className="size-5" aria-hidden="true" />,
            title: "入站管理",
            description: "Inbounds 中的端口与 TUN 参数",
            onPress: () => navigate("/config/inbounds"),
          },
          {
            icon: <BeakerIcon className="size-5" aria-hidden="true" />,
            title: "实验性配置",
            description: "Experimental 中的 Clash API 与缓存",
            onPress: () => navigate("/config/experimental"),
          },
        ]}
      />
    </PageShell>
  );
}
