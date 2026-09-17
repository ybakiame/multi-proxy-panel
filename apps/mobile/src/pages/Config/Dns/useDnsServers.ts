import { useState } from "react";
import { dnsServerProbe, toErrorMessage, uniquePresetTag } from "@pp/client-core";
import type { DnsServer, DnsServerPreset, DnsSlice } from "@pp/client-core";
import type { DnsProbeState } from "./DnsServerListSection";

/** DNS 管理页服务器库 hook（启用/弃用切换、预置项物化、批量延迟探测）。 */
export interface UseDnsServersReturn {
  /** tag → 探测结果（无键 = 未探测）；仅内存态，不落盘。 */
  probes: Readonly<Record<string, DnsProbeState>>;
  /** 批量探测进行中。 */
  probing: boolean;
  /** 启用 / 弃用切换（弃用保留在列表但不进运行配置）。 */
  handleToggleServer: (target: DnsServer, enabled: boolean) => void;
  /** 预置项物化为切片服务器（tag 冲突追加序号，name 取 ISP 名，默认启用）。 */
  handlePickPreset: (preset: DnsServerPreset) => void;
  /** 批量探测启用中的服务器（并发，行内显示真实查询延迟）。 */
  handleProbeServers: (draft: DnsSlice | null) => Promise<void>;
}

/**
 * DNS 服务器库逻辑（从页面组件拆出以控制文件规模，见
 * `.agents/rules/code-organization.md`）。
 *
 * `setDraft` 为页面 DNS 切片草稿的 setter；探测结果保存在 hook 内部 state，
 * 按服务器 tag 键控，探测发起时先把目标置 pending，单个完成即更新对应行。
 */
export function useDnsServers(setDraft: React.Dispatch<React.SetStateAction<DnsSlice | null>>): UseDnsServersReturn {
  const [probes, setProbes] = useState<Record<string, DnsProbeState>>({});
  const [probing, setProbing] = useState(false);

  const handleToggleServer = (target: DnsServer, enabled: boolean) => {
    setDraft((current) =>
      current
        ? { ...current, servers: current.servers.map((item) => (item === target ? { ...item, enabled } : item)) }
        : current,
    );
  };

  const handlePickPreset = (preset: DnsServerPreset) => {
    setDraft((current) => {
      if (!current) return current;
      const tag = uniquePresetTag(
        preset.tag,
        current.servers.map((server) => server.tag),
      );
      const server: DnsServer = {
        tag,
        name: preset.isp,
        enabled: true,
        server: preset.server,
        server_type: preset.serverType,
        server_port: preset.serverPort,
        inet4_range: "",
        inet6_range: "",
        detour: preset.detour ?? "",
        strategy: null,
        domain_resolver: "",
      };
      return { ...current, servers: [...current.servers, server] };
    });
  };

  const handleProbeServers = async (draft: DnsSlice | null) => {
    if (!draft || probing) return;
    const targets = draft.servers.filter((server) => server.enabled);
    setProbing(true);
    setProbes((current) => {
      const next = { ...current };
      for (const server of targets) {
        next[server.tag] = { pending: true, latency: null, error: null, unsupported: false };
      }
      return next;
    });
    await Promise.all(
      targets.map(async (server) => {
        let state: DnsProbeState;
        try {
          const latency = await dnsServerProbe({
            server_type: server.server_type,
            server: server.server,
            server_port: server.server_port,
          });
          state = { pending: false, latency, error: null, unsupported: false };
        } catch (err) {
          const message = toErrorMessage(err);
          state = {
            pending: false,
            latency: null,
            error: message,
            unsupported: message.includes("暂不支持"),
          };
        }
        setProbes((current) => ({ ...current, [server.tag]: state }));
      }),
    );
    setProbing(false);
  };

  return { probes, probing, handleToggleServer, handlePickPreset, handleProbeServers };
}
