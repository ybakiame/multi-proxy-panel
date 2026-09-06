import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  CONFIG_KEY,
  notifyPrefsChanged,
  toErrorMessage,
  toastError,
  toastSuccess,
  toastWarning,
  useClientConfig,
  useSaveConfig,
} from "@pp/client-core";
import type { ClientConfig } from "@pp/client-core";

/** 端口越界/非法时展示在输入框下方的提示。 */
export const PORT_RANGE_ERROR = "端口需在 1-65535 之间";

/** 校验端口草稿：仅接受 1-65535 的整数。 */
export function isValidPort(raw: string): boolean {
  if (!/^\d+$/.test(raw)) {
    return false;
  }
  const value = Number(raw);
  return value >= 1 && value <= 65535;
}

/** 空白 / 非法端口的错误文案。 */
function portError(raw: string): string | null {
  if (raw.trim() === "") {
    return "请输入端口号";
  }
  return isValidPort(raw) ? null : PORT_RANGE_ERROR;
}

export interface UseSettingsConfigReturn {
  /** 配置是否已加载（控制各字段可用性，避免在默认值上误保存）。 */
  ready: boolean;
  /** 是否有保存进行中（即时保存按钮态 / 输入禁用可据此展示）。 */
  saving: boolean;
  vpnNotifyTraffic: boolean;
  vpnNotifySelection: boolean;
  onToggleVpnTraffic: (next: boolean) => Promise<void>;
  onToggleVpnSelection: (next: boolean) => Promise<void>;
  clashApiEnabled: boolean;
  onToggleClashApi: (next: boolean) => Promise<void>;
  clashApiPortDraft: string;
  clashApiPortError: string | null;
  onClashApiPortChange: (raw: string) => void;
  clashApiSecretDraft: string;
  onClashApiSecretChange: (value: string) => void;
  mixedPortDraft: string;
  mixedPortError: string | null;
  onMixedPortChange: (raw: string) => void;
  githubProxyPrefixDraft: string;
  onGithubProxyPrefixChange: (value: string) => void;
}

/**
 * 设置页表单状态 + 保存（对齐 desktop `useSettingsConfig`/persistConfig）：
 *
 * - 单一 Query 缓存（CONFIG_KEY）为权威源，config 变化时在渲染期同步各字段草稿
 *   （adjust-state-during-render，替代 effect 内 setState）；
 * - 保存：读缓存最新配置叠加补丁 → `useSaveConfig`（内部串行化）→ 成功 toast +
 *   失效 CONFIG_KEY 重读；失败 toast + 失效缓存回滚（草稿经 config 回流复位）；
 * - 文本/数字字段走 500ms 防抖，端口仅在 1-65535 合法时才落库（非法输入只提示不保存）；
 * - VPN 通知开关保存成功后追加 `notifyPrefsChanged` 热更新通知栏（仅核心运行中有效，
 *   失败 toast 警告不阻塞）。
 */
