import { useCallback, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useAtomValue } from "jotai";
import { toastError, toastSuccess, toastWarning } from "@pp/client-core";
import { markRestartRequired } from "@pp/client-core";
import { toErrorMessage, tunAuthStatus } from "@pp/client-core";
import { CONFIG_KEY, PROXY_STATUS_KEY, TUN_AUTH_KEY } from "@pp/client-core";
import { lastActionErrorAtom } from "@pp/client-core";
import { useClientConfig, useSaveConfig } from "@pp/client-core";
import type { ClientConfig, ClientStatus, RestartDirtyKey } from "@pp/client-core";

// TODO: read version from package.json (build-time injection or runtime read)
export const APP_VERSION = "0.1.0";

/** TUN 协议栈选项（与后端 `ClientConfigView.tun_stack` 的 serde 值一致）。 */
export const TUN_STACK_OPTIONS = [
  { id: "mixed", label: "mixed" },
  { id: "gvisor", label: "gvisor" },
  { id: "system", label: "system" },
] as const;

/** Clash 面板 UI 选项（与后端 `ClientConfigView.clash_api_ui` 的 serde 值一致，默认 zashboard）。 */
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

/** 端口越界/非法时展示在输入框下方的提示（对齐移动端 `isValidPort` 校验）。 */
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
  clashApiPort: number;
  setClashApiPort: (value: number) => void;
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

export function useSettingsConfig(): UseSettingsConfigReturn {
  const queryClient = useQueryClient();
  // 配置与共享错误均以 Query 缓存 / jotai atom 为权威源（替代原 store 双写）。
  const { data: config = null } = useClientConfig();
  const error = useAtomValue(lastActionErrorAtom);
  const saveConfigMutation = useSaveConfig();
  const [mixedPortDraft, setMixedPortDraft] = useState("17890");
  const [ipv6Enabled, setIpv6Enabled] = useState(false);
  const [tunEnabled, setTunEnabled] = useState(false);
  const [tunStack, setTunStack] = useState<string>("mixed");
  const [tunAutoRoute, setTunAutoRoute] = useState(true);
  const [tunAuthError, setTunAuthError] = useState<string | null>(null);
  const [tunAuthBusy, setTunAuthBusy] = useState(false);
  const [clashApiEnabled, setClashApiEnabled] = useState(false);
  const [clashApiPort, setClashApiPort] = useState(9090);
  const [clashApiSecret, setClashApiSecret] = useState("");
  const [clashApiUi, setClashApiUi] = useState<string>("zashboard");
  const [githubProxyPrefix, setGithubProxyPrefix] = useState("");
  const [fetchViaLocalProxy, setFetchViaLocalProxy] = useState(false);
  const [proxyTestPending, setProxyTestPending] = useState(false);
  const [proxyTestResult, setProxyTestResult] = useState<string | null>(null);
  const [proxyTestError, setProxyTestError] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // config 变化时在渲染期间同步表单本地状态（React 推荐的 adjust-state-during-render
  // 模式，替代 effect 内同步 setState，满足 React Compiler 的 set-state-in-effect 限制）。
  const [prevConfig, setPrevConfig] = useState(config);
  if (prevConfig !== config) {
    setPrevConfig(config);
    if (config) {
      setMixedPortDraft(String(config.mixed_port));
      setIpv6Enabled(config.ipv6_enabled);
      setTunEnabled(config.tun_enabled);
      setTunStack(config.tun_stack);
      setTunAutoRoute(config.tun_auto_route);
      setClashApiEnabled(config.clash_api_enabled);
      setClashApiPort(config.clash_api_port);
      setClashApiSecret(config.clash_api_secret);
      setClashApiUi(config.clash_api_ui || "zashboard");
      setGithubProxyPrefix(config.github_proxy_prefix || "");
      setFetchViaLocalProxy(config.fetch_via_local_proxy);
    }
  }

  /**
   * 配置即时保存：从 Query 缓存取最新配置叠加补丁（避免闭包旧值）。
   * 保存结果通过全局 toast 反馈；失败时失效 CONFIG_KEY 重读回滚。
   */
  const persist = useCallback(
    async (patch: Partial<ClientConfig>) => {
      const current = queryClient.getQueryData<ClientConfig>(CONFIG_KEY);
      if (!current) {
        return;
      }
      try {
        const { warning } = await saveConfigMutation.mutateAsync({ ...current, ...patch });
        if (warning) {
          toastWarning(warning);
        } else {
          toastSuccess("设置已保存");
        }
        // 保存成功后 useSaveConfig 已用入参回写缓存；这里失效共享的
        // ["config"] Query 缓存让其它消费者重读，而不是再手动 invoke 一遍。
        await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
        // 需重启的字段上报全局脏标记（RestartPrompt 消费）；核心未运行时配置随
        // 下次启动生效，无需提示（markRestartRequired 内部已判）。
        const coreRunning = queryClient.getQueryData<ClientStatus>(PROXY_STATUS_KEY)?.core_running ?? false;
        for (const field of Object.keys(patch)) {
          const dirtyKey = RESTART_KEY_BY_FIELD[field];
          if (dirtyKey) {
            markRestartRequired(dirtyKey, coreRunning);
          }
        }
      } catch (err) {
        toastError(toErrorMessage(err));
        // 保存失败回滚：失效缓存触发重读
        await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      }
    },
    [queryClient, saveConfigMutation],
  );

  const persistDebounced = useCallback(
    (patch: Partial<ClientConfig>) => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
      debounceRef.current = setTimeout(() => void persist(patch), 500);
    },
    [persist],
  );

  /** 混合端口输入：草稿即时更新，仅合法值（1-65535 整数）防抖落库。 */
  const onMixedPortChange = useCallback(
    (raw: string) => {
      setMixedPortDraft(raw);
      if (portError(raw) !== null) {
        return;
      }
      persistDebounced({ mixed_port: Number(raw) });
    },
    [persistDebounced],
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
    clashApiPort,
    setClashApiPort,
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
