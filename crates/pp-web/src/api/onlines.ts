import { get, getPaginated } from "./client";
import type { OnlineSession, PaginatedResponse } from "./types";

export const getOnlineCount = () => get<{ count: number }>("/api/v1/onlines/count");
export const getOnlines = (nodeId?: string, clientId?: string) => {
  const params = new URLSearchParams();
  if (nodeId) params.append("node_id", nodeId);
  if (clientId) params.append("client_id", clientId);
  return getPaginated<OnlineSession>(`/api/v1/onlines?${params.toString()}`);
};
