import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import type { AxiosError } from "axios";
import { ArrowPathIcon, ArrowTopRightOnSquareIcon } from "@heroicons/react/24/outline";
import { Button } from "../components/ui/button";
import { Badge } from "../components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { Spinner } from "../components/ui/spinner";
import { PageHeader, ConfirmDialog } from "../components/ui";
import {
  getSystemUpdate,
  getSystemUpdateStatus,
  startSystemUpdate,
  SystemUpdateStatusKind,
} from "../api/system";
import { parseError } from "../api/client";
import { formatDateTime, formatDurationSince } from "../utils/format";

const systemUpdateQueryKey = "system-update";
const systemUpdateStatusQueryKey = "system-update-status";

const TERMINAL_STATUSES: SystemUpdateStatusKind[] = ["succeeded", "failed", "rolled_back"];

function InfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5 sm:flex-row sm:items-baseline sm:justify-between sm:gap-4">
      <span className="shrink-0 text-sm text-muted-foreground">{label}</span>
      <div className="min-w-0 break-all text-sm sm:text-right">{children}</div>
    </div>
  );
}

export function System() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const copyTimer = useRef<number | null>(null);
  const lastObservedStatus = useRef<string | null>(null);

  const updateQuery = useQuery({
    queryKey: [systemUpdateQueryKey],
    queryFn: getSystemUpdate,
  });
  const info = updateQuery.data;

  const statusQuery = useQuery({
    queryKey: [systemUpdateStatusQueryKey],
    queryFn: getSystemUpdateStatus,
    refetchInterval: (query) => (query.state.data?.status === "running" ? 2000 : false),
  });

  const status = statusQuery.data ?? info?.status ?? null;
  const statusKind = status?.status ?? "idle";
  const running = statusKind === "running";
  const stale = statusKind === "stale";

  // Refresh the update check once a run reaches a terminal state.
  useEffect(() => {
    if (!status || status.status === lastObservedStatus.current) return;
    const previous = lastObservedStatus.current;
    lastObservedStatus.current = status.status;
    if (previous !== null && TERMINAL_STATUSES.includes(status.status)) {
      void queryClient.invalidateQueries({ queryKey: [systemUpdateQueryKey] });
    }
  }, [status, queryClient]);

  const startMutation = useMutation({
    mutationFn: () => startSystemUpdate(),
    onSuccess: () => {
      setConfirmOpen(false);
      void queryClient.invalidateQueries({ queryKey: [systemUpdateStatusQueryKey] });
    },
  });

  const handleCheck = () => {
    void updateQuery.refetch();
  };

  const handleCopyCommand = async () => {
    if (!info) return;
    try {
      await navigator.clipboard.writeText(info.manual_command);
      setCopyState("copied");
    } catch {
      setCopyState("failed");
    }
    if (copyTimer.current) window.clearTimeout(copyTimer.current);
    copyTimer.current = window.setTimeout(() => setCopyState("idle"), 3000);
  };

  const statusBadge = (kind: SystemUpdateStatusKind) => {
    let variant: "secondary" | "destructive" = "secondary";
    let className = "";
    if (kind === "succeeded") className = "bg-success/10 text-success";
    else if (kind === "failed" || kind === "stale") variant = "destructive";
    else if (kind === "running" || kind === "rolled_back") className = "bg-warning/10 text-warning";
    return (
      <Badge variant={variant} className={className}>
        {t(`system.status.${kind}`)}
      </Badge>
    );
  };

  const terminalAlert = (kind: SystemUpdateStatusKind) => {
    const alerts: Partial<Record<SystemUpdateStatusKind, React.ReactNode>> = {
      succeeded: (
        <div className="rounded-lg border border-success/30 bg-success/10 p-4 text-success">
          {t("system.succeededAlert")}
        </div>
      ),
      failed: (
        <div role="alert" className="rounded-lg border border-destructive p-4 text-destructive">
          {t("system.failedAlert")}
        </div>
      ),
      rolled_back: (
        <div className="rounded-lg border border-warning/30 bg-warning/10 p-4 text-warning">
          {t("system.rolledBackAlert")}
        </div>
      ),
      stale: (
        <div role="alert" className="rounded-lg border border-destructive p-4 text-destructive">
          {t("system.staleAlert")}
        </div>
      ),
    };
    return alerts[kind] ?? null;
  };

  const startDisabledReason = !info?.execution_enabled
    ? null
    : stale
      ? t("system.staleHint")
      : running
        ? t("system.runningHint")
        : !info.update_available
          ? t("system.noUpdateHint")
          : null;
  const canStart =
    !!info?.execution_enabled &&
    !!info.update_available &&
    !running &&
    !stale &&
    !startMutation.isPending;

  return (
    <div className="space-y-4">
      <PageHeader title={t("system.title")} />
      <p className="-mt-2 text-sm text-muted-foreground">{t("system.description")}</p>

      {updateQuery.isError && (
        <div role="alert" className="rounded-lg border border-destructive p-4 text-destructive">
          {t("system.loadError")}
          <Button
            variant="ghost"
            size="sm"
            className="ml-2"
            disabled={updateQuery.isFetching}
            onClick={handleCheck}
          >
            {t("common.refresh")}
          </Button>
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle>{t("system.versionCardTitle")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {updateQuery.isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : info ? (
            <>
              <div className="space-y-2">
                <InfoRow label={t("system.currentVersion")}>
                  <span className="font-mono">{info.current_version}</span>
                </InfoRow>
                <InfoRow label={t("system.latestVersion")}>
                  <span className="font-mono">{info.latest_version}</span>
                </InfoRow>
                <InfoRow label={t("system.updateState")}>
                  {info.update_available ? (
                    <Badge className="bg-warning/10 text-warning">
                      {t("system.updateAvailable")}
                    </Badge>
                  ) : (
                    <Badge variant="secondary" className="bg-success/10 text-success">
                      {t("system.upToDate")}
                    </Badge>
                  )}
                </InfoRow>
                <InfoRow label={t("system.publishedAt")}>
                  {info.published_at ? formatDateTime(info.published_at) : "-"}
                </InfoRow>
                <InfoRow label={t("system.checkedAt")}>{formatDateTime(info.checked_at)}</InfoRow>
                <InfoRow label={t("system.releaseUrl")}>
                  <a
                    href={info.release_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-1 text-primary underline-offset-2 hover:underline"
                  >
                    <span className="break-all">{info.release_url}</span>
                    <ArrowTopRightOnSquareIcon className="h-3.5 w-3.5 shrink-0" />
                  </a>
                </InfoRow>
              </div>
              <div className="flex justify-end border-t pt-4">
                <Button variant="secondary" disabled={updateQuery.isFetching} onClick={handleCheck}>
                  {updateQuery.isFetching ? (
                    <Spinner aria-label={t("system.checking")} />
                  ) : (
                    <ArrowPathIcon className="h-4 w-4" />
                  )}
                  {updateQuery.isFetching ? t("system.checking") : t("system.checkUpdates")}
                </Button>
              </div>
            </>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("system.executionTitle")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {startMutation.isError && (
            <div role="alert" className="rounded-lg border border-destructive p-4 text-destructive">
              {t("system.startError")}: {parseError(startMutation.error as AxiosError).message}
            </div>
          )}
          {info && !info.execution_enabled ? (
            <div className="space-y-3">
              <p className="text-sm text-muted-foreground">{t("system.executionDisabledHint")}</p>
              <div className="flex flex-col gap-2 sm:flex-row">
                <Input readOnly value={info.manual_command} className="flex-1 font-mono" />
                <Button
                  variant="secondary"
                  className="shrink-0"
                  disabled={copyState === "copied"}
                  onClick={handleCopyCommand}
                >
                  {copyState === "copied" ? t("common.copied") : t("system.copyCommand")}
                </Button>
              </div>
              {copyState === "failed" && (
                <p role="alert" className="text-sm text-destructive">
                  {t("system.copyFailed")}
                </p>
              )}
            </div>
          ) : (
            <>
              <p className="text-sm text-muted-foreground">{t("system.executionEnabledHint")}</p>
              {startDisabledReason && (
                <p className="text-sm text-muted-foreground">{startDisabledReason}</p>
              )}
              <div className="flex justify-end border-t pt-4">
                <Button disabled={!canStart} onClick={() => setConfirmOpen(true)}>
                  {startMutation.isPending && <Spinner aria-label={t("common.loading")} />}
                  {t("system.startUpdate")}
                </Button>
              </div>
            </>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>{t("system.statusTitle")}</CardTitle>
          {statusBadge(statusKind)}
        </CardHeader>
        <CardContent className="space-y-4">
          {statusKind === "idle" ? (
            <p className="text-sm text-muted-foreground">{t("system.idleAlert")}</p>
          ) : (
            status && (
              <>
                <div className="space-y-2">
                  <InfoRow label={t("system.stage")}>
                    <span className="font-mono">{status.stage || "-"}</span>
                  </InfoRow>
                  <InfoRow label={t("system.component")}>
                    <span className="font-mono">{status.component || "-"}</span>
                  </InfoRow>
                  <InfoRow label={t("system.currentVersion")}>
                    <span className="font-mono">{status.current_version ?? "-"}</span>
                  </InfoRow>
                  <InfoRow label={t("system.targetVersion")}>
                    <span className="font-mono">{status.target_version ?? "-"}</span>
                  </InfoRow>
                  <InfoRow label={t("system.startedAt")}>
                    {status.started_at ? formatDateTime(status.started_at) : "-"}
                  </InfoRow>
                  {running && status.started_at && (
                    <InfoRow label={t("system.elapsed")}>
                      {formatDurationSince(status.started_at)}
                    </InfoRow>
                  )}
                  {!running && (
                    <InfoRow label={t("system.finishedAt")}>
                      {status.finished_at ? formatDateTime(status.finished_at) : "-"}
                    </InfoRow>
                  )}
                  {status.message && (
                    <InfoRow label={t("system.message")}>
                      <span className="break-all">{status.message}</span>
                    </InfoRow>
                  )}
                </div>
                {terminalAlert(statusKind)}
              </>
            )
          )}
        </CardContent>
      </Card>

      <ConfirmDialog
        title={t("system.startConfirmTitle")}
        isOpen={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        onConfirm={() => startMutation.mutate()}
        confirmText={t("system.startUpdate")}
        isLoading={startMutation.isPending}
      >
        {t("system.startConfirmBody", {
          current: info?.current_version ?? "-",
          target: info?.latest_version ?? "-",
        })}
      </ConfirmDialog>
    </div>
  );
}
