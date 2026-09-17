import { Alert, Card } from "@heroui/react";
import { MobileSelectSheet } from "../../components/MobileSelectSheet";
import { BackHeader } from "../../components/BackHeader";
import { SettingsInput, SwitchRow } from "../Settings/fields";
import { useSettingsConfig } from "../Settings/useSettingsConfig";

/** TUN 协议栈选项（对齐 sing-box tun inbound `stack` 字段；`go` = 不写 stack 字段，
 * 见 sing-box 迁移指南 Migrate TUN Stack）。说明文字提升到选择器外上方（小字），
 * 选项标签只留栈名。 */
const TUN_STACK_OPTIONS = [
  { value: "go", label: "go" },
  { value: "mixed", label: "mixed" },
  { value: "system", label: "system" },
];

/** 各栈说明（选择器上方的小字段落）。 */
const TUN_STACK_HINT =
  "go：sing-tun 自研栈（推荐，sing-box 1.15 起的默认栈，纯用户态无设备兼容问题）；mixed：TCP 系统栈 + UDP gVisor（部分设备 TCP 不可用）；system：系统栈，性能最佳但部分设备不兼容";

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

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="入站管理" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        <Alert status="accent">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>始终启用</Alert.Title>
            <Alert.Description>
              两条入站对应核心的 inbounds 顶级字段，始终启用；此处调整其监听与协议栈参数。
            </Alert.Description>
          </Alert.Content>
        </Alert>

        {/* 混合入站（mixed-in）：listen 固定 127.0.0.1，仅 listen_port 可调 */}
        <Card>
          <Card.Header>
            <Card.Title>混合入站（mixed-in）</Card.Title>
            <Card.Description>HTTP / SOCKS 混合代理入站，监听 127.0.0.1</Card.Description>
          </Card.Header>
          <Card.Content className="flex flex-col gap-4">
            <SettingsInput
              id="settings-mixed-port"
              label="监听端口（listen_port）"
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
            <Card.Description>虚拟网卡全流量接管（Android VpnService）</Card.Description>
          </Card.Header>
          <Card.Content className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <div className="flex flex-col gap-0.5">
                <span className="text-sm font-medium text-foreground">协议栈（stack）</span>
                <span className="text-xs text-muted">{TUN_STACK_HINT}</span>
              </div>
              <MobileSelectSheet
                label="协议栈"
                value={settings.tunStack}
                onChange={(value) => void settings.onTunStackChange(value)}
                options={TUN_STACK_OPTIONS}
              />
            </div>
            <SwitchRow
              label="自动路由（auto_route）"
              description="自动配置系统路由表接管流量；Android VpnService 下保持开启"
              selected={settings.tunAutoRoute}
              disabled={!settings.ready}
              onChange={(next) => void settings.onToggleTunAutoRoute(next)}
            />
            <SwitchRow
              label="IPv6"
              description="关闭时 DNS 不返回 AAAA 记录（推荐，节点不支持 IPv6 时最稳定）；开启后双栈，IPv6 流量经隧道按规则分流"
              selected={settings.ipv6Enabled}
              disabled={!settings.ready}
              onChange={(next) => void settings.onToggleIpv6(next)}
            />
          </Card.Content>
        </Card>
      </div>
    </div>
  );
}
