import { PageShell } from "../../components/PageShell";
import { useNavigate } from "react-router-dom";
import {
  ArrowsRightLeftIcon,
  BeakerIcon,
  ChevronRightIcon,
  GlobeAltIcon,
  MapIcon,
  WifiIcon,
} from "@heroicons/react/24/outline";
import type { ComponentType, SVGProps } from "react";

interface ConfigEntry {
  to: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
  title: string;
  description: string;
}

/**
 * 双端共用配置管理入口页（分段配置切片编辑）。
 *
 * 入口卡列表（名称对齐 sing-box 顶级字段语义）：DNS 管理（`dns`）、出站管理
 * （`outbounds`）、路由管理（`route`）、入站管理（`inbounds`）、Experimental
 * （`experimental`）。
 *
 * 入站（混合端口 / TUN / IPv6）与 Clash API 不是配置切片：作为高优先级设置
 * （`client.json`）在配置合成 ④ 层覆盖模板/覆写同名字段；入站管理由本页
 * `/config/inbounds` 承载（2026-10 自「设置」页剥离，对齐移动端），Clash API
 * 由 `/config/experimental` 的 Clash API 卡承载（同步自「设置」页剥离）；规则
 * 管理与规则集管理迁入「路由管理」（`/config/route` 入口区，对齐移动端）。
 */
const CONFIG_ENTRIES: ConfigEntry[] = [
  {
    to: "/config/dns",
    icon: GlobeAltIcon,
    title: "DNS 管理",
    description: "DNS 服务器、分流规则与 FakeIP",
  },
  {
    to: "/config/outbounds",
    icon: ArrowsRightLeftIcon,
    title: "出站管理",
    description: "自定义代理节点与分组（内置 proxy / auto 可编辑）",
  },
  {
    to: "/config/route",
    icon: MapIcon,
    title: "路由管理",
    description: "默认出站与默认域名解析器",
  },
  {
    to: "/config/inbounds",
    icon: WifiIcon,
    title: "入站管理",
    description: "Inbounds 中的混合端口与 TUN 参数",
  },
  {
    to: "/config/experimental",
    icon: BeakerIcon,
    title: "Experimental",
    description: "实验性配置（Clash API 与缓存）",
  },
];

export default function Config() {
  const navigate = useNavigate();

  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">配置管理</h1>
        <p className="text-sm text-muted">核心配置的分段编辑；修改需重启代理后生效</p>
      </div>

      <div className="flex flex-col gap-2">
        {CONFIG_ENTRIES.map((entry) => {
          const Icon = entry.icon;
          return (
            <button
              key={entry.to}
              type="button"
              onClick={() => navigate(entry.to)}
              className="flex items-center gap-3 rounded-xl border border-border/60 bg-surface p-4 text-left transition-colors hover:bg-surface-secondary/40"
            >
              <Icon className="size-5 shrink-0 text-muted" aria-hidden="true" />
              <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="text-sm font-medium text-foreground">{entry.title}</span>
                <span className="text-xs text-muted">{entry.description}</span>
              </span>
              <ChevronRightIcon className="size-4 shrink-0 text-muted" aria-hidden="true" />
            </button>
          );
        })}
      </div>
    </PageShell>
  );
}
