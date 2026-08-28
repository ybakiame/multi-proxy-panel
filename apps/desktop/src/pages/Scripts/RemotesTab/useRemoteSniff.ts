/**
 * Hook for sniffing a remote resource URL.
 * Wraps `detectRemote` and normalizes the response into form-ready state.
 */

import { useCallback, useRef, useState } from "react";
import { detectRemote, toErrorMessage } from "../../../api";
import type { DetectRemoteView } from "../../../api";
import { deriveNameFromUrl, normalizeDialect, normalizeKind } from "../utils";
import type { SniffResult } from "./types";

export interface UseRemoteSniffReturn {
  /** Whether a sniff request is in flight. */
  detecting: boolean;
  /** Human-readable sniff status message (null when idle). */
  detectInfo: string | null;
  /** Perform sniff for the given URL. */
  sniff: (url: string, setError: (msg: string | null) => void) => Promise<SniffResult | null>;
  /** Reset internal detect state. */
  reset: () => void;
}

export function useRemoteSniff(): UseRemoteSniffReturn {
  const [detecting, setDetecting] = useState(false);
  const [detectInfo, setDetectInfo] = useState<string | null>(null);
  const lastDetectedUrlRef = useRef("");

  const sniff = useCallback(
    async (url: string, setError: (msg: string | null) => void): Promise<SniffResult | null> => {
      const trimmed = url.trim();
      if (!trimmed || trimmed === lastDetectedUrlRef.current) return null;
      lastDetectedUrlRef.current = trimmed;
      setDetecting(true);
      setError(null);
      setDetectInfo(null);
      try {
        const result: DetectRemoteView = await detectRemote(trimmed);
        const kind = normalizeKind(result.kind);
        const dialect = normalizeDialect(result.dialect);

        const parts: string[] = [];
        if (kind) parts.push(kind === "Script" ? "脚本" : "片段");
        if (dialect) parts.push(dialect);
        if (result.meta?.arguments?.length) parts.push(`${result.meta.arguments.length} 个模块参数`);

        let info: string | null = null;
        if (result.meta?.name) {
          info = `已识别：${result.meta.name}${parts.length ? `（${parts.join(" / ")}）` : ""}`;
        } else if (parts.length > 0) {
          info = `已识别类型：${parts.join(" / ")}`;
        } else {
          info = "未识别出类型与元数据，可手动填写";
        }
        setDetectInfo(info);

        return {
          kind,
          dialect,
          name: result.meta?.name?.trim() || deriveNameFromUrl(trimmed),
          description: result.meta?.desc?.trim() ?? null,
          icon: result.meta?.icon ?? null,
          arguments: result.meta?.arguments ?? [],
          info,
        };
      } catch (err) {
        lastDetectedUrlRef.current = "";
        setDetectInfo(null);
        setError(toErrorMessage(err));
        return null;
      } finally {
        setDetecting(false);
      }
    },
    [],
  );

  const reset = useCallback(() => {
    setDetectInfo(null);
    lastDetectedUrlRef.current = "";
  }, []);

  return { detecting, detectInfo, sniff, reset };
}
