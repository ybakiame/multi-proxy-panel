//! Proxy group / node views and Clash API operations.
//!
//! Provides [`clash_get_proxies`], [`clash_select_proxy`], [`clash_test_delay`] and
//! [`replay_group_selections`] for runtime group selection persistence.

use pp_common::{PanelError, PanelResult};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// View types
// ---------------------------------------------------------------------------

/// Proxy group view (Selector / URLTest / Fallback / LoadBalance …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupView {
    /// Group name (matches Clash API `name`).
    pub name: String,
    /// Group type, e.g. `Selector`, `URLTest`.
    pub group_type: String,
    /// Currently selected member name (for Selector-like groups).
    pub now: String,
    /// Member node names.
    pub members: Vec<String>,
}

/// Proxy node view (individual proxy / relay).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeView {
    /// Node name.
    pub name: String,
    /// Node type, e.g. `Shadowsocks`, `Vmess`, `Trojan`, `Direct` …
    pub node_type: String,
    /// Last measured delay in milliseconds (`None` = untested or timeout).
    pub delay_ms: Option<u16>,
    /// Whether UDP is supported (parsed from `udp` boolean in Clash API).
    pub udp: bool,
}

/// Unified proxy list response: groups and flat nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyList {
    /// All selectable groups.
    pub groups: Vec<GroupView>,
    /// All individual nodes (excluding built-in pseudo-nodes such as DIRECT / REJECT).
    pub nodes: Vec<NodeView>,
}

// ---------------------------------------------------------------------------
// Clash API client helpers
// ---------------------------------------------------------------------------

/// Default delay-test URL used by [`clash_test_delay`] when caller passes `None`.
const DEFAULT_DELAY_TEST_URL: &str = "https://www.gstatic.com/generate_204";

/// Build a `reqwest` client for Clash API: short timeout, no proxy, direct loopback.
fn build_clash_client(timeout_ms: u64) -> PanelResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .no_proxy()
        .build()
        .map_err(|e| PanelError::Client(format!("build Clash API client failed: {e}")))
}

/// Attach Bearer auth when `secret` is non-empty.
fn auth_request(request: reqwest::RequestBuilder, secret: &str) -> reqwest::RequestBuilder {
    if secret.is_empty() {
        request
    } else {
        request.bearer_auth(secret)
    }
}

/// Parse the raw `/proxies` JSON into [`ProxyList`].
///
/// Filtering rules:
/// - `GLOBAL` group is skipped.
/// - Built-in pseudo-nodes (`DIRECT`, `REJECT`, `REJECT-DROP`, `PASS`, `COMPATIBLE`, `BLOCK`, `NOAP`)
///   are excluded from the flat `nodes` list.
/// - Any proxy whose `type` equals `Direct`, `Reject`, `Pass`, `Compatible`, `Block`, `NoAP` is
///   also treated as a pseudo-node and filtered out.
fn parse_proxies_response(body: &serde_json::Value) -> PanelResult<ProxyList> {
    let proxies = body
        .get("proxies")
        .and_then(|p| p.as_object())
        .ok_or_else(|| PanelError::Client("Clash API /proxies missing 'proxies' object".into()))?;

    let mut groups = Vec::new();
    let mut nodes = Vec::new();

    for (name, value) in proxies {
        if name == "GLOBAL" {
            continue;
        }
        let Some(obj) = value.as_object() else {
            continue;
        };

        let proxy_type = obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        // Group detection: has `all` array.
        if let Some(all) = obj.get("all").and_then(|a| a.as_array()) {
            let now = obj
                .get("now")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let members: Vec<String> = all
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            groups.push(GroupView {
                name: name.clone(),
                group_type: proxy_type.clone(),
                now,
                members,
            });
        }

        // Node: skip built-in pseudo-nodes.
        if is_pseudo_node(name, &proxy_type) {
            continue;
        }

        let delay_ms = obj
            .get("history")
            .and_then(|h| h.as_array())
            .and_then(|arr| arr.last())
            .and_then(|last| last.get("delay"))
            .and_then(|d| d.as_u64())
            .and_then(|d| u16::try_from(d).ok());

        let udp = obj.get("udp").and_then(|u| u.as_bool()).unwrap_or(false);

        nodes.push(NodeView {
            name: name.clone(),
            node_type: proxy_type,
            delay_ms,
            udp,
        });
    }

    Ok(ProxyList { groups, nodes })
}

/// Built-in pseudo-node names (uppercase) that should not appear in the node list.
const PSEUDO_NODE_NAMES: &[&str] = &[
    "DIRECT",
    "REJECT",
    "REJECT-DROP",
    "PASS",
    "COMPATIBLE",
    "BLOCK",
    "NOAP",
];

