//! HTTP API client for ProxyPanel Hub.
//!
//! All requests are authenticated with the API key stored under the
//! `pp_api_key` localStorage key (web) or the `PROXYPANEL_API_KEY` environment
//! variable (native). The key is sent as `Authorization: Bearer <key>`.

use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
const API_KEY_STORAGE_KEY: &str = "pp_api_key";

/// API call result. Uses a custom error type so callers can show backend
/// error messages instead of swallowing `reqwest` errors silently.
pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone, Default)]
pub enum ApiError {
    #[default]
    Unknown,
    Network(String),
    Api {
        status: u16,
        message: String,
    },
    Unauthorized,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unknown => write!(f, "unknown error"),
            ApiError::Network(msg) => write!(f, "network error: {}", msg),
            ApiError::Api { status, message } => {
                write!(f, "server error ({}): {}", status, message)
            }
            ApiError::Unauthorized => write!(f, "unauthorized"),
        }
    }
}

/// Read the stored API key.
pub fn get_api_key() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
            .and_then(|storage| storage.get_item(API_KEY_STORAGE_KEY).ok())
            .flatten()
            .filter(|s| !s.is_empty())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("PROXYPANEL_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())
    }
}

/// Store the API key.
pub fn set_api_key(key: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item(API_KEY_STORAGE_KEY, key);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native builds have no persistent keyring in this scope.
        // SAFETY: this is a best-effort fallback for non-WASM dev builds.
        unsafe { std::env::set_var("PROXYPANEL_API_KEY", key) };
    }
}

/// Remove the stored API key.
pub fn clear_api_key() {
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item(API_KEY_STORAGE_KEY);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // SAFETY: best-effort fallback for non-WASM dev builds.
        unsafe { std::env::remove_var("PROXYPANEL_API_KEY") };
    }
}

/// Resolve the Hub base URL.
///
/// Priority:
/// 1. `PROXYPANEL_API_URL` compile-time environment variable.
/// 2. On WASM: the current page origin with `/api` appended.
/// 3. `PROXYPANEL_API_URL` runtime environment variable on native.
/// 4. Fallback `http://localhost:8081`.
pub fn base_url() -> String {
    if let Some(url) = option_env!("PROXYPANEL_API_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(origin) = window.location().origin() {
                if !origin.is_empty() {
                    return format!("{}/api", origin.trim_end_matches('/'));
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(url) = std::env::var("PROXYPANEL_API_URL") {
            return url;
        }
    }

    "http://localhost:8081".to_string()
}

fn api_client() -> reqwest::Client {
    let mut headers = HeaderMap::new();
    if let Some(key) = get_api_key() {
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", key)) {
            headers.insert("Authorization", value);
        }
    }

    let url = base_url();
    if url.starts_with("http://") && !url.contains("localhost") && !url.contains("127.0.0.1") {
        tracing::warn!(
            "ProxyPanel web is using an insecure HTTP API URL ({}). Use HTTPS in production.",
            url
        );
    }

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

async fn parse_error(resp: reqwest::Response) -> ApiError {
    let status = resp.status().as_u16();
    let message = resp
        .text()
        .await
        .unwrap_or_else(|_| "unknown error".to_string());
    if status == 401 {
        clear_api_key();
        ApiError::Unauthorized
    } else {
        ApiError::Api { status, message }
    }
}

async fn get_json(path: &str) -> ApiResult<Value> {
    let client = api_client();
    let resp = client
        .get(format!("{}{}", base_url(), path))
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    if resp.status().is_success() {
        resp.json()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))
    } else {
        Err(parse_error(resp).await)
    }
}

async fn post_json(path: &str, body: Value) -> ApiResult<Value> {
    let client = api_client();
    let resp = client
        .post(format!("{}{}", base_url(), path))
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    if resp.status().is_success() {
        resp.json()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))
    } else {
        Err(parse_error(resp).await)
    }
}

async fn put_json(path: &str, body: Value) -> ApiResult<Value> {
    let client = api_client();
    let resp = client
        .put(format!("{}{}", base_url(), path))
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    if resp.status().is_success() {
        resp.json()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))
    } else {
        Err(parse_error(resp).await)
    }
}

async fn delete(path: &str) -> ApiResult<()> {
    let client = api_client();
    let resp = client
        .delete(format!("{}{}", base_url(), path))
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(parse_error(resp).await)
    }
}

/// Validate a candidate API key without storing it.
pub async fn validate_api_key(key: &str) -> ApiResult<()> {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", key)) {
        headers.insert("Authorization", value);
    }

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = client
        .get(format!("{}/api/v1/nodes", base_url()))
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(parse_error(resp).await)
    }
}

// ===== Nodes =====

pub async fn get_nodes() -> ApiResult<Value> {
    get_json("/api/v1/nodes").await
}

