import axios, { AxiosError, AxiosRequestConfig } from "axios";
import { getApiKey, clearApiKey, baseUrl } from "./config";
import { ApiResponse, ApiError, PaginatedResponse } from "./types";

const api = axios.create({
  baseURL: baseUrl(),
  headers: {
    "Content-Type": "application/json",
  },
});

api.interceptors.request.use((config) => {
  const key = getApiKey();
  if (key) {
    config.headers.Authorization = `Bearer ${key}`;
  }
  return config;
});

api.interceptors.response.use(
  (response) => response,
  (error: AxiosError) => {
    if (error.response?.status === 401) {
      clearApiKey();
      window.location.href = "/login";
    }
    return Promise.reject(error);
  },
);

export function parseError(error: AxiosError): ApiError {
  const status = error.response?.status || 0;
  const data = error.response?.data as { error?: { code?: string; message?: string } } | undefined;

  return {
    code: data?.error?.code || "unknown",
    status,
    message: data?.error?.message || error.message || "Unknown error",
  };
}

export async function get<T>(path: string, config?: AxiosRequestConfig): Promise<T> {
  const resp = await api.get<ApiResponse<T>>(path, config);
  return resp.data.data;
}

export async function getPaginated<T>(
  path: string,
  config?: AxiosRequestConfig,
): Promise<PaginatedResponse<T>> {
  const resp = await api.get<
    { data: T[]; meta?: { total?: number }; pagination?: { total?: number } } | T[]
  >(path, config);
  const payload = resp.data;

  if (Array.isArray(payload)) {
    return {
      data: payload,
      pagination: {
        page: 1,
        per_page: payload.length,
        total: payload.length,
        total_pages: 1,
      },
    };
  }

  const items = payload.data ?? [];
  const total = payload.meta?.total ?? payload.pagination?.total ?? items.length;
  const perPage = items.length || 1;

  return {
    data: items,
    pagination: {
      page: 1,
      per_page: items.length,
      total,
      total_pages: Math.max(1, Math.ceil(total / perPage)),
    },
  };
}

export async function post<T>(path: string, body: unknown): Promise<T> {
  const resp = await api.post<ApiResponse<T>>(path, body);
  return resp.data.data;
}

export async function put<T>(path: string, body: unknown): Promise<T> {
  const resp = await api.put<ApiResponse<T>>(path, body);
  return resp.data.data;
}

export async function del(path: string): Promise<void> {
  await api.delete(path);
}

export async function validateApiKey(key: string): Promise<void> {
  await axios.get(`${baseUrl()}/api/v1/nodes`, {
    headers: {
      Authorization: `Bearer ${key}`,
    },
  });
}

export default api;
