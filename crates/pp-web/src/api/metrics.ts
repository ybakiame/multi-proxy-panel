import { get, getPaginated } from "./client";
import type { Metric, PaginatedResponse } from "./types";

export const getMetrics = (nodeId?: string) =>
  getPaginated<Metric>(`/api/v1/metrics${nodeId ? `?node_id=${nodeId}` : ""}`);
export const getLatestMetrics = (nodeId: string) =>
  get<Metric>(`/api/v1/metrics/${nodeId}/latest`);