pub async fn get_nodes_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/nodes?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn create_node(
    name: &str,
    hostname: &str,
    address: &str,
    usage_coefficient: f64,
    labels: Value,
    group_ids: Vec<String>,
    parent_id: Option<&str>,
) -> ApiResult<Value> {
    let mut payload = json!({
        "name": name,
        "hostname": hostname,
        "address": address,
        "usage_coefficient": usage_coefficient,
        "labels": labels,
        "group_ids": group_ids,
    });
    if let Some(pid) = parent_id.filter(|s| !s.is_empty()) {
        payload["parent_id"] = json!(pid);
    }
    post_json("/api/v1/nodes", payload).await
}

pub async fn update_node(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/nodes/{}", id), payload).await
}

pub async fn delete_node(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/nodes/{}", id)).await
}

pub async fn push_config(id: &str, config: Value) -> ApiResult<Value> {
    post_json(&format!("/api/v1/nodes/{}/push", id), config).await
}

// ===== Protocol Configs =====

pub async fn get_protocols(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/protocols?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn get_all_protocols() -> ApiResult<Value> {
    get_json("/api/v1/protocols").await
}

pub async fn create_protocol(
    name: &str,
    protocol_type: &str,
    core_type: &str,
    listen_address: &str,
    listen_port: u64,
    settings: Value,
    tls_settings: Option<Value>,
) -> ApiResult<Value> {
    let mut payload = json!({
        "name": name,
        "protocol_type": protocol_type,
        "core_type": core_type,
        "listen_address": listen_address,
        "listen_port": listen_port,
        "settings": settings,
    });
    if let Some(tls) = tls_settings {
        payload["tls_settings"] = tls;
    }
    post_json("/api/v1/protocols", payload).await
}

pub async fn update_protocol(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/protocols/{}", id), payload).await
}

pub async fn generate_reality_keys() -> ApiResult<Value> {
    get_json("/api/v1/utils/generate-reality-keys").await
}

pub async fn delete_protocol(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/protocols/{}", id)).await
}

// ===== Node Bindings =====

pub async fn get_bindings() -> ApiResult<Value> {
    get_json("/api/v1/bindings").await
}

pub async fn get_bindings_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/bindings?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn create_binding(
    node_id: &str,
    protocol_config_id: &str,
    is_active: bool,
    override_settings: Option<Value>,
) -> ApiResult<Value> {
    let mut payload = json!({
        "node_id": node_id,
        "protocol_config_id": protocol_config_id,
        "is_active": is_active,
    });
    if let Some(ov) = override_settings {
        payload["override_settings"] = ov;
    }
    post_json("/api/v1/bindings", payload).await
}

#[allow(dead_code)]
pub async fn update_binding(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/bindings/{}", id), payload).await
}

pub async fn delete_binding(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/bindings/{}", id)).await
}

// ===== Clients =====

pub async fn get_clients() -> ApiResult<Value> {
    get_json("/api/v1/clients").await
}

pub async fn get_clients_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/clients?page={}&per_page={}",
        page, per_page
    ))
    .await
}

#[allow(dead_code)]
pub async fn create_client(
    name: &str,
    email: Option<&str>,
    traffic_limit_bytes: i64,
    reset_day: Option<i32>,
    data_limit_reset_strategy: &str,
    group_ids: Vec<String>,
) -> ApiResult<Value> {
    let mut payload = json!({
        "name": name,
        "traffic_limit_bytes": traffic_limit_bytes,
        "data_limit_reset_strategy": data_limit_reset_strategy,
        "group_ids": group_ids,
    });
    if let Some(e) = email {
        payload["email"] = json!(e);
    }
    if let Some(rd) = reset_day {
        payload["reset_day"] = json!(rd);
    }
    post_json("/api/v1/clients", payload).await
}

pub async fn create_client_from_payload(payload: Value) -> ApiResult<Value> {
    post_json("/api/v1/clients", payload).await
}

pub async fn update_client(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/clients/{}", id), payload).await
}

pub async fn delete_client(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/clients/{}", id)).await
}

// ===== Subscription Templates =====

pub async fn get_templates() -> ApiResult<Value> {
    get_json("/api/v1/templates").await
}

#[allow(dead_code)]
pub async fn get_templates_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/templates?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn create_template(
    name: &str,
    format: &str,
    base_config: Option<Value>,
    filter_rules: Option<Value>,
    custom_headers: Option<Value>,
) -> ApiResult<Value> {
    let mut payload = json!({ "name": name, "format": format });
    if let Some(v) = base_config {
        payload["base_config"] = v;
    }
    if let Some(v) = filter_rules {
        payload["filter_rules"] = v;
    }
    if let Some(v) = custom_headers {
        payload["custom_headers"] = v;
    }
    post_json("/api/v1/templates", payload).await
}

#[allow(dead_code)]
pub async fn update_template(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/templates/{}", id), payload).await
}

#[allow(dead_code)]
pub async fn delete_template(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/templates/{}", id)).await
}

// ===== Subscriptions =====

pub async fn get_subscriptions_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/subscriptions?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn create_subscription(client_id: &str, template_id: &str) -> ApiResult<Value> {
    post_json(
        "/api/v1/subscriptions",
        json!({ "client_id": client_id, "template_id": template_id }),
    )
    .await
}

pub async fn update_subscription(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/subscriptions/{}", id), payload).await
}

pub async fn delete_subscription(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/subscriptions/{}", id)).await
}

