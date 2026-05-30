//! HTTP API client for ProxyPanel Hub.

use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
fn base_url() -> String {
    // 开发模式下直接指向 Hub 后端地址（Hub 已配置 permissive CORS）
    "http://localhost:8081".to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn base_url() -> String {
    "http://localhost:8081".to_string()
}

// ===== Nodes =====

pub async fn get_nodes() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/nodes", base_url()))
        .await?
        .json()
        .await
}

pub async fn create_node(
    name: &str,
    hostname: &str,
    address: &str,
    group_ids: Vec<String>,
) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .post(format!("{}/api/v1/nodes", base_url()))
        .json(&json!({ "name": name, "hostname": hostname, "address": address, "group_ids": group_ids }))
        .send()
        .await?
        .json()
        .await
}

pub async fn delete_node(id: &str) -> Result<(), reqwest::Error> {
    reqwest::Client::new()
        .delete(format!("{}/api/v1/nodes/{}", base_url(), id))
        .send()
        .await?;
    Ok(())
}

pub async fn push_config(id: &str, config: Value) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .post(format!("{}/api/v1/nodes/{}/push", base_url(), id))
        .json(&config)
        .send()
        .await?
        .json()
        .await
}

// ===== Protocol Configs =====

pub async fn get_protocols(page: u64, per_page: u64) -> Result<Value, reqwest::Error> {
    reqwest::get(format!(
        "{}/api/v1/protocols?page={}&per_page={}",
        base_url(),
        page,
        per_page
    ))
    .await?
    .json()
    .await
}

pub async fn create_protocol(
    name: &str,
    protocol_type: &str,
    core_type: &str,
    listen_address: &str,
    listen_port: u64,
    settings: Value,
) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .post(format!("{}/api/v1/protocols", base_url()))
        .json(&json!({
            "name": name,
            "protocol_type": protocol_type,
            "core_type": core_type,
            "listen_address": listen_address,
            "listen_port": listen_port,
            "settings": settings,
        }))
        .send()
        .await?
        .json()
        .await
}

pub async fn update_protocol(id: &str, payload: Value) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .put(format!("{}/api/v1/protocols/{}", base_url(), id))
        .json(&payload)
        .send()
        .await?
        .json()
        .await
}

pub async fn generate_reality_keys() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/utils/generate-reality-keys", base_url()))
        .await?
        .json()
        .await
}

pub async fn delete_protocol(id: &str) -> Result<(), reqwest::Error> {
    reqwest::Client::new()
        .delete(format!("{}/api/v1/protocols/{}", base_url(), id))
        .send()
        .await?;
    Ok(())
}

// ===== Node Bindings =====

pub async fn get_bindings() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/bindings", base_url()))
        .await?
        .json()
        .await
}

pub async fn create_binding(
    node_id: &str,
    protocol_config_id: &str,
    is_active: bool,
) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .post(format!("{}/api/v1/bindings", base_url()))
        .json(&json!({
            "node_id": node_id,
            "protocol_config_id": protocol_config_id,
            "is_active": is_active,
        }))
        .send()
        .await?
        .json()
        .await
}

pub async fn delete_binding(id: &str) -> Result<(), reqwest::Error> {
    reqwest::Client::new()
        .delete(format!("{}/api/v1/bindings/{}", base_url(), id))
        .send()
        .await?;
    Ok(())
}

// ===== Clients =====

pub async fn get_clients() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/clients", base_url()))
        .await?
        .json()
        .await
}

pub async fn create_client(
    name: &str,
    email: Option<&str>,
    traffic_limit_bytes: i64,
    reset_day: Option<i32>,
    data_limit_reset_strategy: &str,
    group_ids: Vec<String>,
) -> Result<Value, reqwest::Error> {
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
    reqwest::Client::new()
        .post(format!("{}/api/v1/clients", base_url()))
        .json(&payload)
        .send()
        .await?
        .json()
        .await
}

