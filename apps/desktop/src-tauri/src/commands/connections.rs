//! Connection commands: list active, list closed, close by ID.

use pp_client::ConnectionView;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

/// Active connections response with totals.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionsActiveView {
    pub connections: Vec<ConnectionView>,
    pub upload_total: u64,
    pub download_total: u64,
}

/// List all active connections from the background tracker.
///
/// Returns error when core is not running or Clash API is disabled / unreachable.
#[tauri::command]
pub async fn connections_active(state: State<'_, AppState>) -> Result<ConnectionsActiveView, String> {
    let lock = state.client.lock().await;
    let client = lock.as_ref().ok_or_else(|| "core not running".to_string())?;
    if !client.config.clash_api_enabled {
        return Err("Clash API is disabled".to_string());
    }
    let active = client
        .active_connections()
        .await
        .ok_or_else(|| "core not running".to_string())?;
    Ok(ConnectionsActiveView {
        connections: active.connections,
        upload_total: active.upload_total,
        download_total: active.download_total,
    })
}

/// List closed connections from the background tracker ring buffer.
///
/// Returns error when core is not running or Clash API is disabled.
#[tauri::command]
pub async fn connections_closed(state: State<'_, AppState>) -> Result<Vec<ConnectionView>, String> {
    let lock = state.client.lock().await;
    let client = lock.as_ref().ok_or_else(|| "core not running".to_string())?;
    if !client.config.clash_api_enabled {
        return Err("Clash API is disabled".to_string());
    }
    client
        .closed_connections()
        .await
        .ok_or_else(|| "core not running".to_string())
}

/// Close a single active connection by ID.
///
/// Returns error when core is not running or Clash API is disabled / unreachable.
#[tauri::command]
pub async fn connections_close(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let lock = state.client.lock().await;
    let client = lock.as_ref().ok_or_else(|| "core not running".to_string())?;
    if !client.config.clash_api_enabled {
        return Err("Clash API is disabled".to_string());
    }
    client.close_connection(&id).await.map_err(|e| e.to_string())
}
