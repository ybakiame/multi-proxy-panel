//! Background connection tracker: WebSocket push preferred, HTTP polling as the
//! last-resort fallback.
//!
//! sing-box's Clash API upgrades `GET /connections` to a WebSocket that pushes
//! the full connection snapshot every `?interval=<ms>` (see [`super::ws`]).
//! Snapshots are diffed in [`TrackerState`]:
//!
//! - connections disappearing from the snapshot move into a closed ring buffer
//!   (capacity [`CLOSED_BUFFER_CAPACITY`]) for the UI and are persisted as
//!   detail records via the optional [`StatsStore`] sink;
//! - per-connection byte deltas between snapshots are accumulated into the
//!   daily aggregate through the same sink, so long-lived connections show up
//!   in "today's traffic" progressively instead of only at close time.
//!
//! Double-count avoidance: a connection only contributes its full counters (and
//! a `conn_count`) when its `start` is at or after the tracker's own start
//! time; connections that predate the tracker (e.g. after an app restart while
//! the core kept running) only establish a baseline. Transient snapshot
//! failures keep `last_seen` untouched, so recovered snapshots still account
//! the bytes transferred during the outage window.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::connections::clash::{clash_get_connections, parse_connections_response};
use crate::connections::ws::{self, WsConnectError};
use crate::connections::{ActiveConnections, ConnectionView};
use crate::stats::{StatDelta, StatsStore};

/// Capacity of the closed-connection ring buffer.
const CLOSED_BUFFER_CAPACITY: usize = 500;

/// Snapshot interval (ms) requested from the server in WebSocket mode.
const WS_INTERVAL_MS: u64 = 1000;

/// Polling interval in HTTP fallback mode.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Reconnect backoff after a WebSocket session ends or fails transiently.
const WS_RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// One diff round's output: byte deltas for the daily aggregate plus the
/// connections that closed during this round (for detail records).
#[derive(Debug, Clone, Default)]
pub struct TrackerBatch {
    /// Per-connection byte deltas since the previous snapshot.
    pub deltas: Vec<StatDelta>,
    /// Connections that disappeared from the snapshot this round.
    pub closed: Vec<ConnectionView>,
}

/// Background tracker state: holds the last seen snapshot so we can detect
/// disappearing IDs and move them into the closed ring buffer.
pub(crate) struct TrackerState {
    /// Map connection id → ConnectionView (last known snapshot).
    pub(crate) last_seen: HashMap<String, ConnectionView>,
    /// Ring buffer of closed connections.
    pub(crate) closed: Vec<ConnectionView>,
    /// Tracker start time (Unix seconds): only connections started at or after
    /// this point contribute full counters / conn counts (see module docs).
    started_at: i64,
}

impl TrackerState {
    pub(crate) fn new(started_at: i64) -> Self {
        Self {
            last_seen: HashMap::new(),
            closed: Vec::with_capacity(CLOSED_BUFFER_CAPACITY),
            started_at,
        }
    }

    /// Diff the new snapshot against `last_seen`:
    /// - IDs missing from `current` → closed (ring buffer + batch for detail records);
    /// - known IDs → byte delta vs the previous snapshot;
    /// - new IDs → full counters + conn count when the connection started at or
    ///   after the tracker, otherwise baseline only.
    pub(crate) fn update(&mut self, current: Vec<ConnectionView>, now: i64) -> TrackerBatch {
        let current_ids: HashSet<&str> = current.iter().map(|c| c.id.as_str()).collect();

        // Detect closed: existed in last_seen but not in current.
        let mut closed_in_this_round: Vec<ConnectionView> = Vec::new();
        for (id, conn) in &self.last_seen {
            if !current_ids.contains(id.as_str()) {
                closed_in_this_round.push(conn.clone());
            }
        }

        // Byte deltas for the daily aggregate.
        let mut deltas = Vec::new();
        for conn in &current {
            match self.last_seen.get(&conn.id) {
                Some(prev) => {
                    let upload = conn.upload.saturating_sub(prev.upload);
                    let download = conn.download.saturating_sub(prev.download);
                    if upload > 0 || download > 0 {
                        deltas.push(stat_delta(conn, upload, download, false, now));
                    }
                }
                None => {
                    if (conn.start as i64) >= self.started_at {
                        deltas.push(stat_delta(conn, conn.upload, conn.download, true, now));
                    }
                }
            }
        }

        // Replace last_seen with current snapshot.
        self.last_seen.clear();
        for conn in current {
            self.last_seen.insert(conn.id.clone(), conn);
        }

        // Append closed connections to ring buffer, evict oldest on overflow.
        for conn in &closed_in_this_round {
            if self.closed.len() >= CLOSED_BUFFER_CAPACITY {
                self.closed.remove(0);
            }
            self.closed.push(conn.clone());
        }

        TrackerBatch {
            deltas,
            closed: closed_in_this_round,
        }
    }
}

