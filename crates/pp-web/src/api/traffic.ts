import { getPaginated } from "./client";
import type { TrafficRecord } from "./types";

export const getTraffic = (nodeId?: string, clientId?: string) => {
  const params = new URLSearchParams();
  if (nodeId) params.append("node_id", nodeId);
  if (clientId) params.append("client_id", clientId);
  return getPaginated<TrafficRecord>(`/api/v1/traffic?${params.toString()}`);
};