#[allow(dead_code)]
pub async fn update_client(id: &str, payload: Value) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .put(format!("{}/api/v1/clients/{}", base_url(), id))
        .json(&payload)
        .send()
        .await?
        .json()
        .await
}

pub async fn delete_client(id: &str) -> Result<(), reqwest::Error> {
    reqwest::Client::new()
        .delete(format!("{}/api/v1/clients/{}", base_url(), id))
        .send()
        .await?;
    Ok(())
}

// ===== Subscription Templates =====

pub async fn get_templates() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/templates", base_url()))
        .await?
        .json()
        .await
}

#[allow(dead_code)]
pub async fn create_template(name: &str, format: &str) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .post(format!("{}/api/v1/templates", base_url()))
        .json(&json!({ "name": name, "format": format }))
        .send()
        .await?
        .json()
        .await
}

#[allow(dead_code)]
pub async fn delete_template(id: &str) -> Result<(), reqwest::Error> {
    reqwest::Client::new()
        .delete(format!("{}/api/v1/templates/{}", base_url(), id))
        .send()
        .await?;
    Ok(())
}

// ===== Subscriptions =====

pub async fn get_subscriptions() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/subscriptions", base_url()))
        .await?
        .json()
        .await
}

pub async fn create_subscription(
    client_id: &str,
    template_id: &str,
) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .post(format!("{}/api/v1/subscriptions", base_url()))
        .json(&json!({ "client_id": client_id, "template_id": template_id }))
        .send()
        .await?
        .json()
        .await
}

pub async fn delete_subscription(id: &str) -> Result<(), reqwest::Error> {
    reqwest::Client::new()
        .delete(format!("{}/api/v1/subscriptions/{}", base_url(), id))
        .send()
        .await?;
    Ok(())
}

// ===== Node Groups =====

pub async fn get_groups() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/groups", base_url()))
        .await?
        .json()
        .await
}

pub async fn create_group(
    name: &str,
    description: Option<String>,
) -> Result<Value, reqwest::Error> {
    let mut payload = json!({ "name": name });
    if let Some(d) = description {
        payload["description"] = json!(d);
    }
    reqwest::Client::new()
        .post(format!("{}/api/v1/groups", base_url()))
        .json(&payload)
        .send()
        .await?
        .json()
        .await
}

#[allow(dead_code)]
pub async fn update_group(id: &str, payload: Value) -> Result<Value, reqwest::Error> {
    reqwest::Client::new()
        .put(format!("{}/api/v1/groups/{}", base_url(), id))
        .json(&payload)
        .send()
        .await?
        .json()
        .await
}

pub async fn delete_group(id: &str) -> Result<(), reqwest::Error> {
    reqwest::Client::new()
        .delete(format!("{}/api/v1/groups/{}", base_url(), id))
        .send()
        .await?;
    Ok(())
}

// ===== Traffic =====

#[allow(dead_code)]
pub async fn get_traffic() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/traffic", base_url()))
        .await?
        .json()
        .await
}

// ===== Metrics =====

pub async fn get_metrics() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/metrics", base_url()))
        .await?
        .json()
        .await
}

pub async fn get_metrics_for_node(node_id: &str) -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/metrics?node_id={}", base_url(), node_id))
        .await?
        .json()
        .await
}

// ===== Onlines =====

#[allow(dead_code)]
pub async fn get_online_count() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/onlines/count", base_url()))
        .await?
        .json()
        .await
}

// ===== Logs =====

pub async fn get_logs() -> Result<Value, reqwest::Error> {
    reqwest::get(format!("{}/api/v1/logs", base_url()))
        .await?
        .json()
        .await
}

pub async fn get_logs_filtered(
    level: Option<String>,
    source: Option<String>,
    limit: u64,
) -> Result<Value, reqwest::Error> {
    let mut url = format!("{}/api/v1/logs?limit={}", base_url(), limit);
    if let Some(l) = level {
        url.push_str(&format!("&level={}", l));
    }
    if let Some(s) = source {
        url.push_str(&format!("&source={}", s));
    }
    reqwest::get(url).await?.json().await
}
