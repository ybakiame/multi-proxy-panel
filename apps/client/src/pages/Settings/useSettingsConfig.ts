import { useState } from "react";
import {
  notifyPrefsChanged,
  toErrorMessage,
  toastWarning,
  useClientConfig,
  useSettingsPersist,
  isValidPort,
  portError,
  PORT_RANGE_ERROR,
} from "@pp/client-core";
import type { ClientConfig } from "@pp/client-core";

export { PORT_RANGE_ERROR, isValidPort };

/** Clash API 密钥必填的提示（面板跳转携带密钥，不允许为空）。 */
export const CLASH_API_SECRET_REQUIRED_ERROR = "密钥不能为空，可点右侧按钮随机生成";

/** 随机 Clash API 密钥字母表（URL 安全，无需转义即可拼入面板跳转链接）。 */
const SECRET_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/**
 * 生成随机 Clash API 密钥（24 位 base62，`crypto.getRandomValues` 安全随机源）。
 * 字母表 URL 安全，可直接拼入面板页跳转链接的 query/hash 参数。
 */
export function randomClashApiSecret(): string {
  const bytes = new Uint8Array(24);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => SECRET_ALPHABET[byte % SECRET_ALPHABET.length]).join("");
}

export interface UseSettingsConfigReturn {
  /** 配置是否已加载（控制各字段可用性，避免在默认值上误保存）。 */
  ready: boolean;
  /** 是否有保存进行中（即时保存按钮态 / 输入禁用可据此展示）。 */
  saving: boolean;
  vpnNotifySelection: boolean;
  onToggleVpnSelection: (next: boolean) => Promise<void>;
  ipv6Enabled: boolean;
  onToggleIpv6: (next: boolean) => Promise<void>;
  /** TUN 入站协议栈草稿（`go` / `mixed` / `system`）。 */
  tunStack: string;
  onTunStackChange: (value: string) => Promise<void>;
  /** TUN 入站 auto_route 开关。 */
  tunAutoRoute: boolean;
  onToggleTunAutoRoute: (next: boolean) => Promise<void>;
  clashApiPortDraft: string;
  clashApiPortError: string | null;
  onClashApiPortChange: (raw: string) => void;
  clashApiSecretDraft: string;
  /** 密钥必填校验错误（`null` = 合法）；空密钥不落库。 */
  clashApiSecretError: string | null;
  onClashApiSecretChange: (value: string) => void;
  /** 随机生成密钥并立即落库（显式动作，不经防抖）。 */
  onGenerateClashApiSecret: () => void;
  mixedPortDraft: string;
  mixedPortError: string | null;
  onMixedPortChange: (raw: string) => void;
  githubProxyPrefixDraft: string;
  onGithubProxyPrefixChange: (value: string) => void;
}

/**
 * 共用设置字段视图模型（ADR-0011 试点）：保存引擎复用 client-core 共享的
 * [`useSettingsPersist`]（persist 串行链 + 字段级防抖 + 卸载 flush——该引擎
 * 即由此 hook 的语义抽取单源化），本 hook 同时提供平台专属字段逻辑：VPN 通知
 * 偏好与通知栏热更新、密钥必填校验与随机生成。
 *
 * 表单草稿经 config 渲染期同步（adjust-state-during-render，prevConfig 以
 * undefined 为哨兵保证挂载首渲染即同步）。
 */
