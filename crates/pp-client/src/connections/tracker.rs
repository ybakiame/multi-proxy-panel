//! Background polling tracker that maintains an in-memory ring buffer of closed connections.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::connections::clash::clash_get_connections;
use crate::connections::{ActiveConnections, ConnectionView};

/// Capacity of the closed-connection ring buffer.
const CLOSED_BUFFER_CAPACITY: usize = 500;

/// Polling interval for background connection tracking.
const POLL_INTERVAL_MS: u64 = 2000;

/// Background tracker state: holds the last seen snapshot so we can detect
/// disappearing IDs and move them into the closed ring buffer.
pub(crate) struct TrackerState {
    /// Map connection id → ConnectionView (last known snapshot).
    pub(crate) last_seen: HashMap<String, ConnectionView>,
    /// Ring buffer of closed connections.
    pub(crate) closed: Vec<ConnectionView>,
}

impl TrackerState {
    pub(crate) fn new() -> Self {
        Self {
            last_seen: HashMap::new(),
            closed: Vec::with_capacity(CLOSED_BUFFER_CAPACITY),
        }
    }

    /// Diff the new snapshot against `last_seen`:
    /// - IDs present in `last_seen` but missing in `current` → moved to closed buffer.
    /// - IDs present in both or only in `current` → kept / added to `last_seen`.
    pub(crate) fn update(&mut self, current: Vec<ConnectionView>) {
        let current_ids: std::collections::HashSet<&str> =
            current.iter().map(|c| c.id.as_str()).collect();

        // Detect closed: existed in last_seen but not in current.
        let mut closed_in_this_round: Vec<ConnectionView> = Vec::new();
        for (id, conn) in &self.last_seen {
            if !current_ids.contains(id.as_str()) {
                closed_in_this_round.push(conn.clone());
            }
        }

        // Replace last_seen with current snapshot.
        self.last_seen.clear();
        for conn in &current {
            self.last_seen.insert(conn.id.clone(), conn.clone());
        }

        // Append closed connections to ring buffer, evict oldest on overflow.
        for conn in closed_in_this_round {
            if self.closed.len() >= CLOSED_BUFFER_CAPACITY {
                self.closed.remove(0);
            }
            self.closed.push(conn);
        }
    }
}

/// Handle for the background connection polling task.
pub struct ConnectionTrackerHandle {
    /// Shared state protected by async mutex.
    state: Arc<Mutex<TrackerState>>,
    /// Background polling JoinHandle.
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

    /// Stop the background polling task.
    pub async fn stop(self) {
        self.handle.abort();
        let _ = self.handle.await;
    }
}

/// Start a background task that polls `GET /connections` every 2 seconds.
///
/// The task tracks seen connection IDs; when an ID disappears from the snapshot
/// it is moved into an in-memory ring buffer (capacity 500) as a "closed"
/// record.
///
/// The returned [`ConnectionTrackerHandle`] can be used to read active / closed
/// connections and to stop the background task.
pub fn start_connection_tracker(port: u16, secret: String) -> ConnectionTrackerHandle {
    let state = Arc::new(Mutex::new(TrackerState::new()));
    let state_clone = Arc::clone(&state);

    let handle = tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(POLL_INTERVAL_MS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            match clash_get_connections(port, &secret).await {
                Ok(conns) => {
                    let mut guard = state_clone.lock().await;
                    guard.update(conns);
                }
                Err(e) => {
                    // Core not running or transient failure: clear last_seen so
                    // we don't falsely mark everything as closed on next success.
                    let mut guard = state_clone.lock().await;
                    guard.last_seen.clear();
                    tracing::debug!(error = %e, "connection tracker poll failed");
                }
            }
        }
    });

    ConnectionTrackerHandle { state, handle }
}
