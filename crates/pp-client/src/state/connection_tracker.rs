//! Connection-tracker-related methods for [`ClientState`].

use crate::state::ClientState;
use pp_common::PanelResult;

impl ClientState {
    /// Stop the background connection tracker.
    pub(crate) async fn stop_connection_tracker(&mut self) {
        if let Some(handle) = self.connection_tracker.take() {
            handle.tracker.stop().await;
        }
    }

    /// Read active connections from the background tracker.
    ///
    /// Returns `None` when the tracker is not running (core stopped or Clash API disabled).
    pub async fn active_connections(&self) -> Option<crate::connections::ActiveConnections> {
        if let Some(ref tracker) = self.connection_tracker {
            Some(tracker.tracker.active().await)
        } else {
            None
        }
    }

    /// Read closed connections from the background tracker ring buffer.
    ///
    /// Returns `None` when the tracker is not running.
    pub async fn closed_connections(&self) -> Option<Vec<crate::connections::ConnectionView>> {
        if let Some(ref tracker) = self.connection_tracker {
            Some(tracker.tracker.closed().await)
        } else {
            None
        }
    }

    /// Close a single active connection by ID via Clash API.
    ///
    /// Returns error when core is not running or Clash API is unreachable.
    pub async fn close_connection(&self, id: &str) -> PanelResult<()> {
        if !self.config.clash_api_enabled {
            return Err(pp_common::PanelError::Client(
                "Clash API is disabled".into(),
            ));
        }
        crate::connections::clash_close_connection(
            self.config.clash_api_port,
            &self.config.clash_api_secret,
            id,
        )
        .await
    }
}
