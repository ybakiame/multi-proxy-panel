import { get, getPaginated, post, put, del } from "./client";
import type { AgentLog, Node } from "./types";

export interface PendingUpdate {
  node_id: string;
  node_name: string;
  core_type: string;
  update_type: string;
  updated_at: string;
}

export interface PushPendingResult {
  node_id: string;
  core_type: string;
  ok: boolean;
  error?: string;
}

export interface PushPendingResults {
  results: PushPendingResult[];
  total: number;
  succeeded: number;
  failed: number;
}

export interface CreateNodePayload {
  name: string;
  domain?: string;
  usage_coefficient?: number;
  labels?: Record<string, string>;
  parent_id?: string | null;
}

export interface UpdateNodePayload {
  name?: string;
  domain?: string;
  usage_coefficient?: number;
  labels?: Record<string, string>;
  parent_id?: string | null;
}

export const getNodes = () => get<Node[]>("/api/v1/nodes");
export const getNode = (id: string) => get<Node>(`/api/v1/nodes/${id}`);
export const getNodesPaginated = (page: number, perPage: number) =>
  getPaginated<Node>(`/api/v1/nodes?page=${page}&per_page=${perPage}`);
export const createNode = (payload: CreateNodePayload) => post<Node>("/api/v1/nodes", payload);
export const updateNode = (id: string, payload: UpdateNodePayload) =>
  put<Node>(`/api/v1/nodes/${id}`, payload);
export const deleteNode = (id: string) => del(`/api/v1/nodes/${id}`);
export const pushConfig = (id: string, config: Record<string, unknown>) =>
  post(`/api/v1/nodes/${id}/push`, config);
export const getNodeLogs = (id: string, limit = 100) =>
  get<AgentLog[]>(`/api/v1/nodes/${id}/logs?limit=${limit}`);

export interface CoreBinary {
  file_name: string;
  size_bytes: number;
  modified_at: number;
  in_use: boolean;
}

export const getCoreBinaries = (id: string) =>
  get<{ binaries: CoreBinary[]; error: string }>(`/api/v1/nodes/${id}/binaries`).then(
    (res) => res.binaries,
  );

export const deleteCoreBinary = (id: string, fileName: string) =>
  del(`/api/v1/nodes/${id}/binaries/${encodeURIComponent(fileName)}`);

export const getPendingUpdates = () =>
  get<{ pending: PendingUpdate[] }>("/api/v1/nodes/pending-updates").then((r) => r.pending);

export interface InstallCommand {
  id: string;
  name: string;
  token: string;
  hub_url: string;
  script_url: string;
  version: string;
  command: string;
  was_connected: boolean;
}

export const getInstallCommand = (id: string) =>
  post<InstallCommand>(`/api/v1/nodes/${id}/install-command`, {});

export const pushPendingUpdates = (payload: { node_ids?: string[]; core_type?: string }) =>
  post<PushPendingResults>("/api/v1/nodes/push-pending", payload);
