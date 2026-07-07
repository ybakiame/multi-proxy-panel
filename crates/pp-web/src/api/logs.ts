import { get, getPaginated } from "./client";
import type { Log, PaginatedResponse } from "./types";

export const getLogs = (page: number, perPage: number, level?: string, source?: string) => {
  const params = new URLSearchParams();
  params.append("page", page.toString());
  params.append("per_page", perPage.toString());
  if (level && level !== "all") params.append("level", level);
  if (source) params.append("source", source);
  return getPaginated<Log>(`/api/v1/logs?${params.toString()}`);
};
