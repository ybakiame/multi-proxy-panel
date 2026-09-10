import { useQuery } from "@tanstack/react-query";
import { coreVersion } from "../api";
import { CORE_VERSION_KEY } from "../api/keys";

/**
 * Query the sing-box version compiled into the Android `panelcore.aar`.
 *
 * The version is injected at build time (`Libbox.version()`), so it is static for
 * the lifetime of the installed app: cache indefinitely, no retry. Only the
 * mobile About page consumes it; desktop shells do not register the command.
 */
export function useCoreVersion() {
  return useQuery<string>({
    queryKey: CORE_VERSION_KEY,
    queryFn: coreVersion,
    staleTime: Infinity,
    retry: false,
  });
}
