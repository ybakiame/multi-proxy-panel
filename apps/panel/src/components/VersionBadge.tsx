import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { getHubHealth } from "../api/health";
import { cn } from "@/lib/utils";
import pkg from "../../package.json";

export const UI_VERSION: string = pkg.version;

const HEALTH_REFETCH_INTERVAL = 60_000;

export function useHubVersion() {
  return useQuery({
    queryKey: ["hub-health"],
    queryFn: getHubHealth,
    staleTime: 30_000,
    refetchInterval: HEALTH_REFETCH_INTERVAL,
    retry: 1,
  });
}

interface VersionBadgeProps {
  className?: string;
  /** 是否在文字前展示 Hub 健康状态点 */
  withStatus?: boolean;
}

export function VersionBadge({ className, withStatus = false }: VersionBadgeProps) {
  const { t } = useTranslation();
  const { data, isLoading, isError } = useHubVersion();

  const statusDot = !withStatus ? null : isError ? (
    <span className="size-1.5 rounded-full bg-muted-foreground" aria-hidden="true" />
  ) : data?.status === "degraded" ? (
    <span className="size-1.5 rounded-full bg-warning" aria-hidden="true" />
  ) : data ? (
    <span className="size-1.5 rounded-full bg-success" aria-hidden="true" />
  ) : (
    <span className="size-1.5 animate-pulse rounded-full bg-muted-foreground" aria-hidden="true" />
  );

  const text = isError
    ? t("version.badgeUnavailable", { ui: UI_VERSION })
    : t("version.badge", { hub: isLoading ? "…" : data?.version, ui: UI_VERSION });

  return (
    <span
      className={cn(
        "inline-flex min-w-0 items-center gap-1.5 font-mono text-xs text-muted-foreground",
        className,
      )}
      title={isError ? t("version.hubUnreachable") : text}
    >
      {statusDot}
      <span className={cn("truncate", isLoading && "animate-pulse")}>{text}</span>
    </span>
  );
}
