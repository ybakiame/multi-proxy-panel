import { getPaginated, post, del } from "./client";
import type { ApiKey } from "./types";

export interface CreateApiKeyPayload {
  name: string;
  scopes?: string[];
  ip_allowlist?: string[];
  rate_limit?: number;
}

export const getApiKeys = (page: number, perPage: number) =>
  getPaginated<ApiKey>(`/api/v1/api-keys?page=${page}&per_page=${perPage}`);
export const createApiKey = (payload: CreateApiKeyPayload) =>
  post<ApiKey>("/api/v1/api-keys", payload);
export const deleteApiKey = (id: string) => del(`/api/v1/api-keys/${id}`);
