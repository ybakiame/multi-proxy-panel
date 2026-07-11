import { get, getPaginated, post, put, del } from "./client";
import type { Subscription, SubscriptionTemplate } from "./types";

export interface CreateSubscriptionPayload {
  client_id: string;
}

export interface CreateTemplatePayload {
  name: string;
  format?: string;
  base_config?: string;
  filter_rules?: Record<string, unknown>;
  custom_headers?: Record<string, string>;
}

export interface UpdateTemplatePayload {
  name?: string;
  format?: string;
  base_config?: string;
  filter_rules?: Record<string, unknown>;
  custom_headers?: Record<string, string>;
}

export const getSubscriptions = (page: number, perPage: number) =>
  getPaginated<Subscription>(`/api/v1/subscriptions?page=${page}&per_page=${perPage}`);
export const createSubscription = (payload: CreateSubscriptionPayload) =>
  post<Subscription>("/api/v1/subscriptions", payload);
export const updateSubscription = (
  id: string,
  payload: { is_active?: boolean; expire_at?: string },
) => put<Subscription>(`/api/v1/subscriptions/${id}`, payload);
export const deleteSubscription = (id: string) => del(`/api/v1/subscriptions/${id}`);
export const getTemplates = () => get<SubscriptionTemplate[]>("/api/v1/templates");
export const createTemplate = (payload: CreateTemplatePayload) =>
  post<SubscriptionTemplate>("/api/v1/templates", payload);
export const updateTemplate = (id: string, payload: UpdateTemplatePayload) =>
  put<SubscriptionTemplate>(`/api/v1/templates/${id}`, payload);
export const deleteTemplate = (id: string) => del(`/api/v1/templates/${id}`);
