import { useQuery } from "@tanstack/react-query";
import { getCapabilities } from "../api";
import type { Capabilities } from "../api";

const CAPABILITIES_KEY = ["capabilities"];

/**
 * Query capabilities once at app bootstrap and cache indefinitely.
 *
 * Replaces scattered `os === "android"` checks with a single
 * `capabilities.is_android` or `capabilities.mitm` etc.
 *
 * The capabilities matrix is static for the lifetime of the app
 * (determined by the compiled platform), so we set staleTime to Infinity.
 */
export function useCapabilities() {
  return useQuery<Capabilities>({
    queryKey: CAPABILITIES_KEY,
    queryFn: getCapabilities,
    staleTime: Infinity,
    retry: false,
  });
}
