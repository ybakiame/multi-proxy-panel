import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useSetAtom } from "jotai";
import { getConfig, saveConfig, toErrorMessage } from "../api";
import type { ClientConfig } from "../api";
import { CONFIG_KEY } from "../api/keys";
import { lastActionErrorAtom } from "../atoms/ui";

/**
 * 客户端配置（Tauri `get_config`），Query 缓存为唯一权威源。
 *
 * 读取失败记录到 lastActionErrorAtom（跨页共享错误），成功时清除——
 * 与原 store.loadConfig 的 error 语义一致。
 */
export function useClientConfig() {
  const setLastError = useSetAtom(lastActionErrorAtom);
  const query = useQuery<ClientConfig>({
    queryKey: CONFIG_KEY,
    queryFn: getConfig,
  });

  useEffect(() => {
    if (query.error) {
      setLastError(toErrorMessage(query.error));
    } else if (query.isSuccess) {
      setLastError(null);
    }
  }, [query.error, query.isSuccess, setLastError]);

  return query;
}

/** `saveConfig` 串行化链：前一次保存完成后再执行下一个，避免两次保存交错时旧基底覆盖新修改。 */
let saveChain: Promise<unknown> = Promise.resolve();

/**
 * 保存客户端配置的 mutation。
 *
 * 保留原 store.saveConfig 的语义：
 * - 模块级 Promise 链串行化：前一次保存完成后才执行下一个，链吞异常不中断后续排队；
 * - onSuccess 用保存入参直接 setQueryData 回写 CONFIG_KEY 缓存（不触发重读）；
 * - mutationFn 返回值携带后端 `SaveConfigView.warning`（非阻塞提示，无则为 null）。
 */
export function useSaveConfig() {
  const queryClient = useQueryClient();
  const setLastError = useSetAtom(lastActionErrorAtom);

  return useMutation({
    mutationFn: (cfg: ClientConfig) => {
      const run = saveChain.then(async () => {
        const view = await saveConfig(cfg);
        return { cfg, warning: view.warning ?? null };
      });
      // 链吞掉异常保证后续排队任务不被中断；返回值仍保留给调用方。
      saveChain = run.catch(() => undefined);
      return run;
    },
    onSuccess: ({ cfg }) => {
      queryClient.setQueryData(CONFIG_KEY, cfg);
      setLastError(null);
    },
    onError: (err: unknown) => {
      setLastError(toErrorMessage(err));
    },
  });
}
