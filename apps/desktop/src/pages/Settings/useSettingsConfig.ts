import { useCallback, useEffect, useRef, useState } from "react";
import { useAppStore } from "../../store";
import { toastError, toastSuccess, toastWarning } from "../../toast";
import { toErrorMessage, platformInfo, tunAuthStatus } from "../../api";
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
  refreshNotifPerm: () => Promise<void>;
  refreshTunAuth: () => Promise<void>;
  loadConfig: () => Promise<void>;
}

export function useSettingsConfig(): UseSettingsConfigReturn {
  const { config, error, loadConfig, saveConfig: storeSaveConfig } = useAppStore();
  const [os, setOs] = useState<string | null>(null);
  const [mixedPort, setMixedPort] = useState(1080);
  const [tunEnabled, setTunEnabled] = useState(false);
  const [tunStack, setTunStack] = useState<string>("mixed");
  const [tunAutoRoute, setTunAutoRoute] = useState(true);
  const [tunAuth, setTunAuth] = useState<string | null>(null);
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
  const [notifPerm, setNotifPerm] = useState<string>("unknown");
  const [notifPermBusy, setNotifPermBusy] = useState(false);

  useEffect(() => {
    void loadConfig();
    void platformInfo()
      .then((info) => setOs(info.os))
      .catch(() => {
        // command failure keeps unknown platform (render as desktop)
      });
  }, [loadConfig]);

  useEffect(() => {
    if (!config) {
      return;
    }
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
  }, [config]);

  const isAndroid = os === "android";

  const refreshNotifPerm = useCallback(async () => {
    try {
      const granted = await isPermissionGranted();
      setNotifPerm(granted ? "granted" : "denied");
    } catch {
      setNotifPerm("unknown");
    }
  }, []);

  useEffect(() => {
    if (isAndroid) {
      void refreshNotifPerm();
    }
  }, [isAndroid, refreshNotifPerm]);

  const persist = useCallback(
    async (patch: Partial<ClientConfig>) => {
      const current = useAppStore.getState().config;
      if (!current) {
        return;
      }
      try {
        const warning = await storeSaveConfig({ ...current, ...patch });
        if (warning) {
          toastWarning(warning);
        } else {
          toastSuccess("设置已保存");
        }
        await loadConfig();
      } catch (err) {
        toastError(toErrorMessage(err));
        await loadConfig();
      }
    },
    [storeSaveConfig, loadConfig],
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

  const refreshTunAuth = useCallback(async () => {
    try {
      setTunAuth(await tunAuthStatus());
      setTunAuthError(null);
    } catch (err) {
      setTunAuthError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    if (!tunEnabled) {
      setTunAuth(null);
      setTunAuthError(null);
      return;
    }
    void refreshTunAuth();
  }, [tunEnabled, refreshTunAuth]);

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
    refreshNotifPerm,
    refreshTunAuth,
    loadConfig,
  };
}
