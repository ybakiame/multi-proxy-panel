import { create } from "zustand";
import {
  getConfig,
  listTraffic,
  proxyStatus,
  saveConfig as saveConfigApi,
  startProxy,
  stopProxy,
  toErrorMessage,
} from "./api";
import type { ClientConfig, ClientStatus, TrafficRecord } from "./api";

interface AppStore {
  /** 客户端配置（加载自 Tauri `get_config`）。 */
  config: ClientConfig | null;
  /** 代理运行状态。 */
  status: ClientStatus | null;
  /** MITM 抓包记录（当前后端恒返回空列表）。 */
  traffic: TrafficRecord[];
  /** 有异步命令在途。 */
  loading: boolean;
  /** 最近一次错误信息（null 表示无）。 */
  error: string | null;

  loadConfig: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  refreshTraffic: () => Promise<void>;
  saveConfig: (cfg: ClientConfig) => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  clearError: () => void;
}

export const useAppStore = create<AppStore>((set) => ({
  config: null,
  status: null,
  traffic: [],
  loading: false,
  error: null,

  loadConfig: async () => {
    try {
      const config = await getConfig();
      set({ config, error: null });
    } catch (err) {
      set({ error: toErrorMessage(err) });
    }
  },

  refreshStatus: async () => {
    try {
      const status = await proxyStatus();
      set({ status, error: null });
    } catch (err) {
      set({ error: toErrorMessage(err) });
    }
  },

  refreshTraffic: async () => {
    try {
      const traffic = await listTraffic();
      set({ traffic, error: null });
    } catch (err) {
      set({ error: toErrorMessage(err) });
    }
  },

  saveConfig: async (cfg) => {
    set({ loading: true });
    try {
      await saveConfigApi(cfg);
      set({ config: cfg, error: null });
    } catch (err) {
      set({ error: toErrorMessage(err) });
      throw err;
    } finally {
      set({ loading: false });
    }
  },

  start: async () => {
    set({ loading: true });
    try {
      const status = await startProxy();
      set({ status, error: null });
    } catch (err) {
      set({ error: toErrorMessage(err) });
      throw err;
    } finally {
      set({ loading: false });
    }
  },

  stop: async () => {
    set({ loading: true });
    try {
      const status = await stopProxy();
      set({ status, error: null });
    } catch (err) {
      set({ error: toErrorMessage(err) });
      throw err;
    } finally {
      set({ loading: false });
    }
  },

  clearError: () => set({ error: null }),
}));
