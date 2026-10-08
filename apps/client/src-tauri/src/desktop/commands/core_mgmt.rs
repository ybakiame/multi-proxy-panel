//! Core management commands: download, list, delete, select local core binaries.
//!
//! The client only supports the sing-box core; commands carry no core type parameter.

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::desktop::state::AppState;

/// External view of a local core.
#[derive(Debug, Clone, Serialize)]
pub struct LocalCoreView {
    pub version: String,
    pub path: String,
    pub active: bool,
}

impl LocalCoreView {
    pub(crate) fn from_core(core: &pp_client::LocalCore, active_binary: &std::path::Path) -> Self {
        Self {
            version: core.version.clone(),
            path: core.path.to_string_lossy().into_owned(),
            active: core.path == active_binary,
        }
    }
}

/// Current active core binary path from config (empty if config not saved).
pub(crate) fn active_binary(data_dir: &std::path::Path) -> std::path::PathBuf {
    pp_client::ClientConfig::load(data_dir)
        .map(|c| c.core_binary)
        .unwrap_or_default()
}

/// List local available cores (downloaded / installer-seeded, with active flag).
#[tauri::command]
pub async fn list_cores(state: State<'_, AppState>) -> Result<Vec<LocalCoreView>, String> {
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let active = active_binary(&state.data_dir);
    Ok(inv
        .list_installed()
        .iter()
        .map(|c| LocalCoreView::from_core(c, &active))
        .collect())
}

/// External view of a remote channel's latest version.
#[derive(Debug, Clone, Serialize)]
pub struct RemoteChannelView {
    /// `stable` / `beta` / `prerelease`.
    pub channel: String,
    pub version: String,
}

/// List the latest remote version per release channel (稳定版 / 测试版 / 预发布版,
/// `v` prefix stripped; channel model references `.reference/GUI.for.SingBox`).
#[tauri::command]
pub async fn list_remote_core_channels(
    state: State<'_, AppState>,
) -> Result<Vec<RemoteChannelView>, String> {
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let channels = inv
        .list_remote_channels()
        .await
        .map_err(|e| format!("拉取远端版本失败: {e}"))?;
    Ok(channels
        .iter()
        .map(|c| RemoteChannelView {
            channel: c.channel.as_str().to_string(),
            version: c.version.clone(),
        })
        .collect())
}

/// List downloaded versions (semantic version descending).
#[tauri::command]
pub async fn list_downloaded_versions(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    Ok(inv.list_downloaded_versions())
}

/// Auto-select downloaded core as the active binary (sing-box only; downloads take
/// effect immediately).
pub(crate) fn auto_select_downloaded_core(data_dir: &std::path::Path, core_path: &std::path::Path) {
    let Ok(mut config) = pp_client::ClientConfig::load(data_dir) else {
        return;
    };
    config.core_binary = core_path.to_path_buf();
    if let Err(e) = config.save() {
        tracing::warn!("保存自动选中核心配置失败: {e}");
    }
}

/// Download a specific core version and return its view.
#[tauri::command(rename_all = "snake_case")]
pub async fn download_core(
    state: State<'_, AppState>,
    version: String,
) -> Result<LocalCoreView, String> {
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let core = inv
        .download(&version)
        .await
        .map_err(|e| format!("下载核心失败: {e}"))?;
    auto_select_downloaded_core(&state.data_dir, &core.path);
    let active = active_binary(&state.data_dir);
    Ok(LocalCoreView::from_core(&core, &active))
}

