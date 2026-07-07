import { get, getPaginated, post, del } from "./client";
import type { Binding, PaginatedResponse } from "./types";

export interface CreateBindingPayload {
  node_id: string;
  protocol_config_id: string;
  is_active?: boolean;
  override_settings?: Record<string, unknown>;
}

export const getBindings = () => get<Binding[]>("/api/v1/bindings");
export const getBindingsPaginated = (page: number, perPage: number) =>
  getPaginated<Binding>(`/api/v1/bindings?page=${page}&per_page=${perPage}`);
export const createBinding = (payload: CreateBindingPayload) =>
  post<Binding>("/api/v1/bindings", payload);
export const deleteBinding = (id: string) => del(`/api/v1/bindings/${id}`);
