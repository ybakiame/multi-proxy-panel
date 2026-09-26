import { InlineAlert } from "../../components/InlineAlert";
import { Card } from "../../components/ui";
import { MobileSelectSheet } from "../../components/MobileSelectSheet";
import { SubPageShell } from "../../components/SubPageShell";
import { SettingsInput, SwitchRow } from "../Settings/fields";
import { useSettingsConfig } from "../Settings/useSettingsConfig";
import { useMemo } from "react";

/** TUN 协议栈选项（对齐 sing-box tun inbound `stack` 字段) */
const TUN_STACK_OPTIONS = [
  { value: "go", label: "go", desc: "sing-tun 自研栈（sing-box 1.15 起的默认栈）" },
  { value: "mixed", label: "mixed", desc: "TCP 系统栈 + UDP gVisor（部分设备 TCP 不可用）" },
  { value: "system", label: "system", desc: "系统栈，性能最佳但部分设备不兼容" },
];

/**
 * 入站管理子页（路由 `/config/inbounds`；前身为「网络」页 `/config/network`，2026-09 更名
 * 对齐 sing-box 顶级 `inbounds` 字段语义）。
 *
 * 结构按核心最终配置的两条入站组织：
 * - 混合入站（`mixed-in`）：`listen` 固定 127.0.0.1，仅可调 `listen_port`（mixed_port）；
 * - TUN 入站（`tun-in`）：`stack` / `auto_route`（Android 恒注入 `strict_route`、双栈地址
 *   与 MTU 9000 为内置不可调）；IPv6 参数开关控制双栈承载与 DNS AAAA。
 *
 * 两条入站为代理基础能力（Android 由 panel features 恒注入），始终启用、无关闭开关。
 * 字段均即时保存（useSettingsConfig），核心重启后生效。
 */
export default function InboundsPage() {
  const settings = useSettingsConfig();
  const stackDesc = useMemo(() => {
    return TUN_STACK_OPTIONS.find((option) => option.value === settings.tunStack)?.desc;
  }, [settings.tunStack]);

  return (
    <SubPageShell title="入站管理">
      <InlineAlert kind="info">仅支持部分参数调整</InlineAlert>

      {/* 混合入站（mixed-in）：listen 固定 127.0.0.1，仅 listen_port 可调 */}
      <Card>
        <Card.Header>
          <Card.Title>混合入站</Card.Title>
          <Card.Description>HTTP / SOCKS 混合代理入站，监听 127.0.0.1</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <SettingsInput
            id="settings-mixed-port"
            label="监听端口"
            value={settings.mixedPortDraft}
            onChange={settings.onMixedPortChange}
            placeholder="1080"
            disabled={!settings.ready}
            inputMode="numeric"
            error={settings.mixedPortError}
            hint="其他 App 手动配置代理（HTTP / SOCKS）时使用的本地端口"
          />
        </Card.Content>
      </Card>

      {/* TUN 入站（tun-in）：stack / auto_route 可调，strict_route / 双栈地址 / MTU 内置固定 */}
      <Card>
        <Card.Header>
          <Card.Title>TUN 入站（tun-in）</Card.Title>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <div className="flex flex-col gap-0.5">
              <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">协议栈（stack）</span>
              <span className="text-xs text-zinc-500 dark:text-zinc-400">{stackDesc}</span>
            </div>
            <MobileSelectSheet
              label="协议栈"
              value={settings.tunStack}
              onChange={(value) => void settings.onTunStackChange(value)}
              options={TUN_STACK_OPTIONS}
            />
          </div>
          <SwitchRow
            label="自动路由"
            description="自动配置系统路由表接管流量"
            selected={settings.tunAutoRoute}
            disabled={!settings.ready}
            onChange={(next) => void settings.onToggleTunAutoRoute(next)}
          />
          <SwitchRow
            label="IPv6 支持"
            description="关闭时 DNS 不返回 AAAA 记录（推荐，节点不支持 IPv6 时最稳定）；开启后双栈，IPv6 流量经隧道按规则分流"
            selected={settings.ipv6Enabled}
            disabled={!settings.ready}
            onChange={(next) => void settings.onToggleIpv6(next)}
          />
        </Card.Content>
      </Card>
    </SubPageShell>
  );
}
