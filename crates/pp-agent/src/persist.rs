//! Persistence of the last applied Hub config, one snapshot per core.
//!
//! The agent snapshots every successfully applied `ConfigPush` to
//! `data_dir/last_config.<core>.json`. On startup all snapshots are used to
//! bring the cores back up with their previous config, so every core on a
//! node survives agent/host reboots without waiting for a new Hub push.

use std::path::{Path, PathBuf};

use pp_common::CoreType;
use serde::{Deserialize, Serialize};

const LEGACY_SNAPSHOT_FILE: &str = "last_config.json";
const ALL_CORES: [CoreType; 2] = [CoreType::SingBox, CoreType::Mihomo];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedConfig {
    pub core_type: String,
    pub config: serde_json::Value,
    /// Hub-assigned config version (content hash); empty for snapshots
    /// written before versioning was introduced.
    #[serde(default)]
    pub version: String,
}

fn snapshot_path(data_dir: &Path, core_type: CoreType) -> PathBuf {
    data_dir.join(format!("last_config.{}.json", core_type))
}

/// Persist the last successfully applied config (atomic write, 0600 on unix).
pub async fn save_last_config(
    data_dir: &Path,
    core_type: CoreType,
    config: &serde_json::Value,
    version: &str,
) -> anyhow::Result<()> {
    let snapshot = PersistedConfig {
        core_type: core_type.to_string(),
        config: config.clone(),
        version: version.to_string(),
    };
    let data = serde_json::to_vec_pretty(&snapshot)?;

    let path = snapshot_path(data_dir, core_type);
    let tmp_path = path.with_extension("tmp");
    tokio::fs::write(&tmp_path, &data).await?;
    tokio::fs::rename(&tmp_path, &path).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&path).await?.permissions();
        perms.set_mode(0o600);
        tokio::fs::set_permissions(&path, perms).await?;
    }

    Ok(())
}

async fn load_one(path: &Path) -> Option<PersistedConfig> {
    if !path.exists() {
        return None;
    }

    let raw = match tokio::fs::read(path).await {
        Ok(raw) => raw,
        Err(e) => {
            tracing::warn!("failed to read {}: {}", path.display(), e);
            return None;
        }
    };

    match serde_json::from_slice::<PersistedConfig>(&raw) {
        Ok(snapshot) => Some(snapshot),
        Err(e) => {
            tracing::warn!("ignoring corrupt config snapshot {}: {}", path.display(), e);
            None
        }
    }
}

/// Move a legacy single-slot `last_config.json` (written by older agents)
/// to its per-core path so nothing is lost across the upgrade.
async fn migrate_legacy_snapshot(data_dir: &Path) {
    let legacy = data_dir.join(LEGACY_SNAPSHOT_FILE);
    let Some(snapshot) = load_one(&legacy).await else {
        return;
    };
    let Ok(core_type) = snapshot.core_type.parse::<CoreType>() else {
        tracing::warn!("legacy config snapshot has unknown core_type, dropping");
        let _ = tokio::fs::remove_file(&legacy).await;
        return;
    };

    let target = snapshot_path(data_dir, core_type);
    if target.exists() {
        let _ = tokio::fs::remove_file(&legacy).await;
    } else if let Err(e) = tokio::fs::rename(&legacy, &target).await {
        tracing::warn!("failed to migrate legacy config snapshot: {}", e);
    } else {
        tracing::info!("migrated legacy config snapshot for {:?}", core_type);
    }
}

/// Load all per-core config snapshots. Corrupt or unreadable snapshots are
/// logged and skipped.
pub async fn load_last_configs(data_dir: &Path) -> Vec<(CoreType, PersistedConfig)> {
    migrate_legacy_snapshot(data_dir).await;

    let mut snapshots = Vec::new();
    for core_type in ALL_CORES {
        if let Some(snapshot) = load_one(&snapshot_path(data_dir, core_type)).await {
            snapshots.push((core_type, snapshot));
        }
    }
    snapshots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("pp-agent-persist-{}", pp_common::generate_uuid()))
    }

    #[tokio::test]
    async fn save_and_load_roundtrip_per_core() {
        let dir = temp_dir();
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let singbox_cfg = serde_json::json!({"inbounds": []});
        let mihomo_cfg = serde_json::json!({"listeners": []});
        save_last_config(&dir, CoreType::SingBox, &singbox_cfg, "v1")
            .await
            .unwrap();
        save_last_config(&dir, CoreType::Mihomo, &mihomo_cfg, "v2")
            .await
            .unwrap();

        let loaded = load_last_configs(&dir).await;
        assert_eq!(loaded.len(), 2);
        let by_core: std::collections::HashMap<_, _> = loaded.into_iter().collect();
        assert_eq!(by_core[&CoreType::SingBox].config, singbox_cfg);
        assert_eq!(by_core[&CoreType::SingBox].version, "v1");
        assert_eq!(by_core[&CoreType::Mihomo].config, mihomo_cfg);
        assert_eq!(by_core[&CoreType::Mihomo].version, "v2");

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn load_missing_returns_empty() {
        let dir = temp_dir();
        assert!(load_last_configs(&dir).await.is_empty());
    }

    #[tokio::test]
    async fn load_corrupt_is_skipped() {
        let dir = temp_dir();
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(snapshot_path(&dir, CoreType::SingBox), b"not json")
            .await
            .unwrap();

        assert!(load_last_configs(&dir).await.is_empty());

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn legacy_snapshot_is_migrated() {
        let dir = temp_dir();
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let legacy = serde_json::json!({"core_type": "sing-box", "config": {"inbounds": [1]}});
        tokio::fs::write(
            dir.join(LEGACY_SNAPSHOT_FILE),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .await
        .unwrap();

        let loaded = load_last_configs(&dir).await;
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, CoreType::SingBox);
        assert_eq!(loaded[0].1.version, "");
        assert!(!dir.join(LEGACY_SNAPSHOT_FILE).exists());
        assert!(snapshot_path(&dir, CoreType::SingBox).exists());

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }
}
