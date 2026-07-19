import { get, getPaginated } from "./client";
import type { UsageRecord, UsageSummaryItem } from "./types";

export interface UsageFilters {
  nodeId?: string;
  clientId?: string;
  start?: string;
  end?: string;
  limit?: number;
}

function buildParams(filters: UsageFilters): URLSearchParams {
  const params = new URLSearchParams();
  if (filters.nodeId) params.append("node_id", filters.nodeId);
  if (filters.clientId) params.append("client_id", filters.clientId);
  if (filters.start) params.append("start", filters.start);
  if (filters.end) params.append("end", filters.end);
  if (filters.limit) params.append("limit", filters.limit.toString());
  return params;
}

export const getUsage = (filters: UsageFilters = {}) =>
  getPaginated<UsageRecord>(`/api/v1/usage?${buildParams(filters).toString()}`);

export const getUsageSummary = (
  groupBy: "client" | "node" = "client",
  filters: UsageFilters = {},
) => {
  const params = buildParams(filters);
  params.append("group_by", groupBy);
  return get<UsageSummaryItem[]>(`/api/v1/usage/summary?${params.toString()}`);
};
