import { get, getPaginated, post, put, del } from "./client";
import type { Node, PaginatedResponse } from "./types";

export interface CreateNodePayload {
  name: string;
  hostname?: string;
  address?: string;
  usage_coefficient?: number;
  labels?: Record<string, string>;
  group_ids?: string[];
  parent_id?: string | null;
}

export const getNodes = () => get<Node[]>("/api/v1/nodes");
export const getNodesPaginated = (page: number, perPage: number) =>
  getPaginated<Node>(`/api/v1/nodes?page=${page}&per_page=${perPage}`);
export const createNode = (payload: CreateNodePayload) => post<Node>("/api/v1/nodes", payload);
export const updateNode = (id: string, payload: Partial<CreateNodePayload>) =>
  put<Node>(`/api/v1/nodes/${id}`, payload);
export const deleteNode = (id: string) => del(`/api/v1/nodes/${id}`);
export const pushConfig = (id: string, config: Record<string, unknown>) =>
  post(`/api/v1/nodes/${id}/push`, config);
