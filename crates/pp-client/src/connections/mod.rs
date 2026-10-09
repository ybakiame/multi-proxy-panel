//! Connection views and Clash API operations.
//!
//! Provides [`clash_get_connections`], [`clash_close_connection`], and a background
//! tracker (WebSocket push preferred, HTTP polling fallback) that maintains an
//! in-memory ring buffer of closed connections and feeds a [`crate::stats::StatsStore`]
//! sink with per-snapshot deltas for persistent traffic statistics.

mod clash;
mod tracker;
mod ws;

pub use clash::{clash_close_connection, clash_get_connections};
pub use tracker::{ConnectionTrackerHandle, TrackerBatch, start_connection_tracker};

use serde::{Deserialize, Serialize};

/// Active or closed connection view (Clash API `GET /connections`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionView {
    /// Connection ID (matches Clash API `id`).
    pub id: String,
    /// Target host: `metadata.host` when available, otherwise `destination_ip:port`.
    pub host: String,
    /// Aggregation target: request domain when known, otherwise the destination IP.
    pub target: String,
    /// Destination IP (`metadata.destinationIP`, empty when unknown).
    pub destination_ip: String,
    /// Leaf outbound tag (`chains[0]`, i.e. the final outbound the traffic exits from).
    pub outbound: String,
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

    fn make_conn(id: &str, host: &str, upload: u64, download: u64, start: u64) -> ConnectionView {
        ConnectionView {
            id: id.into(),
            host: host.into(),
            target: host.into(),
            destination_ip: String::new(),
            outbound: "Proxy".into(),
            network: "tcp".into(),
            chain: "Proxy".into(),
            rule: "DOMAIN".into(),
            rule_payload: host.into(),
            upload,
            download,
            start,
        }
    }

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
        assert_eq!(c1.target, "example.com");
        assert_eq!(c1.destination_ip, "93.184.216.34");
        assert_eq!(c1.outbound, "DIRECT");
        assert_eq!(c1.network, "tcp");
        assert_eq!(c1.chain, "Proxy → DIRECT");
        assert_eq!(c1.rule, "DOMAIN");
        assert_eq!(c1.rule_payload, "example.com");
        assert_eq!(c1.upload, 1024);
        assert_eq!(c1.download, 2048);
        assert_eq!(c1.start, 1704067200);

        let c2 = conns.iter().find(|c| c.id == "conn-2").unwrap();
        assert_eq!(c2.host, "8.8.8.8:53");
        assert_eq!(c2.target, "8.8.8.8");
        assert_eq!(c2.destination_ip, "8.8.8.8");
        assert_eq!(c2.outbound, "DIRECT");
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

        let mut tracker = TrackerState::new(0);

        let conn_a = make_conn("a", "a.com", 100, 200, 1);
        let conn_b = make_conn("b", "b.com", 50, 100, 2);

        // First snapshot: a + b.
        let batch = tracker.update(vec![conn_a.clone(), conn_b.clone()], 10);
        assert_eq!(tracker.last_seen.len(), 2);
        assert!(tracker.closed.is_empty());
        assert!(batch.closed.is_empty());
        // 两条均为 tracker 启动后的新连接：全量入账 + new_conn 计数。
        assert_eq!(batch.deltas.len(), 2);
        assert!(batch.deltas.iter().all(|d| d.new_conn));

        // Second snapshot: only b → a is closed.
        let batch = tracker.update(vec![conn_b.clone()], 20);
        assert_eq!(tracker.last_seen.len(), 1);
        assert_eq!(tracker.closed.len(), 1);
        assert_eq!(tracker.closed[0].id, "a");
        assert_eq!(batch.closed.len(), 1);
        assert_eq!(batch.closed[0].id, "a");

        // Third snapshot: b + c → nothing closed.
        let conn_c = make_conn("c", "c.com", 10, 20, 25);
        let batch = tracker.update(vec![conn_b.clone(), conn_c.clone()], 30);
        assert_eq!(tracker.last_seen.len(), 2);
        assert_eq!(tracker.closed.len(), 1);
        assert!(batch.closed.is_empty());
        // b 无增量；c 为新连接。
        assert_eq!(batch.deltas.len(), 1);
        assert_eq!(batch.deltas[0].target, "c.com");
        assert!(batch.deltas[0].new_conn);
    }

    #[test]
    fn tracker_delta_accounting() {
        use crate::connections::tracker::TrackerState;

        let mut tracker = TrackerState::new(100);

        // 快照一：新连接（start >= started_at）全量入账。
        let batch = tracker.update(vec![make_conn("a", "a.com", 100, 200, 150)], 160);
        assert_eq!(batch.deltas.len(), 1);
        assert_eq!(batch.deltas[0].upload, 100);
        assert_eq!(batch.deltas[0].download, 200);
        assert!(batch.deltas[0].new_conn);

        // 快照二：仅差量入账。
        let batch = tracker.update(vec![make_conn("a", "a.com", 130, 260, 150)], 170);
        assert_eq!(batch.deltas.len(), 1);
        assert_eq!(batch.deltas[0].upload, 30);
        assert_eq!(batch.deltas[0].download, 60);
        assert!(!batch.deltas[0].new_conn);

        // 快照三：无增量不产生 delta。
        let batch = tracker.update(vec![make_conn("a", "a.com", 130, 260, 150)], 180);
        assert!(batch.deltas.is_empty());

        // 快照四：计数器回退（如核心重启复用 ID）→ 饱和减法记 0，不出负账。
        let batch = tracker.update(vec![make_conn("a", "a.com", 10, 10, 150)], 190);
        assert!(batch.deltas.is_empty());
    }

    #[test]
    fn tracker_preexisting_connection_baselines_only() {
        use crate::connections::tracker::TrackerState;

        // tracker 启动前已存在的连接（start < started_at）：只建基线，不入账不计数，
        // 避免 App 重启但核心未重启时把历史流量重复计入今日。
        let mut tracker = TrackerState::new(1000);
        let batch = tracker.update(vec![make_conn("old", "old.com", 999, 999, 10)], 1100);
        assert!(batch.deltas.is_empty());

        // 后续差量正常入账。
        let batch = tracker.update(vec![make_conn("old", "old.com", 1050, 1100, 10)], 1200);
        assert_eq!(batch.deltas.len(), 1);
        assert_eq!(batch.deltas[0].upload, 51);
        assert_eq!(batch.deltas[0].download, 101);
        assert!(!batch.deltas[0].new_conn);
    }

    #[test]
    fn tracker_ring_buffer_evicts_oldest() {
        use crate::connections::tracker::TrackerState;

        const CAPACITY: usize = 500;
        let mut tracker = TrackerState::new(0);

        // Fill buffer to capacity.
        for i in 0..CAPACITY {
            let conn = make_conn(
                &format!("conn-{i}"),
                &format!("host-{i}"),
                i as u64,
                i as u64,
                i as u64,
            );
            tracker.last_seen.insert(conn.id.clone(), conn);
            // Immediately remove by updating with empty vec.
            tracker.update(vec![], i as i64);
        }

        assert_eq!(tracker.closed.len(), CAPACITY);
        assert_eq!(tracker.closed[0].id, "conn-0");

        // One more eviction.
        tracker.last_seen.insert(
            "extra".into(),
            make_conn("extra", "extra.com", 999, 999, 999),
        );
        tracker.update(vec![], 1000);

        assert_eq!(tracker.closed.len(), CAPACITY);
        assert_eq!(tracker.closed[0].id, "conn-1");
        assert_eq!(tracker.closed.last().unwrap().id, "extra");
    }

    #[test]
    fn tracker_keeps_last_seen_across_failures() {
        use crate::connections::tracker::TrackerState;

        // 快照获取失败时不再清空 last_seen：恢复后的差分能补上故障窗口内的流量，
        // 且不会把存活连接误判为关闭。
        let mut tracker = TrackerState::new(0);
        tracker.update(vec![make_conn("x", "x.com", 1, 2, 1)], 10);
        assert_eq!(tracker.last_seen.len(), 1);

        // （故障窗口：外部不发生 update 调用，last_seen 保持不变）

        // 恢复后的快照正常差分。
        let batch = tracker.update(vec![make_conn("x", "x.com", 5, 9, 1)], 30);
        assert_eq!(tracker.last_seen.len(), 1);
        assert!(tracker.closed.is_empty());
        assert_eq!(batch.deltas.len(), 1);
        assert_eq!(batch.deltas[0].upload, 4);
        assert_eq!(batch.deltas[0].download, 7);
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
