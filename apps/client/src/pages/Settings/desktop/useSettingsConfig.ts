import { useCallback, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useAtomValue, useSetAtom } from "jotai";
import { tunAuthStatus } from "@pp/client-core";
import { TUN_AUTH_KEY } from "@pp/client-core";
import { lastActionErrorAtom } from "@pp/client-core";
import { useClientConfig, useSettingsPersist } from "@pp/client-core";
import { PORT_RANGE_ERROR, isValidPort, portError } from "@pp/client-core";
import type { ClientConfig, RestartDirtyKey } from "@pp/client-core";

// TODO: read version from package.json (build-time injection or runtime read)

export { PORT_RANGE_ERROR, isValidPort };

/** TUN 协议栈选项（与后端 `ClientConfigView.tun_stack` 的 serde 值一致）。 */
export const TUN_STACK_OPTIONS = [
  { id: "mixed", label: "mixed" },
  { id: "gvisor", label: "gvisor" },
  { id: "system", label: "system" },
] as const;

/** Clash 面板 UI 选项（与后端 `ClientConfigView.clash_api_ui` 的 serde 值一致，默认 `zashboard`）。 */
export const CLASH_UI_OPTIONS = [
  { id: "zashboard", label: "zashboard" },
  { id: "yacd", label: "yacd" },
  { id: "metacubexd", label: "metacubexd" },
] as const;

/**
 * 需重启核心才生效的 ClientConfig 字段 → 脏顶级配置域映射（RestartPrompt 消费）。
 * 未列出的字段（github_proxy_prefix / fetch_via_local_proxy 等）不影响运行中核心，不上报。
 */
const RESTART_KEY_BY_FIELD: Record<string, RestartDirtyKey> = {
  mixed_port: "inbounds",
  ipv6_enabled: "inbounds",
  tun_enabled: "inbounds",
  tun_stack: "inbounds",
  tun_auto_route: "inbounds",
  clash_api_enabled: "clash_api",
  clash_api_port: "clash_api",
  clash_api_secret: "clash_api",
  clash_api_ui: "clash_api",
  dns_fakeip_enabled: "dns",
};

/** 按补丁字段推导重启脏标记键（桌面端补丁可能多字段，逐字段映射去重）。 */
function restartKeysOf(patch: Partial<ClientConfig>): RestartDirtyKey[] {
  const keys = new Set<RestartDirtyKey>();
  for (const field of Object.keys(patch)) {
    const key = RESTART_KEY_BY_FIELD[field];
    if (key) {
      keys.add(key);
    }
  }
  return [...keys];
}

export interface UseSettingsConfigReturn {
  config: ClientConfig | null;
  error: string | null;
  /** 混合端口草稿（字符串，输入校验后合法才落库）。 */
  mixedPortDraft: string;
  mixedPortError: string | null;
  onMixedPortChange: (raw: string) => void;
  ipv6Enabled: boolean;
  setIpv6Enabled: (value: boolean) => void;
  tunEnabled: boolean;
  setTunEnabled: (value: boolean) => void;
  tunStack: string;
  setTunStack: (value: string) => void;
  tunAutoRoute: boolean;
  setTunAutoRoute: (value: boolean) => void;
  tunAuth: string | null;
  tunAuthError: string | null;
  tunAuthBusy: boolean;
  setTunAuthBusy: (value: boolean) => void;
  setTunAuth: (value: string | null) => void;
  setTunAuthError: (value: string | null) => void;
  clashApiEnabled: boolean;
  setClashApiEnabled: (value: boolean) => void;
  /** Clash API 端口草稿（字符串，输入校验后合法才落库）。 */
  clashApiPortDraft: string;
  clashApiPortError: string | null;
  onClashApiPortChange: (raw: string) => void;
  clashApiSecret: string;
  setClashApiSecret: (value: string) => void;
  clashApiUi: string;
  setClashApiUi: (value: string) => void;
  githubProxyPrefix: string;
  setGithubProxyPrefix: (value: string) => void;
  fetchViaLocalProxy: boolean;
  setFetchViaLocalProxy: (value: boolean) => void;
  proxyTestPending: boolean;
  proxyTestResult: string | null;
  proxyTestError: string | null;
  setProxyTestPending: (value: boolean) => void;
  setProxyTestResult: (value: string | null) => void;
  setProxyTestError: (value: string | null) => void;
  persist: (patch: Partial<ClientConfig>) => Promise<void>;
  persistDebounced: (patch: Partial<ClientConfig>) => void;
}

