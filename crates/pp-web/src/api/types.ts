export interface ApiError {
  code: string;
  status: number;
  message: string;
}

export interface PaginatedResponse<T> {
  data: T[];
  pagination: {
    page: number;
    per_page: number;
    total: number;
    total_pages: number;
  };
}

export interface ApiResponse<T> {
  data: T;
}

export interface CoreStatus {
  core_type: string;
  version: string;
  running: boolean;
  uptime_sec: number;
  last_error: string | null;
  updated_at: string;
}

export interface Node {
  id: string;
  name: string;
  hostname: string;
  address: string;
  cores_available: string[];
  labels: Record<string, string>;
  usage_coefficient: number;
  status: string;
  parent_id: string | null;
  last_seen_at: string | null;
  created_at: string;
  updated_at: string;
  token?: string;
  core_statuses?: CoreStatus[];
}

export interface ManagedCertificate {
  id: string;
  node_id: string;
  node_name: string | null;
  domain: string;
  status: string;
  challenge_type: string;
  expires_at: string | null;
  last_issued_at: string | null;
  last_error: string | null;
  created_at: string;
}

export interface CoreVersion {
  id: string;
  core_type: string;
  version: string;
  channel: string;
  is_active: boolean;
  published_at?: string | null;
  commit_sha?: string | null;
  created_at: string;
}

export interface ProtocolConfig {
  id: string;
  name: string;
  protocol_type: string;
  core_type: string;
  core_version: string | null;
  listen_address: string;
  listen_port: number;
  settings: Record<string, unknown>;
  tls_settings: Record<string, unknown> | null;
  created_at: string;
  updated_at: string;
}

export interface Binding {
  id: string;
  node_id: string;
  protocol_config_id: string;
  is_active: boolean;
  override_settings: Record<string, unknown> | null;
  group_ids: string[];
  created_at: string;
}

export interface InboundHost {
  id: string;
  protocol_config_id: string;
  node_id: string;
  remark: string;
  address: string;
  port: number;
  sni: string | null;
  host: string | null;
  path: string | null;
  security: string | null;
  alpn: string | null;
  fingerprint: string | null;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface Client {
  id: string;
  user_id: string;
  name: string;
  email: string | null;
  traffic_limit_bytes: number;
  traffic_used_bytes: number;
  all_time_used_bytes: number;
  traffic_used_total: number;
  is_exceeded: boolean;
  expiry_date: string | null;
  reset_day: number | null;
  data_limit_reset_strategy: string;
  last_traffic_reset_time: string | null;
  max_devices: number | null;
  status: string;
  on_hold_expire_duration_secs: number | null;
  on_hold_timeout: string | null;
  group_ids: string[];
  created_at: string;
  updated_at: string;
}

export interface Group {
  id: string;
  name: string;
  description: string | null;
  labels: Record<string, string> | null;
  binding_ids: string[];
  created_at: string;
  updated_at: string;
}

export interface SubscriptionTemplate {
  id: string;
  name: string;
  format: string;
  base_config: string | null;
  filter_rules: Record<string, unknown> | null;
  custom_headers: Record<string, string> | null;
  is_builtin: boolean;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface Subscription {
  id: string;
  client_id: string;
  token: string;
  url_path: string;
  is_active: boolean;
  expire_at: string | null;
  last_accessed_at: string | null;
  created_at: string;
}

export interface AgentLog {
  id: string;
  node_id: string;
  level: string;
  target: string;
  message: string;
  fields: Record<string, unknown> | null;
  created_at: string;
}

export interface Metric {
  id: string;
  node_id: string;
  timestamp: string;
  cpu_percent: number;
  mem_used: number;
  mem_total: number;
  disk_used: number;
  disk_total: number;
  net_rx: number;
  net_tx: number;
  load_avg1: number;
  load_avg5: number;
  load_avg15: number;
}

export interface OnlineSession {
  id: string;
  client_id: string;
  node_id: string;
  ip_address: string;
  inbound_tag: string | null;
  connected_at: string;
  last_active_at: string;
}

export interface TrafficRecord {
  id: string;
  node_id: string | null;
  protocol_config_id: string | null;
  client_id: string | null;
  hour_bucket: string;
  upload_bytes: number;
  download_bytes: number;
  created_at: string;
}

export interface UsageRecord {
  id: string;
  node_id: string;
  client_id: string;
  hour_bucket: string;
  upload_bytes: number;
  download_bytes: number;
  rate: number;
}

export interface UsageSummaryItem {
  id: string;
  upload_bytes: number;
  download_bytes: number;
  total_bytes: number;
}

export interface ClientIps {
  client_id: string;
  ips: string[];
}

export interface Log {
  id: string;
  level: string;
  source: string;
  message: string;
  metadata: Record<string, unknown> | null;
  created_at: string;
}

export interface ApiKey {
  id: string;
  name: string;
  scopes: string[];
  ip_allowlist: string[] | null;
  rate_limit: number | null;
  expires_at: string | null;
  is_active: boolean;
  created_at: string;
}

export interface Webhook {
  id: string;
  name: string;
  url: string;
  events: string[];
  secret: string | null;
  is_active: boolean;
  created_at: string;
}
