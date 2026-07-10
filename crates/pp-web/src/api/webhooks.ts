import { getPaginated, post, del } from "./client";
import type { Webhook } from "./types";

export interface CreateWebhookPayload {
  name: string;
  url: string;
  events?: string[];
  secret?: string;
  is_active?: boolean;
}

export const getWebhooks = (page: number, perPage: number) =>
  getPaginated<Webhook>(`/api/v1/webhooks?page=${page}&per_page=${perPage}`);
export const createWebhook = (payload: CreateWebhookPayload) =>
  post<Webhook>("/api/v1/webhooks", payload);
export const deleteWebhook = (id: string) => del(`/api/v1/webhooks/${id}`);