export function useSettingsConfig(): UseSettingsConfigReturn {
  const queryClient = useQueryClient();
  const { data: config } = useClientConfig();
  const saveConfigMutation = useSaveConfig();

  // ---- 表单草稿（config 变化时渲染期同步） ----
  const [vpnNotifyTraffic, setVpnNotifyTraffic] = useState(false);
  const [vpnNotifySelection, setVpnNotifySelection] = useState(false);
  const [clashApiEnabled, setClashApiEnabled] = useState(false);
  const [mixedPortDraft, setMixedPortDraft] = useState("");
  const [clashApiPortDraft, setClashApiPortDraft] = useState("");
  const [clashApiSecretDraft, setClashApiSecretDraft] = useState("");
  const [githubProxyPrefixDraft, setGithubProxyPrefixDraft] = useState("");

  const [prevConfig, setPrevConfig] = useState(config);
  if (prevConfig !== config) {
    setPrevConfig(config);
    if (config) {
      setVpnNotifyTraffic(config.vpn_notify_show_traffic);
      setVpnNotifySelection(config.vpn_notify_show_selection);
      setClashApiEnabled(config.clash_api_enabled);
      setMixedPortDraft(String(config.mixed_port));
      setClashApiPortDraft(String(config.clash_api_port));
      setClashApiSecretDraft(config.clash_api_secret ?? "");
      setGithubProxyPrefixDraft(config.github_proxy_prefix ?? "");
    }
  }

  // 各防抖字段独立计时（字段级 key），避免互相清掉对方待落库的保存。
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  /**
   * 配置即时保存：从 Query 缓存取最新配置叠加补丁（避免闭包旧值）。
   * 保存结果通过全局 toast 反馈；失败时失效 CONFIG_KEY 触发重读回滚。返回是否成功。
   */
  const persist = async (patch: Partial<ClientConfig>): Promise<boolean> => {
    const current = queryClient.getQueryData<ClientConfig>(CONFIG_KEY);
    if (!current) {
      return false;
    }
    try {
      const { warning } = await saveConfigMutation.mutateAsync({ ...current, ...patch });
      if (warning) {
        toastWarning(warning);
      } else {
        toastSuccess("设置已保存");
      }
      // useSaveConfig 已用入参回写缓存；失效共享 CONFIG_KEY 让其它消费者重读后端权威值。
      await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      // 保存失败回滚：失效缓存触发重读。
      await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      return false;
    }
  };

  /** 500ms 防抖保存；patch 以工厂形式读取执行时刻的最新草稿。 */
  const schedulePersist = (key: string, makePatch: () => Partial<ClientConfig>) => {
    const pending = timersRef.current.get(key);
    if (pending) {
      clearTimeout(pending);
    }
    timersRef.current.set(
      key,
      setTimeout(() => {
        timersRef.current.delete(key);
        void persist(makePatch());
      }, 500),
    );
  };

  /** 取消某字段尚未落库的防抖保存（端口变非法时丢弃旧值）。 */
  const cancelPersist = (key: string) => {
    const pending = timersRef.current.get(key);
    if (pending) {
      clearTimeout(pending);
      timersRef.current.delete(key);
    }
  };

  /** 保存 VPN 通知偏好成功后的通知栏热更新（失败仅 toast 警告，不阻塞主流程）。 */
  const notifyVpnPrefs = async (showTraffic: boolean, showSelection: boolean) => {
    try {
      await notifyPrefsChanged(showTraffic, showSelection);
    } catch (err) {
      toastWarning(`通知栏偏好热更新失败：${toErrorMessage(err)}（核心未运行时将在下次启动生效）`);
    }
  };

  const onToggleVpnTraffic = async (next: boolean) => {
    setVpnNotifyTraffic(next);
    if (!(await persist({ vpn_notify_show_traffic: next }))) {
      return;
    }
    await notifyVpnPrefs(next, vpnNotifySelection);
  };

  const onToggleVpnSelection = async (next: boolean) => {
    setVpnNotifySelection(next);
    if (!(await persist({ vpn_notify_show_selection: next }))) {
      return;
    }
    await notifyVpnPrefs(vpnNotifyTraffic, next);
  };

  const onToggleClashApi = async (next: boolean) => {
    setClashApiEnabled(next);
    await persist({ clash_api_enabled: next });
  };

  const onMixedPortChange = (raw: string) => {
    setMixedPortDraft(raw);
    cancelPersist("mixed_port");
    if (!isValidPort(raw)) {
      return;
    }
    schedulePersist("mixed_port", () => ({ mixed_port: Number(raw) }));
  };

  const onClashApiPortChange = (raw: string) => {
    setClashApiPortDraft(raw);
    cancelPersist("clash_api_port");
    if (!isValidPort(raw)) {
      return;
    }
    schedulePersist("clash_api_port", () => ({ clash_api_port: Number(raw) }));
  };

  const onClashApiSecretChange = (value: string) => {
    setClashApiSecretDraft(value);
    cancelPersist("clash_api_secret");
    schedulePersist("clash_api_secret", () => ({ clash_api_secret: value }));
  };

  const onGithubProxyPrefixChange = (value: string) => {
    setGithubProxyPrefixDraft(value);
    cancelPersist("github_proxy_prefix");
    schedulePersist("github_proxy_prefix", () => ({ github_proxy_prefix: value }));
  };

  return {
    ready: config !== null,
    saving: saveConfigMutation.isPending,
    vpnNotifyTraffic,
    vpnNotifySelection,
    onToggleVpnTraffic,
    onToggleVpnSelection,
    clashApiEnabled,
    onToggleClashApi,
    clashApiPortDraft,
    clashApiPortError: portError(clashApiPortDraft),
    onClashApiPortChange,
    clashApiSecretDraft,
    onClashApiSecretChange,
    mixedPortDraft,
    mixedPortError: portError(mixedPortDraft),
    onMixedPortChange,
    githubProxyPrefixDraft,
    onGithubProxyPrefixChange,
  };
}
