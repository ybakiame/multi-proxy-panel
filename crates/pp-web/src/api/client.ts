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
  }
);

export function parseError(error: AxiosError): ApiError {
  const status = error.response?.status || 0;
  const data = error.response?.data as { type?: string; message?: string } | undefined;

  return {
    type: data?.type || "unknown",
    status,
    message: data?.message || error.message || "Unknown error",
  };
}

export async function get<T>(path: string, config?: AxiosRequestConfig): Promise<T> {
  const resp = await api.get<ApiResponse<T>>(path, config);
  return resp.data.data;
}

export async function getPaginated<T>(path: string, config?: AxiosRequestConfig): Promise<PaginatedResponse<T>> {
  const resp = await api.get<PaginatedResponse<T>>(path, config);
  return resp.data;
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
