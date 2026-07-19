import { getPaginated } from "./client";
import type { TrafficRecord } from "./types";

export interface TrafficFilters {
  nodeId?: string;
  clientId?: string;
  start?: string;
  end?: string;
  limit?: number;
}

export const getTraffic = (nodeIdOrFilters?: string | TrafficFilters, clientId?: string) => {
  const filters: TrafficFilters =
    typeof nodeIdOrFilters === "string" || nodeIdOrFilters === undefined
      ? { nodeId: nodeIdOrFilters, clientId }
      : nodeIdOrFilters;
  const params = new URLSearchParams();
  if (filters.nodeId) params.append("node_id", filters.nodeId);
  if (filters.clientId) params.append("client_id", filters.clientId);
  if (filters.start) params.append("start", filters.start);
  if (filters.end) params.append("end", filters.end);
  if (filters.limit) params.append("limit", filters.limit.toString());
  return getPaginated<TrafficRecord>(`/api/v1/traffic?${params.toString()}`);
};
