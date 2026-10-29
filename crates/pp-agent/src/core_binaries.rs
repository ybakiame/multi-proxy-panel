use pp_core::CoreSupervisor;

pub(crate) async fn list_core_binaries(supervisor: &CoreSupervisor) -> pp_proto::CoreBinaryList {
    let Some(bin_dir) = supervisor.bin_dir().await else {
        return pp_proto::CoreBinaryList {
            binaries: Vec::new(),
            error: "core binary directory is not initialized".to_string(),
        };
    };

    let mut in_use = std::collections::HashSet::new();
    for core in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
        if supervisor.get(core).await.is_some() {
            in_use.insert(pp_core::core_binary_path(&bin_dir, core));
        }
    }

    let mut binaries = Vec::new();
    let mut entries = match tokio::fs::read_dir(&bin_dir).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("failed to read bin dir {}: {}", bin_dir.display(), e);
            return pp_proto::CoreBinaryList {
                binaries,
                error: format!("failed to read {}: {}", bin_dir.display(), e),
            };
        }
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        binaries.push(pp_proto::CoreBinary {
            file_name: name,
            size_bytes: meta.len() as i64,
            modified_at,
            in_use: in_use.contains(&entry.path()),
        });
    }
    binaries.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    pp_proto::CoreBinaryList {
        binaries,
        error: String::new(),
    }
}

/// Delete a core binary, refusing path traversal and binaries used by a registered core.
pub(crate) async fn delete_core_binary(
    supervisor: &CoreSupervisor,
    file_name: &str,
) -> Result<(), String> {
    if file_name.is_empty()
        || file_name.starts_with('.')
        || file_name.contains('/')
        || file_name.contains('\\')
    {
        return Err("invalid file name".to_string());
    }
    let Some(bin_dir) = supervisor.bin_dir().await else {
        return Err("bin dir unknown".to_string());
    };
    let path = bin_dir.join(file_name);

    for core in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
        if supervisor.get(core).await.is_some() && pp_core::core_binary_path(&bin_dir, core) == path
        {
            return Err(format!("{} is in use by a running core", file_name));
        }
    }

    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| format!("failed to delete {}: {}", file_name, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn distinguishes_missing_empty_and_populated_dirs() {
        let root =
            std::env::temp_dir().join(format!("pp-core-list-{}", pp_common::generate_uuid()));
        let supervisor = CoreSupervisor::new();
        supervisor.discover(&root, &root).await.unwrap();

        let missing = list_core_binaries(&supervisor).await;
        assert!(missing.binaries.is_empty());
        assert!(!missing.error.is_empty());

        tokio::fs::create_dir(&root).await.unwrap();
        let empty = list_core_binaries(&supervisor).await;
        assert!(empty.binaries.is_empty());
        assert!(empty.error.is_empty());

        tokio::fs::write(root.join("sing-box"), b"test")
            .await
            .unwrap();
        let populated = list_core_binaries(&supervisor).await;
        assert_eq!(populated.binaries.len(), 1);
        assert_eq!(populated.binaries[0].file_name, "sing-box");
        assert!(populated.error.is_empty());

        tokio::fs::remove_dir_all(&root).await.unwrap();
    }
}