// ===== Node Groups =====

pub async fn get_groups() -> ApiResult<Value> {
    get_json("/api/v1/groups").await
}

pub async fn get_groups_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/groups?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn create_group(
    name: &str,
    description: Option<String>,
    labels: Option<Value>,
) -> ApiResult<Value> {
    let mut payload = json!({ "name": name });
    if let Some(d) = description {
        payload["description"] = json!(d);
    }
    if let Some(l) = labels {
        payload["labels"] = l;
    }
    post_json("/api/v1/groups", payload).await
}

pub async fn update_group(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/groups/{}", id), payload).await
}

pub async fn delete_group(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/groups/{}", id)).await
}

// ===== Traffic =====

pub async fn get_traffic(node_id: Option<&str>, client_id: Option<&str>) -> ApiResult<Value> {
    let mut url = "/api/v1/traffic".to_string();
    let mut first = true;
    if let Some(id) = node_id {
        url.push_str(&format!("?node_id={}", id));
        first = false;
    }
    if let Some(id) = client_id {
        url.push_str(&format!(
            "{}client_id={}",
            if first { "?" } else { "&" },
            id
        ));
    }
    get_json(&url).await
}

// ===== Metrics =====

pub async fn get_metrics() -> ApiResult<Value> {
    get_json("/api/v1/metrics").await
}

pub async fn get_metrics_for_node(node_id: &str) -> ApiResult<Value> {
    get_json(&format!("/api/v1/metrics?node_id={}", node_id)).await
}

// ===== Onlines =====

pub async fn get_online_count() -> ApiResult<Value> {
    get_json("/api/v1/onlines/count").await
}

pub async fn get_onlines(node_id: Option<&str>, client_id: Option<&str>) -> ApiResult<Value> {
    let mut url = "/api/v1/onlines".to_string();
    let mut first = true;
    if let Some(id) = node_id {
        url.push_str(&format!("?node_id={}", id));
        first = false;
    }
    if let Some(id) = client_id {
        url.push_str(&format!(
            "{}client_id={}",
            if first { "?" } else { "&" },
            id
        ));
    }
    get_json(&url).await
}

// ===== Logs =====

pub async fn get_logs() -> ApiResult<Value> {
    get_json("/api/v1/logs").await
}

pub async fn get_logs_filtered(
    level: Option<String>,
    source: Option<String>,
    page: u64,
    per_page: u64,
) -> ApiResult<Value> {
    let mut url = format!("/api/v1/logs?page={}&per_page={}", page, per_page);
    if let Some(l) = level {
        url.push_str(&format!("&level={}", l));
    }
    if let Some(s) = source {
        url.push_str(&format!("&source={}", s));
    }
    get_json(&url).await
}

// ===== API Keys =====

pub async fn get_api_keys_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/api-keys?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn create_api_key(
    name: &str,
    scopes: Value,
    ip_allowlist: Option<Value>,
    rate_limit: Option<i32>,
) -> ApiResult<Value> {
    let mut payload = json!({ "name": name, "scopes": scopes });
    if let Some(v) = ip_allowlist {
        payload["ip_allowlist"] = v;
    }
    if let Some(v) = rate_limit {
        payload["rate_limit"] = json!(v);
    }
    post_json("/api/v1/api-keys", payload).await
}

#[allow(dead_code)]
pub async fn update_api_key(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/api-keys/{}", id), payload).await
}

pub async fn delete_api_key(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/api-keys/{}", id)).await
}

// ===== Webhooks =====

pub async fn get_webhooks_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/webhooks?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn create_webhook(
    name: &str,
    url: &str,
    events: Value,
    secret: Option<String>,
    is_active: bool,
) -> ApiResult<Value> {
    let mut payload = json!({
        "name": name,
        "url": url,
        "events": events,
        "is_active": is_active,
    });
    if let Some(s) = secret {
        payload["secret"] = json!(s);
    }
    post_json("/api/v1/webhooks", payload).await
}

#[allow(dead_code)]
pub async fn update_webhook(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/webhooks/{}", id), payload).await
}

pub async fn delete_webhook(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/webhooks/{}", id)).await
}

// ===== Inbound Hosts =====

#[allow(dead_code)]
pub async fn list_hosts() -> ApiResult<Value> {
    get_json("/api/v1/hosts").await
}

pub async fn get_hosts_paginated(page: u64, per_page: u64) -> ApiResult<Value> {
    get_json(&format!(
        "/api/v1/hosts?page={}&per_page={}",
        page, per_page
    ))
    .await
}

pub async fn create_host(payload: Value) -> ApiResult<Value> {
    post_json("/api/v1/hosts", payload).await
}

#[allow(dead_code)]
pub async fn get_host(id: &str) -> ApiResult<Value> {
    get_json(&format!("/api/v1/hosts/{}", id)).await
}

pub async fn update_host(id: &str, payload: Value) -> ApiResult<Value> {
    put_json(&format!("/api/v1/hosts/{}", id), payload).await
}

pub async fn delete_host(id: &str) -> ApiResult<()> {
    delete(&format!("/api/v1/hosts/{}", id)).await
}
