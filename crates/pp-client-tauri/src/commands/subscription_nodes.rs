//! Static subscription node list command.
//!
//! Reads node tags from a subscription's local content cache
//! (`data_dir/subscription_cache/<id>.json`) — no running core / network needed.
//! This is the static counterpart to [`crate::commands::proxies_list`] (Clash
//! API, only available while the core runs).

use pp_client::NodeTagView;
use tauri::State;
use uuid::Uuid;

use crate::commands::parse_subscription_id;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Pure command body (unit-testable without a Tauri runtime)
// ---------------------------------------------------------------------------

/// Read a subscription's cached node tags; missing / corrupted cache yields an
/// empty list (see [`pp_client::cached_node_tags`]).
pub(crate) fn subscription_node_tags_impl(
    data_dir: &std::path::Path,
    id: Uuid,
) -> Result<Vec<NodeTagView>, String> {
    pp_client::cached_node_tags(data_dir, id).map_err(|e| format!("读取订阅节点缓存失败: {e}"))
}

// ---------------------------------------------------------------------------
// Tauri command
// ---------------------------------------------------------------------------

/// List a subscription's node tags from the local cache.
///
/// Static source: does not need a running core or network. Tags match the
/// runtime Clash API proxy names (same rules as config injection).
#[tauri::command]
pub async fn subscription_node_tags(
    state: State<'_, AppState>,
    subscription_id: String,
) -> Result<Vec<NodeTagView>, String> {
    let id = parse_subscription_id(&subscription_id)?;
    subscription_node_tags_impl(&state.data_dir, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_client::{CachedSubscriptionContent, SubscriptionStore};

    #[test]
    fn subscription_node_tags_impl_reads_cache_and_tolerates_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("sub", "https://example.com/sub", true, None)
            .unwrap();
        store
            .write_cached_content(
                sub.id,
                &CachedSubscriptionContent {
                    format: pp_client::SubFormat::SingBoxJson,
                    singbox_nodes: vec![
                        serde_json::json!({ "tag": "n1", "type": "vless" }),
                        serde_json::json!({ "tag": "n2", "type": "shadowsocks" }),
                    ],
                },
            )
            .unwrap();

        let tags = subscription_node_tags_impl(dir.path(), sub.id).unwrap();
        assert_eq!(
            tags.iter().map(|t| t.tag.as_str()).collect::<Vec<_>>(),
            vec!["n1", "n2"]
        );

        // No cache for an unknown id → empty list, no error.
        let missing = subscription_node_tags_impl(dir.path(), Uuid::new_v4()).unwrap();
        assert!(missing.is_empty());
    }
}
