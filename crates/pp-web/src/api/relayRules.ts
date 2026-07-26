import { get, post, put, del } from "./client";

export interface RelayRule {
  id: string;
  node_id: string;
  node_name?: string;
  exit_binding_id: string;
  exit_node_id?: string;
  exit_node_name?: string;
  exit_config_name?: string;
  relay_client_id: string;
  name: string;
  match_type: "inline" | "rule_set";
  match_config: {
    domains?: string[];
    domain_suffixes?: string[];
    library?: string;
    custom?: {
      singbox?: { url?: string; format?: string };
      mihomo?: { url?: string; behavior?: string };
    };
  };
  enabled: boolean;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface RuleSetLibraryEntry {
  name: string;
  singbox_url: string;
  singbox_format: string;
  mihomo_url: string;
  mihomo_behavior: string;
}

export const getRelayRules = () => get<RelayRule[]>("/api/v1/relay-rules");
export const getRuleSetLibrary = () => get<RuleSetLibraryEntry[]>("/api/v1/relay-rules/library");
export const createRelayRule = (payload: Partial<RelayRule>) =>
  post<RelayRule>("/api/v1/relay-rules", payload);
export const updateRelayRule = (id: string, payload: Partial<RelayRule>) =>
  put<RelayRule>(`/api/v1/relay-rules/${id}`, payload);
export const deleteRelayRule = (id: string) => del(`/api/v1/relay-rules/${id}`);
