import { get, post } from "./client";

export type SystemUpdateStatusKind =
  | "idle"
  | "running"
  | "succeeded"
  | "failed"
  | "rolled_back"
  | "stale";

export interface SystemMaintenanceStatus {
  status: SystemUpdateStatusKind;
  stage: string;
  component: string;
  current_version: string | null;
  target_version: string | null;
  message: string;
  started_at: string | null;
  finished_at: string | null;
}

export interface SystemUpdateInfo {
  current_version: string;
  latest_version: string;
  update_available: boolean;
  release_url: string;
  published_at: string | null;
  checked_at: string;
  execution_enabled: boolean;
  manual_command: string;
  status: SystemMaintenanceStatus;
}

export const getSystemUpdate = () => get<SystemUpdateInfo>("/api/v1/system/update");

export const getSystemUpdateStatus = () =>
  get<SystemMaintenanceStatus>("/api/v1/system/update/status");

export const startSystemUpdate = (targetVersion?: string) =>
  post<{ status: SystemMaintenanceStatus; message: string }>("/api/v1/system/update", {
    target_version: targetVersion,
  });
