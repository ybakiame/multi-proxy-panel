import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { useSetAtom } from "jotai";
import { proxyStatus, toErrorMessage } from "../api";
import type { ClientStatus } from "../api";
import { PROXY_STATUS_KEY } from "../api/keys";
import { lastActionErrorAtom } from "../atoms/ui";

/**
 * 代理运行状态（Tauri `proxy_status`），2s 轮询，Query 缓存为唯一权威源。
 *
 * 与原 store.refreshStatus 的 error 语义一致：轮询失败记录到 lastActionErrorAtom，
 * 轮询成功不清 error（避免吞掉 start/stop/saveConfig 等操作刚记录的错误，
 * 错误由后续成功的 start/stop/saveConfig/loadConfig 清除）。
 */
export function useProxyStatus() {
  const setLastError = useSetAtom(lastActionErrorAtom);
  const query = useQuery<ClientStatus>({
    queryKey: PROXY_STATUS_KEY,
    queryFn: proxyStatus,
    refetchInterval: 2000,
    retry: false,
  });

  useEffect(() => {
    if (query.error) {
      setLastError(toErrorMessage(query.error));
    }
  }, [query.error, setLastError]);

  return query;
}
