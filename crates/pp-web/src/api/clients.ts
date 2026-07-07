import { get, getPaginated, post, put, del } from "./client";
import type { Client, PaginatedResponse } from "./types";

export interface CreateClientPayload {
  name: string;
  user_id?: string;
  email?: string;
  traffic_limit_bytes?: number;
  expiry_date?: string;
  reset_day?: number;
  data_limit_reset_strategy?: string;
  max_devices?: number;
  group_ids?: string[];
  status?: string;
  on_hold_expire_duration_secs?: number;
  on_hold_timeout?: string;
}

export const getClients = (page: number, perPage: number) =>
  getPaginated<Client>(`/api/v1/clients?page=${page}&per_page=${perPage}`);
export const createClient = (payload: CreateClientPayload) =>
  post<Client>("/api/v1/clients", payload);
export const updateClient = (id: string, payload: Partial<CreateClientPayload>) =>
  put<Client>(`/api/v1/clients/${id}`, payload);
export const deleteClient = (id: string) => del(`/api/v1/clients/${id}`);
export const resetClientTraffic = (id: string) =>
  post<Client>(`/api/v1/clients/${id}/reset-traffic`, {});
