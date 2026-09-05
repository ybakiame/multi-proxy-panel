import { useCallback, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useAtomValue } from "jotai";
import { toastError, toastSuccess, toastWarning } from "../../toast";
import { toErrorMessage, platformInfo, tunAuthStatus } from "../../api";
import { CONFIG_KEY, NOTIF_PERM_KEY, PLATFORM_KEY, TUN_AUTH_KEY } from "../../api/keys";
import { lastActionErrorAtom } from "../../atoms/ui";
import { useClientConfig, useSaveConfig } from "../../hooks/useClientConfig";
import { isPermissionGranted } from "@tauri-apps/plugin-notification";
import type { ClientConfig } from "../../api";

// TODO: read version from package.json (build-time injection or runtime read)
export const APP_VERSION = "0.1.0";

/** 兼容任务描述中的 PascalCase 值（`SingBox`/`Mihomo`）与后端 serde 值（`singbox`/`mihomo`）。 */
export function normalizeCoreType(value: string): string {
  if (value === "SingBox") {
    return "singbox";
  }
  if (value === "Mihomo") {
    return "mihomo";
  }
  return value;
}

/** 核心类型展示名（与复写页的 CORE_LABELS 一致）。 */
export const CORE_LABELS: Record<string, string> = {
  singbox: "sing-box",
  mihomo: "mihomo",
};

/** 核心类型 Chip 配色：sing-box 用强调色、mihomo 用警告色区分。 */
export const CORE_CHIP_COLORS: Record<string, "accent" | "warning"> = {
  singbox: "accent",
  mihomo: "warning",
};

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

export interface UseSettingsConfigReturn {
  config: ClientConfig | null;
  error: string | null;
  os: string | null;
  isAndroid: boolean;
  mixedPort: number;
  setMixedPort: (value: number) => void;
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
  notifPerm: string;
  setNotifPerm: (value: string) => void;
  notifPermBusy: boolean;
  setNotifPermBusy: (value: boolean) => void;
}

export function useSettingsConfig(): UseSettingsConfigReturn {
  const queryClient = useQueryClient();
  // 配置与共享错误均以 Query 缓存 / jotai atom 为权威源（替代原 store 双写）。
  const { data: config = null } = useClientConfig();
  const error = useAtomValue(lastActionErrorAtom);
  const saveConfigMutation = useSaveConfig();
  const [mixedPort, setMixedPort] = useState(1080);
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
  const [notifPermBusy, setNotifPermBusy] = useState(false);

  // 平台探测（Android 显示「打开下载目录」引导；失败按桌面渲染）。
  const { data: platformData } = useQuery<{ os: string }>({
    queryKey: PLATFORM_KEY,
    queryFn: platformInfo,
    staleTime: Infinity,
    retry: false,
  });

  const os = platformData?.os ?? null;

  // config 变化时在渲染期间同步表单本地状态（React 推荐的 adjust-state-during-render
  // 模式，替代 effect 内同步 setState，满足 React Compiler 的 set-state-in-effect 限制）。
  const [prevConfig, setPrevConfig] = useState(config);
  if (prevConfig !== config) {
    setPrevConfig(config);
    if (config) {
      setMixedPort(config.mixed_port);
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

  const isAndroid = os === "android";

  // Android 通知权限查询（Query 缓存为权威源；失败保持 unknown）。
  const { data: notifPermData } = useQuery<string>({
    queryKey: NOTIF_PERM_KEY,
    queryFn: async () => ((await isPermissionGranted()) ? "granted" : "denied"),
    enabled: isAndroid,
    retry: false,
  });
  const notifPerm = notifPermData ?? "unknown";
  // 申请权限后直接回写缓存（NotificationSettings 消费）。
  const setNotifPerm = useCallback(
    (value: string) => {
      queryClient.setQueryData(NOTIF_PERM_KEY, value);
    },
    [queryClient],
  );

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

  // TUN 授权状态查询（桌面端且 TUN 启用时），Query 缓存为权威源。
  const { data: tunAuthData } = useQuery<string>({
    queryKey: TUN_AUTH_KEY,
    queryFn: tunAuthStatus,
    enabled: !isAndroid && tunEnabled,
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
    os,
    isAndroid,
    mixedPort,
    setMixedPort,
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
    notifPerm,
    setNotifPerm,
    notifPermBusy,
    setNotifPermBusy,
  };
}
