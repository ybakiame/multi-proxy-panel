import { get, getPaginated, post, put, del } from "./client";
import type { Group } from "./types";

export interface CreateGroupPayload {
  name: string;
  description?: string;
  labels?: Record<string, string>;
}

export const getGroups = () => get<Group[]>("/api/v1/groups");
export const getGroupsPaginated = (page: number, perPage: number) =>
  getPaginated<Group>(`/api/v1/groups?page=${page}&per_page=${perPage}`);
export const createGroup = (payload: CreateGroupPayload) => post<Group>("/api/v1/groups", payload);
export const updateGroup = (id: string, payload: Partial<CreateGroupPayload>) =>
  put<Group>(`/api/v1/groups/${id}`, payload);
export const deleteGroup = (id: string) => del(`/api/v1/groups/${id}`);
