import { get, getPaginated, post, put, del } from "./client";
import type { ProtocolConfig } from "./types";

export interface CreateProtocolPayload {
  name: string;
  protocol_type: string;
  core_type: string;
  core_version?: string;
  listen_address: string;
  listen_port: number;
  settings: Record<string, unknown>;
  tls_settings?: Record<string, unknown>;
}

export const getProtocols = (page: number, perPage: number) =>
  getPaginated<ProtocolConfig>(`/api/v1/protocols?page=${page}&per_page=${perPage}`);
export const getAllProtocols = () => get<ProtocolConfig[]>("/api/v1/protocols");
export const createProtocol = (payload: CreateProtocolPayload) =>
  post<ProtocolConfig>("/api/v1/protocols", payload);
export const updateProtocol = (id: string, payload: Partial<CreateProtocolPayload>) =>
  put<ProtocolConfig>(`/api/v1/protocols/${id}`, payload);
export const deleteProtocol = (id: string) => del(`/api/v1/protocols/${id}`);
export const generateRealityKeys = () =>
  get<Record<string, string>>("/api/v1/utils/generate-reality-keys");
