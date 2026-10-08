/**
 * 设置页共享保存引擎（ADR-0011 D2 试点）：persist 串行链 + 字段级防抖 +
 * 卸载 flush + 端口校验。
 *
 * 语义取自移动端 `useSettingsConfig`（双端中更健壮的实现，逐字段防抖 + 补丁工厂 +
 * 组件卸载时 flush 未落库修改——真机 WebView 上 timer 随页面销毁/后台可能被丢弃）；
 * 桌面端原有的单 timer 防抖存在「两个文本字段快速连续编辑时前一字段保存被取消」的
 * 竞态，统一后按字段级 key 独立计时。
 *
 * 与 UI 库零耦合：平台 hook（desktop/mobile `useSettingsConfig`）经组合使用本引擎，
 * 平台专属字段（桌面 TUN 授权态 / 移动 VPN 通知偏好）留在平台层。
 */

import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toErrorMessage } from "../api";
import type { ClientConfig } from "../api";
import { CONFIG_KEY } from "../api/keys";
import { markRestartRequired } from "../pendingRestart";
import type { RestartDirtyKey } from "../pendingRestart";
import { toastError, toastSuccess, toastWarning } from "../toast";
import { useProxyStatus } from "./useProxyStatus";
import { useSaveConfig } from "./useClientConfig";

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
export function portError(raw: string): string | null {
  if (raw.trim() === "") {
    return "请输入端口号";
  }
  return isValidPort(raw) ? null : PORT_RANGE_ERROR;
}

/** 某字段待落库的防抖保存：timer 句柄 + 执行时刻最新草稿的补丁工厂 + 需重启标记。 */
interface PendingSave {
  timer: ReturnType<typeof setTimeout>;
  makePatch: () => Partial<ClientConfig>;
  restartKeys: RestartDirtyKey[];
}

export interface SettingsPersistEngine {
  /**
   * 立即保存补丁（串行链执行，返回是否成功）。每次执行都先等前一次保存完成
   * （useSaveConfig 已用其入参回写缓存）再读基底，后一次保存永远叠加在前一次
   * 结果之上，不会用旧基底整对象覆盖新修改。
   */
  persist: (patch: Partial<ClientConfig>, restartKeys?: RestartDirtyKey | RestartDirtyKey[]) => Promise<boolean>;
  /** 500ms 字段级防抖保存；patch 以工厂形式读取执行时刻的最新草稿。 */
  schedulePersist: (
    key: string,
    makePatch: () => Partial<ClientConfig>,
    restartKeys?: RestartDirtyKey | RestartDirtyKey[],
  ) => void;
  /** 取消某字段尚未落库的防抖保存（如端口变非法时丢弃旧值）。 */
  cancelPersist: (key: string) => void;
  /** 是否有保存进行中（按钮态 / 输入禁用展示）。 */
  saving: boolean;
}

function toKeyArray(keys: RestartDirtyKey | RestartDirtyKey[] | undefined): RestartDirtyKey[] {
  if (!keys) {
    return [];
  }
  return Array.isArray(keys) ? keys : [keys];
}

/**
 * 设置保存引擎：query 缓存（CONFIG_KEY）为唯一权威源；成功 toast + 失效重读 +
 * 重启脏标记上报；失败 toast + 失效回滚；组件卸载时 flush 全部未落库防抖修改
 * （合并为单次保存，单一 toast）。
 */
export function useSettingsPersist(): SettingsPersistEngine {
  const queryClient = useQueryClient();
  const saveConfigMutation = useSaveConfig();
  const { data: proxyStatus } = useProxyStatus();
  const coreRunning = proxyStatus?.core_running ?? false;
  // ref 镜像：供卸载 flush 读取最新值（避免闭包旧值）。
  const coreRunningRef = useRef(coreRunning);
  useEffect(() => {
    coreRunningRef.current = coreRunning;
  });

  // 各防抖字段独立计时（字段级 key），避免互相清掉对方待落库的保存；
  // 每条 pending 记录同时持有补丁工厂，供卸载 flush 读取最新草稿。
  const pendingRef = useRef<Map<string, PendingSave>>(new Map());
  /** persist 串行链：前一次落库并回写缓存后再读基底，避免并发保存互相覆盖。 */
  const persistChainRef = useRef<Promise<void>>(Promise.resolve());

  const persist = (
    patch: Partial<ClientConfig>,
    restartKeys?: RestartDirtyKey | RestartDirtyKey[],
  ): Promise<boolean> => {
    const keys = toKeyArray(restartKeys);
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
        await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
        for (const key of keys) {
          markRestartRequired(key, coreRunning);
        }
        return true;
      } catch (err) {
        toastError(toErrorMessage(err));
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

  // persist 每次渲染重建；commit 后经 ref 暴露最新实例（供防抖回调与卸载 flush
  // 使用；ref 只允许在 effect/handler 内读写）。
  const persistRef = useRef(persist);
  useEffect(() => {
    persistRef.current = persist;
  });

  const schedulePersist: SettingsPersistEngine["schedulePersist"] = (key, makePatch, restartKeys) => {
    const pending = pendingRef.current.get(key);
    if (pending) {
      clearTimeout(pending.timer);
    }
    pendingRef.current.set(key, {
      makePatch,
      restartKeys: toKeyArray(restartKeys),
      timer: setTimeout(() => {
        pendingRef.current.delete(key);
        void persistRef.current(makePatch(), restartKeys);
      }, 500),
    });
  };

  const cancelPersist: SettingsPersistEngine["cancelPersist"] = (key) => {
    const pending = pendingRef.current.get(key);
    if (pending) {
      clearTimeout(pending.timer);
      pendingRef.current.delete(key);
    }
  };

  // 卸载时 flush 未落库的防抖修改：路由切页/组件销毁不再依赖悬空 timer 存活，
  // 而是立即合并为单次保存（字段补丁互不重叠，Object.assign 合并安全）。
  useEffect(() => {
    return () => {
      const pending = pendingRef.current;
      if (pending.size === 0) {
        return;
      }
      pendingRef.current = new Map();
      const patches: Partial<ClientConfig>[] = [];
      const restartKeys = new Set<RestartDirtyKey>();
      pending.forEach(({ timer, makePatch, restartKeys: keys }) => {
        clearTimeout(timer);
        patches.push(makePatch());
        keys.forEach((k) => restartKeys.add(k));
      });
      const merged = Object.assign({}, ...patches) as Partial<ClientConfig>;
      void persistRef.current(merged).then((ok) => {
        if (ok) {
          restartKeys.forEach((key) => markRestartRequired(key, coreRunningRef.current));
        }
      });
    };
  }, []);

  return {
    persist,
    schedulePersist,
    cancelPersist,
    saving: saveConfigMutation.isPending,
  };
}