/**
 * 桌面设置页视图模型（ADR-0011 试点）：保存引擎复用 client-core 共享的
 * [`useSettingsPersist`]（persist 串行链 + 字段级防抖 + 卸载 flush），
 * 本 hook 只保留桌面专属逻辑：RESTART_KEY_BY_FIELD 派生、TUN 授权态查询、
 * TUN 关闭时残留授权错误清理、Clash 面板 UI 选择与代理连通性测试状态。
 */
export function useSettingsConfig(): UseSettingsConfigReturn {
  const queryClient = useQueryClient();
  // 配置与共享错误均以 Query 缓存 / jotai atom 为权威源（替代原 store 双写）。
  const { data: config = null } = useClientConfig();
  const error = useAtomValue(lastActionErrorAtom);
  const setLastError = useSetAtom(lastActionErrorAtom);
  const engine = useSettingsPersist();
  const [mixedPortDraft, setMixedPortDraft] = useState("17890");
  const [ipv6Enabled, setIpv6Enabled] = useState(false);
  const [tunEnabled, setTunEnabled] = useState(false);
  const [tunStack, setTunStack] = useState<string>("mixed");
  const [tunAutoRoute, setTunAutoRoute] = useState(true);
  const [tunAuthError, setTunAuthError] = useState<string | null>(null);
  const [tunAuthBusy, setTunAuthBusy] = useState(false);
  const [clashApiEnabled, setClashApiEnabled] = useState(false);
  const [clashApiPortDraft, setClashApiPortDraft] = useState("9090");
  const [clashApiSecret, setClashApiSecret] = useState("");
  const [clashApiUi, setClashApiUi] = useState<string>("zashboard");
  const [githubProxyPrefix, setGithubProxyPrefix] = useState("");
  const [fetchViaLocalProxy, setFetchViaLocalProxy] = useState(false);
  const [proxyTestPending, setProxyTestPending] = useState(false);
  const [proxyTestResult, setProxyTestResult] = useState<string | null>(null);
  const [proxyTestError, setProxyTestError] = useState<string | null>(null);

  // config 变化时在渲染期间同步表单本地状态（React 推荐的 adjust-state-during-render
  // 模式，替代 effect 内同步 setState，满足 React Compiler 的 set-state-in-effect 限制）。
  // prevConfig 初始为 null（而非 config）：挂载时 Query 缓存若已就绪（如从首页导航
  // 过来），`prevConfig === config` 会导致首次同步被跳过、表单停留在 useState 默认值
  // （如 TUN 实际开启却显示关闭）——以 null 为哨兵保证首次渲染即同步。
  const [prevConfig, setPrevConfig] = useState<ClientConfig | null>(null);
  if (prevConfig !== config) {
    setPrevConfig(config);
    if (config) {
      setMixedPortDraft(String(config.mixed_port));
      setIpv6Enabled(config.ipv6_enabled);
      setTunEnabled(config.tun_enabled);
      setTunStack(config.tun_stack);
      setTunAutoRoute(config.tun_auto_route);
      setClashApiEnabled(config.clash_api_enabled);
      setClashApiPortDraft(String(config.clash_api_port));
      setClashApiSecret(config.clash_api_secret);
      setClashApiUi(config.clash_api_ui || "zashboard");
      setGithubProxyPrefix(config.github_proxy_prefix || "");
      setFetchViaLocalProxy(config.fetch_via_local_proxy);
    }
  }

  /**
   * 配置即时保存：委托共享引擎（串行链 + 重启脏标记按补丁字段派生）。
   * 关闭 TUN 后，此前「TUN 未授权」启动失败残留在共享错误里的记录不再适用：
   * 桌面端 TUN 为可选模式，未启用时启动不再需要授权，清除避免 Dashboard
   * 继续展示授权门禁（该门禁另按 config.tun_enabled 守卫，双保险）。
   */
  const persist = useCallback(
    async (patch: Partial<ClientConfig>) => {
      const ok = await engine.persist(patch, restartKeysOf(patch));
      if (ok && patch.tun_enabled === false) {
        setLastError((current) => (current?.includes("tun_auth_required") ? null : current));
      }
    },
    [engine, setLastError],
  );

  /** 字段级防抖保存：补丁首字段名为 key（桌面补丁均单字段），非法输入由调用方先行取消。 */
  const persistDebounced = useCallback(
    (patch: Partial<ClientConfig>) => {
      const field = Object.keys(patch)[0] ?? "patch";
      engine.schedulePersist(field, () => patch, restartKeysOf(patch));
    },
    [engine],
  );

  /** 混合端口输入：草稿即时更新，仅合法值（1-65535 整数）防抖落库；非法时取消待落库保存。 */
  const onMixedPortChange = useCallback(
    (raw: string) => {
      setMixedPortDraft(raw);
      engine.cancelPersist("mixed_port");
      if (portError(raw) !== null) {
        return;
      }
      persistDebounced({ mixed_port: Number(raw) });
    },
    [engine, persistDebounced],
  );

  /** Clash API 端口输入：同混合端口，仅合法值防抖落库。 */
  const onClashApiPortChange = useCallback(
    (raw: string) => {
      setClashApiPortDraft(raw);
      engine.cancelPersist("clash_api_port");
      if (portError(raw) !== null) {
        return;
      }
      persistDebounced({ clash_api_port: Number(raw) });
    },
    [engine, persistDebounced],
  );

  // TUN 授权状态查询（桌面端且 TUN 启用时），Query 缓存为权威源。
  const { data: tunAuthData } = useQuery<string>({
    queryKey: TUN_AUTH_KEY,
    queryFn: tunAuthStatus,
    enabled: tunEnabled,
    retry: false,
  });
  // TUN 关闭时不展示授权状态。
  const tunAuth = tunEnabled ? (tunAuthData ?? null) : null;
  // 授权操作后直接回写缓存（NetworkSettings 消费）。
  const setTunAuth = useCallback(
    (value: string | null) => {
      queryClient.setQueryData(TUN_AUTH_KEY, value);
    },
    [queryClient],
  );

  return {
    config,
    error,
    mixedPortDraft,
    mixedPortError: portError(mixedPortDraft),
    onMixedPortChange,
    ipv6Enabled,
    setIpv6Enabled,
    tunEnabled,
    setTunEnabled,
    tunStack,
    setTunStack,
    tunAutoRoute,
    setTunAutoRoute,
    tunAuth,
    tunAuthError,
    tunAuthBusy,
    setTunAuthBusy,
    setTunAuth,
    setTunAuthError,
    clashApiEnabled,
    setClashApiEnabled,
    clashApiPortDraft,
    clashApiPortError: portError(clashApiPortDraft),
    onClashApiPortChange,
    clashApiSecret,
    setClashApiSecret,
    clashApiUi,
    setClashApiUi,
    githubProxyPrefix,
    setGithubProxyPrefix,
    fetchViaLocalProxy,
    setFetchViaLocalProxy,
    proxyTestPending,
    proxyTestResult,
    proxyTestError,
    setProxyTestPending,
    setProxyTestResult,
    setProxyTestError,
    persist,
    persistDebounced,
  };
}
