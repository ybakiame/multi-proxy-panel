const API_KEY_STORAGE_KEY = "pp_api_key";

export function getApiKey(): string | null {
  return localStorage.getItem(API_KEY_STORAGE_KEY);
}

export function setApiKey(key: string): void {
  localStorage.setItem(API_KEY_STORAGE_KEY, key);
}

export function clearApiKey(): void {
  localStorage.removeItem(API_KEY_STORAGE_KEY);
}

export function baseUrl(): string {
  if (import.meta.env.VITE_PROXYPANEL_API_URL) {
    return import.meta.env.VITE_PROXYPANEL_API_URL as string;
  }

  if (import.meta.env.PROD) {
    return `${window.location.origin}/api`;
  }

  return import.meta.env.VITE_API_BASE_URL || "http://localhost:8081";
}