export function useSettingsConfig(): UseSettingsConfigReturn {
  const { data: config } = useClientConfig();
  const engine = useSettingsPersist();

  // ---- 表单草稿（config 变化时渲染期同步） ----
  const [vpnNotifySelection, setVpnNotifySelection] = useState(false);
  const [ipv6Enabled, setIpv6Enabled] = useState(false);
  const [tunStack, setTunStack] = useState("mixed");
  const [tunAutoRoute, setTunAutoRoute] = useState(true);
  const [mixedPortDraft, setMixedPortDraft] = useState("");
  const [clashApiPortDraft, setClashApiPortDraft] = useState("");
  const [clashApiSecretDraft, setClashApiSecretDraft] = useState("");
  const [githubProxyPrefixDraft, setGithubProxyPrefixDraft] = useState("");

  // 初始为 undefined（而非 config）：组件重新挂载时若 config 已缓存（有值），
  // 首渲染 prevConfig !== config 成立 → 渲染期回流执行，草稿回显初始值而非残留 false/空串。
  const [prevConfig, setPrevConfig] = useState<ClientConfig | undefined>(undefined);
  if (prevConfig !== config) {
    setPrevConfig(config);
    if (config) {
      setVpnNotifySelection(config.vpn_notify_show_selection);
      setIpv6Enabled(config.ipv6_enabled);
      setTunStack(config.tun_stack || "mixed");
      setTunAutoRoute(config.tun_auto_route);
      setMixedPortDraft(String(config.mixed_port));
      setClashApiPortDraft(String(config.clash_api_port));
      setClashApiSecretDraft(config.clash_api_secret ?? "");
      setGithubProxyPrefixDraft(config.github_proxy_prefix ?? "");
    }
  }

  /** 保存 VPN 通知偏好成功后的通知栏热更新（失败仅 toast 警告，不阻塞主流程）。 */
  const notifyVpnPrefs = async (showSelection: boolean) => {
    try {
      await notifyPrefsChanged(showSelection);
    } catch (err) {
      toastWarning(`通知栏偏好热更新失败：${toErrorMessage(err)}（核心未运行时将在下次启动生效）`);
    }
  };

  const onToggleVpnSelection = async (next: boolean) => {
    setVpnNotifySelection(next);
    if (!(await engine.persist({ vpn_notify_show_selection: next }))) {
      return;
    }
    await notifyVpnPrefs(next);
  };

  const onToggleIpv6 = async (next: boolean) => {
    setIpv6Enabled(next);
    await engine.persist({ ipv6_enabled: next }, "inbounds");
  };

  const onTunStackChange = async (value: string) => {
    setTunStack(value);
    await engine.persist({ tun_stack: value }, "inbounds");
  };

  const onToggleTunAutoRoute = async (next: boolean) => {
    setTunAutoRoute(next);
    await engine.persist({ tun_auto_route: next }, "inbounds");
  };

  const onMixedPortChange = (raw: string) => {
    setMixedPortDraft(raw);
    engine.cancelPersist("mixed_port");
    if (!isValidPort(raw)) {
      return;
    }
    engine.schedulePersist("mixed_port", () => ({ mixed_port: Number(raw) }), "inbounds");
  };

  const onClashApiPortChange = (raw: string) => {
    setClashApiPortDraft(raw);
    engine.cancelPersist("clash_api_port");
    if (!isValidPort(raw)) {
      return;
    }
    engine.schedulePersist("clash_api_port", () => ({ clash_api_port: Number(raw) }), "clash_api");
  };

  const onClashApiSecretChange = (value: string) => {
    setClashApiSecretDraft(value);
    engine.cancelPersist("clash_api_secret");
    // 密钥必填（面板跳转携带密钥不允许为空）：空值只提示不落库。
    if (value.trim() === "") {
      return;
    }
    engine.schedulePersist("clash_api_secret", () => ({ clash_api_secret: value }), "clash_api");
  };

  const onGenerateClashApiSecret = () => {
    const secret = randomClashApiSecret();
    setClashApiSecretDraft(secret);
    engine.cancelPersist("clash_api_secret");
    void engine.persist({ clash_api_secret: secret }, "clash_api");
  };

  const onGithubProxyPrefixChange = (value: string) => {
    setGithubProxyPrefixDraft(value);
    engine.cancelPersist("github_proxy_prefix");
    engine.schedulePersist("github_proxy_prefix", () => ({ github_proxy_prefix: value }));
  };

  return {
    ready: config !== undefined,
    saving: engine.saving,
    vpnNotifySelection,
    onToggleVpnSelection,
    ipv6Enabled,
    onToggleIpv6,
    tunStack,
    onTunStackChange,
    tunAutoRoute,
    onToggleTunAutoRoute,
    clashApiPortDraft,
    clashApiPortError: portError(clashApiPortDraft),
    onClashApiPortChange,
    clashApiSecretDraft,
    clashApiSecretError: clashApiSecretDraft.trim() === "" ? CLASH_API_SECRET_REQUIRED_ERROR : null,
    onClashApiSecretChange,
    onGenerateClashApiSecret,
    mixedPortDraft,
    mixedPortError: portError(mixedPortDraft),
    onMixedPortChange,
    githubProxyPrefixDraft,
    onGithubProxyPrefixChange,
  };
}
