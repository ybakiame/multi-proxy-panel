import { create } from "zustand";

/**
 * 配置变更「需重启核心」脏标记 store（纯逻辑，无 UI；ADR-0003 客户端）。
 *
 * 背景：核心运行中修改配置（DNS/出站/路由/规则等切片与部分 ClientConfig 字段）
 * 不会热更新，须重启核心才生效。各保存成功路径调用 `markRestartRequired` 上报
 * 脏顶级配置域；壳 UI（当前为 mobile `RestartPrompt`）消费本 store 呈现
 * 「弹窗一次 + 悬浮按钮」的重启引导；核心停止/重启成功后 `reset`。
 *
 * 设计文档：docs/plans/2026-09-30-mobile-restart-prompt-design.md
 */

/** 需重启的顶级配置域（key = 域 id，上报与展示共用）。 */
export type RestartDirtyKey =
  | "dns"
  | "outbounds"
  | "route"
  | "experimental"
  | "rules"
  | "rulesets"
  | "inbounds"
  | "clash_api"
  | "subscription";

/** 顶级配置域中文展示名（变更项列表展示）。 */
export const RESTART_DIRTY_LABELS: Record<RestartDirtyKey, string> = {
  dns: "DNS 管理",
  outbounds: "出站管理",
  route: "路由管理",
  experimental: "实验性配置",
  rules: "规则管理",
  rulesets: "规则集管理",
  inbounds: "入站管理",
  clash_api: "Clash API",
  subscription: "订阅",
};

interface PendingRestartStore {
  /** 脏顶级配置域（key 保序插入，展示按插入顺序）。 */
  dirtyKeys: ReadonlyMap<RestartDirtyKey, string>;
  /** 本次脏周期内自动弹窗是否已被「稍后」收起（收起后由悬浮按钮承接）。 */
  dismissed: boolean;
  /** 上报一个需重启的配置域（幂等，同 key 重复上报无副作用）。 */
  markDirty: (key: RestartDirtyKey) => void;
  /** 弹窗「稍后」：本脏周期内不再自动弹窗，收为悬浮按钮。 */
  dismiss: () => void;
  /** 核心停止/重启成功后复位（清空脏标记与 dismissed）。 */
  reset: () => void;
}

export const usePendingRestartStore = create<PendingRestartStore>((set) => ({
  dirtyKeys: new Map(),
  dismissed: false,
  markDirty: (key) =>
    set((state) => {
      if (state.dirtyKeys.has(key)) {
        return state;
      }
      const next = new Map(state.dirtyKeys);
      next.set(key, RESTART_DIRTY_LABELS[key]);
      return { dirtyKeys: next };
    }),
  dismiss: () => set({ dismissed: true }),
  reset: () => set({ dirtyKeys: new Map(), dismissed: false }),
}));

/** 是否有待重启的脏配置（选择器便捷函数）。 */
export function selectHasPendingRestart(state: PendingRestartStore): boolean {
  return state.dirtyKeys.size > 0;
}

/**
 * 保存成功路径统一上报入口：仅核心运行中才入队（未运行配置随下次启动生效，
 * 无需提示）。
 */
export function markRestartRequired(key: RestartDirtyKey, coreRunning: boolean): void {
  if (!coreRunning) {
    return;
  }
  usePendingRestartStore.getState().markDirty(key);
}
