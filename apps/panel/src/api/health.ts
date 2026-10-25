import { get } from "./client";

export interface HubHealth {
  status: "healthy" | "degraded" | string;
  database: "connected" | "disconnected" | string;
  version: string;
}

export const getHubHealth = () => get<HubHealth>("/health");
