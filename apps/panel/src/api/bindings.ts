import { get, getPaginated, post, put, del } from "./client";
import type { Binding } from "./types";

export interface CreateBindingPayload {
  node_id: string;
  protocol_config_id: string;
  is_active?: boolean;
  override_settings?: Record<string, unknown>;
}

export interface UpdateBindingPayload {
  is_active?: boolean;
  override_settings?: Record<string, unknown>;
}

export const getBindings = () => get<Binding[]>("/api/v1/bindings");
export const getBindingsPaginated = (page: number, perPage: number) =>
  getPaginated<Binding>(`/api/v1/bindings?page=${page}&per_page=${perPage}`);
export const createBinding = (payload: CreateBindingPayload) =>
  post<Binding>("/api/v1/bindings", payload);
export const updateBinding = (id: string, payload: UpdateBindingPayload) =>
  put<Binding>(`/api/v1/bindings/${id}`, payload);
export const deleteBinding = (id: string) => del(`/api/v1/bindings/${id}`);