/// Built-in pseudo-node types (case-insensitive) that should not appear in the node list.
const PSEUDO_NODE_TYPES: &[&str] = &["Direct", "Reject", "Pass", "Compatible", "Block", "NoAP"];

fn is_pseudo_node(name: &str, node_type: &str) -> bool {
    if PSEUDO_NODE_NAMES
        .iter()
        .any(|&n| n.eq_ignore_ascii_case(name))
    {
        return true;
    }
    if PSEUDO_NODE_TYPES
        .iter()
        .any(|&t| t.eq_ignore_ascii_case(node_type))
    {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Public Clash API operations
// ---------------------------------------------------------------------------

/// Fetch proxy list from Clash API `GET /proxies`.
///
/// Returns [`PanelError::Client`] with "core not running" semantics when the
/// connection is refused / unreachable.
pub async fn clash_get_proxies(port: u16, secret: &str) -> PanelResult<ProxyList> {
    let client = build_clash_client(5000)?;
    let request = auth_request(
        client.get(format!("http://127.0.0.1:{port}/proxies")),
        secret,
    );

    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) if e.is_connect() || e.is_timeout() || is_connection_refused(&e) => {
            return Err(PanelError::Client(format!("core not running: {e}")));
        }
        Err(e) => return Err(PanelError::Client(format!("Clash API request failed: {e}"))),
    };

    if !resp.status().is_success() {
        return Err(PanelError::Client(format!(
            "Clash API /proxies returned HTTP {}",
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PanelError::Client(format!("Clash API /proxies invalid JSON: {e}")))?;

    parse_proxies_response(&body)
}

/// Heuristic: detect connection-refused from reqwest error.
fn is_connection_refused(e: &reqwest::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("connection refused") || msg.contains("refused") || msg.contains("os error 111")
}

/// Select a proxy in a group via Clash API `PUT /proxies/{group}`.
///
/// Body: `{"name": "<name>"}`.
pub async fn clash_select_proxy(
    port: u16,
    secret: &str,
    group: &str,
    name: &str,
) -> PanelResult<()> {
    let client = build_clash_client(5000)?;
    let request = auth_request(
        client
            .put(format!("http://127.0.0.1:{port}/proxies/{group}"))
            .json(&serde_json::json!({ "name": name })),
        secret,
    );

    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) if e.is_connect() || e.is_timeout() || is_connection_refused(&e) => {
            return Err(PanelError::Client(format!("core not running: {e}")));
        }
        Err(e) => return Err(PanelError::Client(format!("Clash API request failed: {e}"))),
    };

    if !resp.status().is_success() {
        return Err(PanelError::Client(format!(
            "Clash API PUT /proxies/{group} returned HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}

/// Test delay of a single proxy via Clash API `GET /proxies/{name}/delay`.
///
/// `url` defaults to [`DEFAULT_DELAY_TEST_URL`]; `timeout_ms` is the query
/// parameter sent to Clash API (also used as the HTTP client timeout).
/// Returns `Ok(None)` when the test fails or times out.
pub async fn clash_test_delay(
    port: u16,
    secret: &str,
    name: &str,
    url: Option<&str>,
    timeout_ms: u64,
) -> PanelResult<Option<u16>> {
    let test_url = url.unwrap_or(DEFAULT_DELAY_TEST_URL);
    let client = build_clash_client(timeout_ms + 1000)?;
    let request = auth_request(
        client.get(format!(
            "http://127.0.0.1:{port}/proxies/{name}/delay?url={}&timeout={}",
            urlencoding::encode(test_url),
            timeout_ms
        )),
        secret,
    );

    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) if e.is_connect() || e.is_timeout() || is_connection_refused(&e) => {
            return Err(PanelError::Client(format!("core not running: {e}")));
        }
        Err(e) => {
            tracing::debug!(error = %e, name, "proxy delay test request failed");
            return Ok(None);
        }
    };

    if !resp.status().is_success() {
        tracing::debug!(status = %resp.status(), name, "proxy delay test returned non-2xx");
        return Ok(None);
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(error = %e, name, "proxy delay test invalid JSON");
            return Ok(None);
        }
    };

    let delay = body
        .get("delay")
        .and_then(|d| d.as_u64())
        .and_then(|d| u16::try_from(d).ok());

    Ok(delay)
}

// ---------------------------------------------------------------------------
// Selection persistence helpers
// ---------------------------------------------------------------------------

/// Persist a group selection to [`ClientConfig::group_selections`] and save
/// `client.json`.
///
/// Called by the Tauri command layer after a successful [`clash_select_proxy`].
pub fn persist_group_selection(
    data_dir: &std::path::Path,
    group: &str,
    name: &str,
) -> PanelResult<()> {
    let mut cfg = crate::config::ClientConfig::load(data_dir)
        .map_err(|e| PanelError::Client(format!("failed to load config: {e}")))?;
    cfg.group_selections
        .insert(group.to_string(), name.to_string());
    cfg.save()
        .map_err(|e| PanelError::Client(format!("failed to save config: {e}")))
}

/// Replay persisted group selections after core startup.
///
/// Iterates over [`ClientConfig::group_selections`] and calls
/// [`clash_select_proxy`] for each entry. Individual failures are logged as
/// warnings and do **not** abort the replay.
pub async fn replay_group_selections(port: u16, secret: &str, data_dir: &std::path::Path) {
    let cfg = match crate::config::ClientConfig::load(data_dir) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "skip group-selection replay: config load failed");
            return;
        }
    };

    if cfg.group_selections.is_empty() {
        return;
    }

    for (group, name) in &cfg.group_selections {
        if let Err(e) = clash_select_proxy(port, secret, group, name).await {
            tracing::warn!(group, name, error = %e, "group selection replay failed");
        } else {
            tracing::info!(group, name, "group selection replayed");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_proxies_response_builds_groups_and_nodes() {
        let json = serde_json::json!({
            "proxies": {
                "GLOBAL": {
                    "name": "GLOBAL",
                    "type": "Selector",
                    "now": "Auto",
                    "all": ["Auto", "DIRECT"]
                },
                "Auto": {
                    "name": "Auto",
                    "type": "URLTest",
                    "now": "Node-A",
                    "all": ["Node-A", "Node-B", "DIRECT"],
                    "history": [{"delay": 120}]
                },
                "Node-A": {
                    "name": "Node-A",
                    "type": "Shadowsocks",
                    "history": [{"delay": 150}],
                    "udp": true
                },
                "Node-B": {
                    "name": "Node-B",
                    "type": "Vmess",
                    "history": [],
                    "udp": false
                },
                "DIRECT": {
                    "name": "DIRECT",
                    "type": "Direct",
                    "history": []
                },
                "REJECT": {
                    "name": "REJECT",
                    "type": "Reject",
                    "history": []
                }
            }
        });

        let list = parse_proxies_response(&json).unwrap();

        // GLOBAL skipped.
        assert!(!list.groups.iter().any(|g| g.name == "GLOBAL"));

        // Auto group present.
        let auto = list.groups.iter().find(|g| g.name == "Auto").unwrap();
        assert_eq!(auto.group_type, "URLTest");
        assert_eq!(auto.now, "Node-A");
        assert_eq!(auto.members, vec!["Node-A", "Node-B", "DIRECT"]);

        // Nodes: DIRECT / REJECT filtered out.
        let node_names: Vec<&str> = list.nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(node_names.contains(&"Node-A"));
        assert!(node_names.contains(&"Node-B"));
        assert!(!node_names.contains(&"DIRECT"));
        assert!(!node_names.contains(&"REJECT"));

        // Node details.
        let node_a = list.nodes.iter().find(|n| n.name == "Node-A").unwrap();
        assert_eq!(node_a.node_type, "Shadowsocks");
        assert_eq!(node_a.delay_ms, Some(150));
        assert!(node_a.udp);

        let node_b = list.nodes.iter().find(|n| n.name == "Node-B").unwrap();
        assert_eq!(node_b.node_type, "Vmess");
        assert_eq!(node_b.delay_ms, None);
        assert!(!node_b.udp);
    }

    #[test]
    fn parse_proxies_response_skips_compatible_pseudo_nodes() {
        let json = serde_json::json!({
            "proxies": {
                "COMPATIBLE": {
                    "name": "COMPATIBLE",
                    "type": "Compatible",
                    "history": []
                },
                "RealNode": {
                    "name": "RealNode",
                    "type": "Trojan",
                    "history": [{"delay": 200}],
                    "udp": true
                }
            }
        });
        let list = parse_proxies_response(&json).unwrap();
        assert_eq!(list.nodes.len(), 1);
        assert_eq!(list.nodes[0].name, "RealNode");
    }

    #[test]
    fn group_selections_serde_backward_compat() {
        // Old client.json without group_selections should load with default empty map.
        let json = r#"{
            "data_dir": "/tmp/pp-client-test",
            "hub_url": "http://127.0.0.1:50052",
            "sub_token": "tok",
            "core_type": "singbox",
            "core_binary": "/usr/local/bin/sing-box",
            "mixed_port": 17890,
            "mitm_enabled": true,
            "mitm": { "ca_dir": "/tmp/pp-client-test/certs", "hostnames": [], "script_dialect": "Surge" },
            "system_proxy_enabled": false
        }"#;
        let cfg: crate::config::ClientConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.group_selections.is_empty());
    }

    #[test]
    fn group_selections_roundtrip() {
        let mut cfg = crate::config::ClientConfig::new(
            std::path::PathBuf::from("/tmp/pp-client-test"),
            "http://127.0.0.1:50052",
            "tok",
            pp_common::CoreType::SingBox,
            std::path::PathBuf::from("/usr/local/bin/sing-box"),
        );
        cfg.group_selections.insert("Auto".into(), "Node-A".into());
        cfg.group_selections.insert("Proxy".into(), "Node-B".into());

        let json = serde_json::to_string(&cfg).unwrap();
        let back: crate::config::ClientConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.group_selections.len(), 2);
        assert_eq!(
            back.group_selections.get("Auto"),
            Some(&"Node-A".to_string())
        );
        assert_eq!(
            back.group_selections.get("Proxy"),
            Some(&"Node-B".to_string())
        );
    }

    #[tokio::test]
    async fn clash_get_proxies_hits_local_endpoint() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/proxies",
            axum::routing::get(|| async {
                axum::Json(serde_json::json!({
                    "proxies": {
                        "Proxy": {
                            "name": "Proxy",
                            "type": "Selector",
                            "now": "Node1",
                            "all": ["Node1", "Node2"]
                        },
                        "Node1": {
                            "name": "Node1",
                            "type": "Shadowsocks",
                            "history": [{"delay": 100}],
                            "udp": true
                        },
                        "Node2": {
                            "name": "Node2",
                            "type": "Vmess",
                            "history": [],
                            "udp": false
                        }
                    }
                }))
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let list = clash_get_proxies(addr.port(), "").await.unwrap();
        assert_eq!(list.groups.len(), 1);
        assert_eq!(list.groups[0].name, "Proxy");
        // Proxy groups themselves are also listed as nodes if they are not pseudo-nodes.
        // Node1 + Node2 = 2 nodes (Proxy is a Selector, not a pseudo-node, so it also appears).
        assert_eq!(list.nodes.len(), 3);
    }

    #[tokio::test]
    async fn clash_select_proxy_puts_with_body() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
        let cap_ref = std::sync::Arc::clone(&captured);
        let app = axum::Router::new().route(
            "/proxies/{group}",
            axum::routing::put(
                move |req: axum::http::Request<axum::body::Body>| async move {
                    let bytes = axum::body::to_bytes(req.into_body(), 1024).await.unwrap();
                    *cap_ref.lock().unwrap() = Some(bytes.to_vec());
                    axum::http::StatusCode::NO_CONTENT
                },
            ),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        clash_select_proxy(addr.port(), "", "Proxy", "Node1")
            .await
            .unwrap();
        assert_eq!(
            captured.lock().unwrap().as_ref().unwrap(),
            &br#"{"name":"Node1"}"#.to_vec()
        );
    }

    #[tokio::test]
    async fn clash_test_delay_parses_delay_field() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/proxies/{name}/delay",
            axum::routing::get(|| async { axum::Json(serde_json::json!({ "delay": 233 })) }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let delay = clash_test_delay(addr.port(), "", "Node1", None, 5000)
            .await
            .unwrap();
        assert_eq!(delay, Some(233));
    }

    #[tokio::test]
    async fn clash_test_delay_returns_none_on_non_2xx() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/proxies/{name}/delay",
            axum::routing::get(|| async { axum::http::StatusCode::BAD_REQUEST }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let delay = clash_test_delay(addr.port(), "", "Node1", None, 5000)
            .await
            .unwrap();
        assert_eq!(delay, None);
    }

    #[tokio::test]
    async fn clash_api_uses_bearer_auth_when_secret_set() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
        let cap_ref = std::sync::Arc::clone(&captured);
        let app = axum::Router::new().route(
            "/proxies",
            axum::routing::get(
                move |req: axum::http::Request<axum::body::Body>| async move {
                    let auth = req
                        .headers()
                        .get("authorization")
                        .cloned()
                        .map(|h| h.to_str().unwrap_or("").to_string());
                    *cap_ref.lock().unwrap() = auth;
                    axum::Json(serde_json::json!({ "proxies": {} }))
                },
            ),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        clash_get_proxies(addr.port(), "mysecret").await.unwrap();
        assert_eq!(
            captured.lock().unwrap().as_ref().unwrap(),
            "Bearer mysecret"
        );
    }
}