/// Build a [`StatDelta`] from a connection's aggregate dimensions.
fn stat_delta(
    conn: &ConnectionView,
    upload: u64,
    download: u64,
    new_conn: bool,
    now: i64,
) -> StatDelta {
    StatDelta {
        target: conn.target.clone(),
        destination_ip: conn.destination_ip.clone(),
        rule: conn.rule.clone(),
        rule_payload: conn.rule_payload.clone(),
        outbound: conn.outbound.clone(),
        upload,
        download,
        new_conn,
        seen_at: now,
    }
}

/// Handle for the background connection tracking task.
pub struct ConnectionTrackerHandle {
    /// Shared state protected by async mutex.
    state: Arc<Mutex<TrackerState>>,
    /// Background tracking JoinHandle.
    handle: JoinHandle<()>,
}

impl ConnectionTrackerHandle {
    /// Read the current active connections (from the latest snapshot).
    pub async fn active(&self) -> ActiveConnections {
        let guard = self.state.lock().await;
        let mut upload_total = 0u64;
        let mut download_total = 0u64;
        let connections: Vec<ConnectionView> = guard.last_seen.values().cloned().collect();
        for c in &connections {
            upload_total += c.upload;
            download_total += c.download;
        }
        ActiveConnections {
            connections,
            upload_total,
            download_total,
        }
    }

    /// Read the closed-connection ring buffer (oldest first).
    pub async fn closed(&self) -> Vec<ConnectionView> {
        let guard = self.state.lock().await;
        guard.closed.clone()
    }

    /// Stop the background tracking task.
    pub async fn stop(self) {
        self.handle.abort();
        let _ = self.handle.await;
    }
}

/// Start the background connection tracker.
///
/// Prefers the Clash API `/connections` WebSocket push (interval
/// [`WS_INTERVAL_MS`]); permanently falls back to HTTP polling every
/// [`POLL_INTERVAL`] only when the server refuses the WebSocket upgrade, and
/// reconnects the WebSocket with [`WS_RECONNECT_DELAY`] backoff on transient
/// failures.
///
/// `sink` receives per-snapshot batches (deltas + closed connections) for
/// persistent traffic statistics; `None` keeps the tracker in-memory only.
pub fn start_connection_tracker(
    port: u16,
    secret: String,
    sink: Option<Arc<StatsStore>>,
) -> ConnectionTrackerHandle {
    let state = Arc::new(Mutex::new(TrackerState::new(
        chrono::Local::now().timestamp(),
    )));
    let state_clone = Arc::clone(&state);

    let handle = tokio::spawn(async move {
        let mut ws_supported = true;
        loop {
            if ws_supported {
                match ws::connect(port, &secret, WS_INTERVAL_MS).await {
                    Ok(mut stream) => {
                        use futures::StreamExt;
                        while let Some(frame) = stream.next().await {
                            match frame {
                                Ok(text) => handle_snapshot_text(&text, &state_clone, &sink).await,
                                Err(e) => {
                                    tracing::debug!(error = %e, "连接追踪 WebSocket 会话结束，准备重连");
                                    break;
                                }
                            }
                        }
                        tokio::time::sleep(WS_RECONNECT_DELAY).await;
                    }
                    Err(WsConnectError::Unsupported) => {
                        ws_supported = false;
                    }
                    Err(WsConnectError::Transient(e)) => {
                        tracing::debug!(error = %e, "连接追踪 WebSocket 连接失败，稍后重试");
                        tokio::time::sleep(WS_RECONNECT_DELAY).await;
                    }
                }
            } else {
                match clash_get_connections(port, &secret).await {
                    Ok(conns) => apply_snapshot(conns, &state_clone, &sink).await,
                    Err(e) => {
                        // 保留 last_seen：恢复后的快照差分仍能补上故障窗口内的
                        // 字节数；窗口内消失的连接按最后已知计数记为关闭。
                        tracing::debug!(error = %e, "connection tracker poll failed");
                    }
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    });

    ConnectionTrackerHandle { state, handle }
}

/// Parse a raw snapshot JSON frame and apply it.
async fn handle_snapshot_text(
    text: &str,
    state: &Arc<Mutex<TrackerState>>,
    sink: &Option<Arc<StatsStore>>,
) {
    let body: serde_json::Value = match serde_json::from_str(text) {
        Ok(body) => body,
        Err(e) => {
            tracing::debug!(error = %e, "连接追踪快照 JSON 解析失败");
            return;
        }
    };
    let conns = match parse_connections_response(&body) {
        Ok(conns) => conns,
        Err(e) => {
            tracing::debug!(error = %e, "连接追踪快照内容解析失败");
            return;
        }
    };
    apply_snapshot(conns, state, sink).await;
}

/// Diff one snapshot and forward the resulting batch to the stats sink.
async fn apply_snapshot(
    conns: Vec<ConnectionView>,
    state: &Arc<Mutex<TrackerState>>,
    sink: &Option<Arc<StatsStore>>,
) {
    let now = chrono::Local::now().timestamp();
    let batch = {
        let mut guard = state.lock().await;
        guard.update(conns, now)
    };
    if let Some(store) = sink {
        let date = crate::stats::local_date(now);
        if let Err(e) = store.apply_batch(&date, &batch.deltas, &batch.closed).await {
            tracing::warn!(error = %e, "连接统计数据写入失败");
        }
    }
}
