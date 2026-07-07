import { get, getPaginated, post, put, del } from "./client";
import type { InboundHost, PaginatedResponse } from "./types";

export interface CreateHostPayload {
  protocol_config_id: string;
  node_id: string;
  remark: string;
  address: string;
  port: number;
  sni?: string;
  host?: string;
  path?: string;
  security?: string;
  alpn?: string;
  fingerprint?: string;
  is_active?: boolean;
}

export const getHosts = (page: number, perPage: number) =>
  getPaginated<InboundHost>(`/api/v1/hosts?page=${page}&per_page=${perPage}`);
export const createHost = (payload: CreateHostPayload) =>
  post<InboundHost>("/api/v1/hosts", payload);
export const updateHost = (id: string, payload: Partial<CreateHostPayload>) =>
  put<InboundHost>(`/api/v1/hosts/${id}`, payload);
export const deleteHost = (id: string) => del(`/api/v1/hosts/${id}`);
