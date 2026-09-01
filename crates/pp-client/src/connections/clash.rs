//! Clash API client helpers and public operations.

use pp_common::{PanelError, PanelResult};

use crate::connections::ConnectionView;

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

/// Heuristic: detect connection-refused from reqwest error.
fn is_connection_refused(e: &reqwest::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("connection refused") || msg.contains("refused") || msg.contains("os error 111")
}

/// Parse the raw `/connections` JSON into a vector of [`ConnectionView`].
pub(crate) fn parse_connections_response(body: &serde_json::Value) -> PanelResult<Vec<ConnectionView>> {
    let conns = body
        .get("connections")
        .and_then(|c| c.as_array())
        .ok_or_else(|| {
            PanelError::Client("Clash API /connections missing 'connections' array".into())
        })?;

    let mut result = Vec::with_capacity(conns.len());
    for value in conns {
        let Some(obj) = value.as_object() else {
            continue;
        };

        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }

        let metadata = obj.get("metadata").and_then(|m| m.as_object());

        let host = metadata
            .and_then(|m| m.get("host").and_then(|h| h.as_str()))
            .map(String::from)
            .or_else(|| {
                let dst_ip = metadata?.get("destinationIP")?.as_str()?;
                let dst_port = metadata?.get("destinationPort")?.as_str()?;
                Some(format!("{dst_ip}:{dst_port}"))
            })
            .unwrap_or_default();

        let network = metadata
            .and_then(|m| m.get("network").and_then(|n| n.as_str()))
            .unwrap_or("")
            .to_string();

        let chain = obj
            .get("chains")
            .and_then(|c| c.as_array())
            .map(|arr| {
                let mut names: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                names.reverse();
                names.join(" → ")
            })
            .unwrap_or_default();

        let rule = obj
            .get("rule")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();

        let rule_payload = obj
            .get("rulePayload")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();

        let upload = obj.get("upload").and_then(|u| u.as_u64()).unwrap_or(0);

        let download = obj.get("download").and_then(|d| d.as_u64()).unwrap_or(0);

        let start = obj
            .get("start")
            .and_then(|s| s.as_str())
            .and_then(|s| {
                // Clash API returns RFC3339-like strings; parse to timestamp.
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.timestamp() as u64)
            })
            .unwrap_or(0);

        result.push(ConnectionView {
            id,
            host,
            network,
            chain,
            rule,
            rule_payload,
            upload,
            download,
            start,
        });
    }

    Ok(result)
}

/// Fetch active connections from Clash API `GET /connections`.
///
/// Returns [`PanelError::Client`] with "core not running" semantics when the
/// connection is refused / unreachable.
pub async fn clash_get_connections(port: u16, secret: &str) -> PanelResult<Vec<ConnectionView>> {
    let client = build_clash_client(5000)?;
    let request = auth_request(
        client.get(format!("http://127.0.0.1:{port}/connections")),
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
            "Clash API /connections returned HTTP {}",
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PanelError::Client(format!("Clash API /connections invalid JSON: {e}")))?;

    parse_connections_response(&body)
}

/// Close a single connection via Clash API `DELETE /connections/{id}`.
///
/// Returns [`PanelError::Client`] with "core not running" semantics when the
/// connection is refused / unreachable.
pub async fn clash_close_connection(port: u16, secret: &str, id: &str) -> PanelResult<()> {
    let client = build_clash_client(5000)?;
    let request = auth_request(
        client.delete(format!("http://127.0.0.1:{port}/connections/{id}")),
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
            "Clash API DELETE /connections/{id} returned HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}
