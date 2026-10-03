import type { GroupView, NodeView } from "@pp/client-core";

/** 分组类型中文标签（对齐 desktop Proxies）。 */
export function groupTypeLabel(type: string): string {
  const map: Record<string, string> = {
    Selector: "选择器",
    URLTest: "自动测速",
    Fallback: "故障转移",
    LoadBalance: "负载均衡",
  };
  return map[type] ?? type;
}

/** 节点类型中文标签（常见类型映射，对齐 desktop Proxies NodeItem）。 */
export function nodeTypeLabel(type: string | undefined): string {
  if (!type) return "未知";
  const map: Record<string, string> = {
    Shadowsocks: "SS",
    Vmess: "VMess",
    Trojan: "Trojan",
    Vless: "VLESS",
    Hysteria: "Hysteria",
    Hysteria2: "Hy2",
    Tuic: "Tuic",
    WireGuard: "WG",
    Direct: "直连",
    Reject: "拒绝",
    Http: "HTTP",
    Socks5: "SOCKS5",
    Snell: "Snell",
  };
  return map[type] ?? type;
}

/** 延迟分级色：绿 < 300ms，黄 < 800ms，灰 = 超时/未测（对齐 desktop NodeItem）。 */
export function delayColor(delay: number | null | undefined): "success" | "warning" | "default" {
  if (delay == null) return "default";
  if (delay < 300) return "success";
  if (delay < 800) return "warning";
  return "default";
}

/** 延迟展示文本。 */
export function delayText(delay: number | null | undefined): string {
  if (delay == null) return "超时";
  return `${delay}ms`;
}

/** 构建节点名 → 节点详情的映射表。 */
export function buildNodeMap(nodes: NodeView[]): Map<string, NodeView> {
  const map = new Map<string, NodeView>();
  for (const node of nodes) {
    map.set(node.name, node);
  }
  return map;
}

/** 主出口分组：第一个 `Selector` 分组；无 Selector 时兜底第一个分组；无分组返回 `null`。 */
export function mainProxyGroup(groups: GroupView[]): GroupView | null {
  return groups.find((group) => group.group_type === "Selector") ?? groups[0] ?? null;
}
