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
  /** 直接用返回值回写运行状态（供 `set_rule_mode` 等返回 status 的命令使用）。 */
  setStatus: (status: ClientStatus) => void;
  /** 保存配置；返回后端 `SaveConfigView.warning`（非阻塞提示，无则为 null）。 */
  saveConfig: (cfg: ClientConfig) => Promise<string | null>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  clearError: () => void;
}

/** `saveConfig` 串行化链：前一次保存完成后再执行下一个，避免两次保存交错时旧基底覆盖新修改。 */
let saveChain: Promise<unknown> = Promise.resolve();

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
      // 注意：轮询成功不再清 error，避免吞掉 start/stop/saveConfig 等操作刚记录的错误；
      // 错误由后续成功的 start/stop/saveConfig/loadConfig 清除。
      set({ status });
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

  setStatus: (status) => set({ status }),

  saveConfig: async (cfg) => {
    // 模块级 Promise 链串行化：前一次保存完成后才执行下一个，避免交错时旧基底覆盖新修改。
    const run = saveChain.then(async () => {
      set({ loading: true });
      try {
        const view = await saveConfigApi(cfg);
        set({ config: cfg, error: null });
        return view.warning ?? null;
      } catch (err) {
        set({ error: toErrorMessage(err) });
        throw err;
      } finally {
        set({ loading: false });
      }
    });
    // 链吞掉异常保证后续排队任务不被中断；返回值仍保留给调用方。
    saveChain = run.catch(() => undefined);
    return run;
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
