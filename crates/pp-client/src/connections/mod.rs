//! Connection views and Clash API operations.
//!
//! Provides [`clash_get_connections`], [`clash_close_connection`], and a background
//! polling tracker that maintains an in-memory ring buffer of closed connections.

mod clash;
mod tracker;

pub use clash::{clash_get_connections, clash_close_connection};
pub use tracker::{ConnectionTrackerHandle, start_connection_tracker};

use serde::{Deserialize, Serialize};

/// Active or closed connection view (Clash API `GET /connections`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionView {
    /// Connection ID (matches Clash API `id`).
    pub id: String,
    /// Target host: `metadata.host` when available, otherwise `destination_ip:port`.
    pub host: String,
    /// Network protocol, e.g. `tcp` / `udp`.
    pub network: String,
    /// Proxy chain as a human-readable string (`chains` reversed, joined by ` → `).
    pub chain: String,
    /// Matched rule name.
    pub rule: String,
    /// Matched rule payload (e.g. domain / IP-CIDR).
    pub rule_payload: String,
    /// Uploaded bytes.
    pub upload: u64,
    /// Downloaded bytes.
    pub download: u64,
    /// Connection start timestamp (seconds since Unix epoch).
    pub start: u64,
}

/// Active connections summary response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveConnections {
    /// Currently active connections.
    pub connections: Vec<ConnectionView>,
    /// Total uploaded bytes across all active connections.
    pub upload_total: u64,
    /// Total downloaded bytes across all active connections.
    pub download_total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::clash::parse_connections_response;

    #[test]
    fn parse_connections_response_builds_views() {
        let json = serde_json::json!({
            "connections": [
                {
                    "id": "conn-1",
                    "metadata": {
                        "host": "example.com",
                        "network": "tcp",
                        "destinationIP": "93.184.216.34",
                        "destinationPort": "443"
                    },
                    "chains": ["DIRECT", "Proxy"],
                    "rule": "DOMAIN",
                    "rulePayload": "example.com",
                    "upload": 1024,
                    "download": 2048,
                    "start": "2024-01-01T00:00:00+00:00"
                },
                {
                    "id": "conn-2",
                    "metadata": {
                        "network": "udp",
                        "destinationIP": "8.8.8.8",
                        "destinationPort": "53"
                    },
                    "chains": ["DIRECT"],
                    "rule": "MATCH",
                    "rulePayload": "",
                    "upload": 512,
                    "download": 1024,
                    "start": "2024-01-01T00:00:01+00:00"
                }
            ]
        });

        let conns = parse_connections_response(&json).unwrap();
        assert_eq!(conns.len(), 2);

        let c1 = conns.iter().find(|c| c.id == "conn-1").unwrap();
        assert_eq!(c1.host, "example.com");
        assert_eq!(c1.network, "tcp");
        assert_eq!(c1.chain, "Proxy → DIRECT");
        assert_eq!(c1.rule, "DOMAIN");
        assert_eq!(c1.rule_payload, "example.com");
        assert_eq!(c1.upload, 1024);
        assert_eq!(c1.download, 2048);
        assert_eq!(c1.start, 1704067200);

        let c2 = conns.iter().find(|c| c.id == "conn-2").unwrap();
        assert_eq!(c2.host, "8.8.8.8:53");
        assert_eq!(c2.network, "udp");
        assert_eq!(c2.chain, "DIRECT");
        assert_eq!(c2.rule, "MATCH");
        assert_eq!(c2.rule_payload, "");
    }

    #[test]
    fn parse_connections_response_skips_missing_id() {
        let json = serde_json::json!({
            "connections": [
                {
                    "metadata": { "host": "example.com", "network": "tcp" },
                    "upload": 100
                }
            ]
        });
        let conns = parse_connections_response(&json).unwrap();
        assert!(conns.is_empty());
    }

    #[test]
    fn tracker_detects_closed_connections() {
        use crate::connections::tracker::TrackerState;

        let mut tracker = TrackerState::new();

        let conn_a = ConnectionView {
            id: "a".into(),
            host: "a.com".into(),
            network: "tcp".into(),
            chain: "Proxy".into(),
            rule: "DOMAIN".into(),
            rule_payload: "a.com".into(),
            upload: 100,
            download: 200,
            start: 1,
        };
        let conn_b = ConnectionView {
            id: "b".into(),
            host: "b.com".into(),
            network: "udp".into(),
            chain: "DIRECT".into(),
            rule: "MATCH".into(),
            rule_payload: "".into(),
            upload: 50,
            download: 100,
            start: 2,
        };

        // First snapshot: a + b.
        tracker.update(vec![conn_a.clone(), conn_b.clone()]);
        assert_eq!(tracker.last_seen.len(), 2);
        assert!(tracker.closed.is_empty());

        // Second snapshot: only b → a is closed.
        tracker.update(vec![conn_b.clone()]);
        assert_eq!(tracker.last_seen.len(), 1);
        assert_eq!(tracker.closed.len(), 1);
        assert_eq!(tracker.closed[0].id, "a");

        // Third snapshot: b + c → nothing closed.
        let conn_c = ConnectionView {
            id: "c".into(),
            host: "c.com".into(),
            network: "tcp".into(),
            chain: "Proxy".into(),
            rule: "DOMAIN".into(),
            rule_payload: "c.com".into(),
            upload: 10,
            download: 20,
            start: 3,
        };
        tracker.update(vec![conn_b.clone(), conn_c.clone()]);
        assert_eq!(tracker.last_seen.len(), 2);
        assert_eq!(tracker.closed.len(), 1);
    }

    #[test]
    fn tracker_ring_buffer_evicts_oldest() {
        use crate::connections::tracker::TrackerState;

        const CAPACITY: usize = 500;
        let mut tracker = TrackerState::new();

        // Fill buffer to capacity.
        for i in 0..CAPACITY {
            let conn = ConnectionView {
                id: format!("conn-{i}"),
                host: format!("host-{i}"),
                network: "tcp".into(),
                chain: "DIRECT".into(),
                rule: "MATCH".into(),
                rule_payload: "".into(),
                upload: i as u64,
                download: i as u64,
                start: i as u64,
            };
            tracker.last_seen.insert(conn.id.clone(), conn.clone());
            // Immediately remove by updating with empty vec.
            tracker.update(vec![]);
        }

        assert_eq!(tracker.closed.len(), CAPACITY);
        assert_eq!(tracker.closed[0].id, "conn-0");

        // One more eviction.
        let extra = ConnectionView {
            id: "extra".into(),
            host: "extra".into(),
            network: "tcp".into(),
            chain: "DIRECT".into(),
            rule: "MATCH".into(),
            rule_payload: "".into(),
            upload: 999,
            download: 999,
            start: 999,
        };
        tracker.last_seen.insert(extra.id.clone(), extra.clone());
        tracker.update(vec![]);

        assert_eq!(tracker.closed.len(), CAPACITY);
        assert_eq!(tracker.closed[0].id, "conn-1");
        assert_eq!(tracker.closed.last().unwrap().id, "extra");
    }

    #[test]
    fn tracker_clears_last_seen_on_poll_failure() {
        use crate::connections::tracker::TrackerState;

        let mut tracker = TrackerState::new();
        let conn = ConnectionView {
            id: "x".into(),
            host: "x.com".into(),
            network: "tcp".into(),
            chain: "DIRECT".into(),
            rule: "MATCH".into(),
            rule_payload: "".into(),
            upload: 1,
            download: 2,
            start: 1,
        };
        tracker.update(vec![conn.clone()]);
        assert_eq!(tracker.last_seen.len(), 1);

        // Simulate poll failure: clear last_seen.
        tracker.last_seen.clear();
        assert!(tracker.last_seen.is_empty());
        assert!(tracker.closed.is_empty());

        // Next snapshot with same conn should NOT mark it as closed.
        tracker.update(vec![conn.clone()]);
        assert_eq!(tracker.last_seen.len(), 1);
        assert!(tracker.closed.is_empty());
    }

    #[tokio::test]
    async fn clash_get_connections_hits_local_endpoint() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/connections",
            axum::routing::get(|| async {
                axum::Json(serde_json::json!({
                    "connections": [
                        {
                            "id": "test-1",
                            "metadata": {
                                "host": "github.com",
                                "network": "tcp",
                                "destinationIP": "140.82.121.4",
                                "destinationPort": "443"
                            },
                            "chains": ["DIRECT", "Proxy"],
                            "rule": "DOMAIN-SUFFIX",
                            "rulePayload": "github.com",
                            "upload": 100,
                            "download": 200,
                            "start": "2024-06-01T12:00:00+00:00"
                        }
                    ]
                }))
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let conns = clash_get_connections(addr.port(), "").await.unwrap();
        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].id, "test-1");
        assert_eq!(conns[0].host, "github.com");
        assert_eq!(conns[0].chain, "Proxy → DIRECT");
    }

    #[tokio::test]
    async fn clash_close_connection_deletes() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
        let cap_ref = std::sync::Arc::clone(&captured);
        let app = axum::Router::new().route(
            "/connections/{id}",
            axum::routing::delete(
                move |req: axum::http::Request<axum::body::Body>| async move {
                    let path = req.uri().path().to_string();
                    *cap_ref.lock().unwrap() = Some(path);
                    axum::http::StatusCode::NO_CONTENT
                },
            ),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        clash_close_connection(addr.port(), "", "conn-123")
            .await
            .unwrap();
        assert_eq!(
            captured.lock().unwrap().as_ref().unwrap(),
            "/connections/conn-123"
        );
    }

    #[tokio::test]
    async fn clash_api_uses_bearer_auth_for_connections() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
        let cap_ref = std::sync::Arc::clone(&captured);
        let app = axum::Router::new().route(
            "/connections",
            axum::routing::get(
                move |req: axum::http::Request<axum::body::Body>| async move {
                    let auth = req
                        .headers()
                        .get("authorization")
                        .cloned()
                        .map(|h| h.to_str().unwrap_or("").to_string());
                    *cap_ref.lock().unwrap() = auth;
                    axum::Json(serde_json::json!({ "connections": [] }))
                },
            ),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        clash_get_connections(addr.port(), "mysecret")
            .await
            .unwrap();
        assert_eq!(
            captured.lock().unwrap().as_ref().unwrap(),
            "Bearer mysecret"
        );
    }
}
