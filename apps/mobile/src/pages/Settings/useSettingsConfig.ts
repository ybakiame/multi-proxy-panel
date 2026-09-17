import { useEffect, useRef, useState } from "react";
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
 * 表单状态 + 保存（对齐 desktop `useSettingsConfig`/persistConfig）：
 *
 * - 单一 Query 缓存（CONFIG_KEY）为权威源，config 变化时在渲染期同步各字段草稿
 *   （adjust-state-during-render，替代 effect 内 setState）；
 * - 保存：读缓存最新配置叠加补丁 → `useSaveConfig`（内部串行化）→ 成功 toast +
 *   失效 CONFIG_KEY 重读；失败 toast + 失效缓存回滚（草稿经 config 回流复位）；
 *   persist 自身再经本地串行链排队，后一次保存叠加在前一次结果上（不丢并发修改）；
 * - 文本/数字字段走 500ms 防抖，端口仅在 1-65535 合法时才落库（非法输入只提示不保存）；
 * - 组件卸载（切页）时 flush 未落库的防抖修改，立即保存，不依赖悬空 timer；
 * - VPN 通知开关保存成功后追加 `notifyPrefsChanged` 热更新通知栏（仅核心运行中有效，
 *   失败 toast 警告不阻塞）。
 */

/** 某字段待落库的防抖保存：timer 句柄 + 执行时刻最新草稿的补丁工厂。 */
interface PendingSave {
  timer: ReturnType<typeof setTimeout>;
  makePatch: () => Partial<ClientConfig>;
}

export function useSettingsConfig(): UseSettingsConfigReturn {
  const queryClient = useQueryClient();
  const { data: config } = useClientConfig();
  const saveConfigMutation = useSaveConfig();

  // ---- 表单草稿（config 变化时渲染期同步） ----
  const [vpnNotifyTraffic, setVpnNotifyTraffic] = useState(false);
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
      setVpnNotifyTraffic(config.vpn_notify_show_traffic);
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

  // 各防抖字段独立计时（字段级 key），避免互相清掉对方待落库的保存。
  // 每条 pending 记录同时持有补丁工厂，供卸载 flush 读取最新草稿。
  const pendingRef = useRef<Map<string, PendingSave>>(new Map());

  /** persist 串行链：前一次落库并回写缓存后再读基底，避免并发保存互相覆盖。 */
  const persistChainRef = useRef<Promise<void>>(Promise.resolve());

  /**
   * 配置即时保存：从 Query 缓存取最新配置叠加补丁（避免闭包旧值）。
   * 保存结果通过全局 toast 反馈；失败时失效 CONFIG_KEY 触发重读回滚。返回是否成功。
   *
   * 入队到 persistChainRef 串行执行：每次执行都先等前一次 persist 完成
   * （useSaveConfig 的 onSuccess 已用其入参回写 CONFIG_KEY 缓存）再读基底，
   * 保证后一次保存永远叠加在前一次结果之上，不会用旧基底整对象覆盖新修改。
   */
  const persist = (patch: Partial<ClientConfig>): Promise<boolean> => {
    const run = persistChainRef.current.then(async () => {
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
    });
    // 链吞掉异常保证后续排队任务不被中断；返回值仍保留给调用方。
    persistChainRef.current = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  };

  // persist 每次渲染重建；每次 commit 后在 effect 中经 ref 暴露最新实例
  // （供防抖回调与卸载 flush 使用；ref 只允许在 effect/handler 内读写）。
  const persistRef = useRef(persist);
  useEffect(() => {
    persistRef.current = persist;
  });

  /** 500ms 防抖保存；patch 以工厂形式读取执行时刻的最新草稿。 */
  const schedulePersist = (key: string, makePatch: () => Partial<ClientConfig>) => {
    const pending = pendingRef.current.get(key);
    if (pending) {
      clearTimeout(pending.timer);
    }
    pendingRef.current.set(key, {
      makePatch,
      timer: setTimeout(() => {
        pendingRef.current.delete(key);
        void persistRef.current(makePatch());
      }, 500),
    });
  };

  /** 取消某字段尚未落库的防抖保存（端口变非法时丢弃旧值）。 */
  const cancelPersist = (key: string) => {
    const pending = pendingRef.current.get(key);
    if (pending) {
      clearTimeout(pending.timer);
      pendingRef.current.delete(key);
    }
  };

  // 卸载时 flush 未落库的防抖修改：路由切页/组件销毁不再依赖悬空 timer 存活
  // （真机 WebView 上 timer 随页面销毁/后台可能被丢弃），而是立即执行保存，
  // 修复「修改字段 → 500ms 内切页 → 保存从未发生 → 切回页面回显旧值」的竞态。
  useEffect(() => {
    return () => {
      const pending = pendingRef.current;
      if (pending.size === 0) {
        return;
      }
      pendingRef.current = new Map();
      const patches: Partial<ClientConfig>[] = [];
      pending.forEach(({ timer, makePatch }) => {
        clearTimeout(timer);
        patches.push(makePatch());
      });
      // 字段补丁互不重叠，合并为单次保存（单一 toast，也避免分次落库互相覆盖）。
      const merged = Object.assign({}, ...patches) as Partial<ClientConfig>;
      void persistRef.current(merged);
    };
  }, []);

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

  const onToggleIpv6 = async (next: boolean) => {
    setIpv6Enabled(next);
    await persist({ ipv6_enabled: next });
  };

  const onTunStackChange = async (value: string) => {
    setTunStack(value);
    await persist({ tun_stack: value });
  };

  const onToggleTunAutoRoute = async (next: boolean) => {
    setTunAutoRoute(next);
    await persist({ tun_auto_route: next });
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
    // 密钥必填（面板跳转携带密钥不允许为空）：空值只提示不落库。
    if (value.trim() === "") {
      return;
    }
    schedulePersist("clash_api_secret", () => ({ clash_api_secret: value }));
  };

  const onGenerateClashApiSecret = () => {
    const secret = randomClashApiSecret();
    setClashApiSecretDraft(secret);
    cancelPersist("clash_api_secret");
    void persist({ clash_api_secret: secret });
  };

  const onGithubProxyPrefixChange = (value: string) => {
    setGithubProxyPrefixDraft(value);
    cancelPersist("github_proxy_prefix");
    schedulePersist("github_proxy_prefix", () => ({ github_proxy_prefix: value }));
  };

  return {
    ready: config !== undefined,
    saving: saveConfigMutation.isPending,
    vpnNotifyTraffic,
    vpnNotifySelection,
    onToggleVpnTraffic,
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