/// Set a path as the active core binary.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_active_core(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let bin = PathBuf::from(&path);
    if !bin.is_file() {
        return Err(format!("核心二进制不存在: {path}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&bin).map_err(|e| format!("读取核心信息失败: {e}"))?;
        if meta.permissions().mode() & 0o111 == 0 {
            return Err(format!("核心二进制不可执行: {path}"));
        }
    }
    let mut config = match pp_client::ClientConfig::load(&state.data_dir) {
        Ok(cfg) => cfg,
        Err(_) => pp_client::ClientConfig::new(
            state.data_dir.clone(),
            String::new(),
            String::new(),
            PathBuf::new(),
        ),
    };
    config.core_binary = bin;
    config.save().map_err(|e| format!("保存配置失败: {e}"))
}

/// Delete core implementation (testable pure logic).
pub(crate) fn delete_core_impl(data_dir: &std::path::Path, path: &str) -> Result<(), String> {
    let bin = PathBuf::from(path);
    let inv = pp_client::ClientCoreInventory::new(data_dir.to_path_buf());
    let active = active_binary(data_dir);
    if bin == active {
        return Err("正在使用的核心不可删除：请先切换其他核心".to_string());
    }
    // cores 目录外的路径（含曾经意义上的「系统核心」）由 pp-client 的越界校验拒绝。
    inv.delete(&bin, &active)
        .map_err(|e| format!("删除核心失败: {e}"))
}

/// Delete a downloaded core (currently active core cannot be deleted).
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_core(state: State<'_, AppState>, path: String) -> Result<(), String> {
    delete_core_impl(&state.data_dir, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_client::ClientConfig;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pp-client-ui-core-mgmt-test-{}-{}",
                std::process::id(),
                n
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_empty_path<T>(f: impl FnOnce() -> T) -> T {
        let _guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", "/nonexistent-pp-test-bin");
        }
        let result = f();
        match old {
            Some(v) => unsafe { std::env::set_var("PATH", v) },
            None => unsafe { std::env::remove_var("PATH") },
        }
        result
    }

    fn write_core(data_dir: &std::path::Path, core_dir: &str, version: &str) {
        let bin = data_dir
            .join("cores")
            .join(core_dir)
            .join(version)
            .join(core_dir);
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, b"fake core").unwrap();
    }

    #[test]
    fn list_downloaded_versions_lists_semantic_descending() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        write_core(dir.path(), "sing-box", "1.14.0-beta.4");
        write_core(dir.path(), "sing-box", "1.14.0");

        let inv = pp_client::ClientCoreInventory::new(dir.path().to_path_buf());
        let versions = inv.list_downloaded_versions();
        assert_eq!(versions, vec!["1.14.0", "1.14.0-beta.4", "1.13.15"]);
    }

    #[test]
    fn auto_select_downloaded_core_updates_core_binary() {
        let dir = TestDir::new();
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        let downloaded = dir.path().join("cores/sing-box/1.14.0/sing-box");
        auto_select_downloaded_core(dir.path(), &downloaded);

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_binary, downloaded);
    }

    #[test]
    fn auto_select_downloaded_core_without_config_does_not_create_one() {
        let dir = TestDir::new();
        auto_select_downloaded_core(
            dir.path(),
            &dir.path().join("cores/sing-box/1.14.0/sing-box"),
        );
        assert!(!dir.path().join("client.json").exists());
    }

    #[test]
    fn delete_core_deletes_downloaded_core_and_clears_version_dir() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        write_core(dir.path(), "sing-box", "1.14.0");
        let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");

        with_empty_path(|| delete_core_impl(dir.path(), &bin.to_string_lossy()).unwrap());

        assert!(!bin.exists());
        assert!(!dir.path().join("cores/sing-box/1.13.15").exists());
        assert!(dir.path().join("cores/sing-box/1.14.0/sing-box").exists());
    }

    #[test]
    fn delete_core_rejects_active_binary() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            bin.clone(),
        );
        cfg.save().unwrap();

        let err =
            with_empty_path(|| delete_core_impl(dir.path(), &bin.to_string_lossy()).unwrap_err());
        assert!(err.contains("正在使用的核心不可删除"), "{err}");
        assert!(bin.exists());
    }

    #[test]
    fn delete_core_rejects_nonexistent_path() {
        let dir = TestDir::new();
        // pp-client's delete first canonicalizes the cores download dir; create one
        // core so the dir exists and the missing binary is what gets rejected.
        write_core(dir.path(), "sing-box", "1.13.15");
        let missing = dir.path().join("cores/sing-box/9.9.9/sing-box");
        let err = with_empty_path(|| {
            delete_core_impl(dir.path(), &missing.to_string_lossy()).unwrap_err()
        });
        assert!(err.contains("does not exist"), "{err}");
    }
}
