//! Core management commands: download, list, delete, select local core binaries.

use std::path::PathBuf;

use pp_common::CoreType;
use serde::Serialize;
use tauri::State;

use crate::commands::core_type_from_str;
use crate::state::AppState;

/// External view of a local core.
#[derive(Debug, Clone, Serialize)]
pub struct LocalCoreView {
    pub core_type: String,
    pub version: String,
    pub path: String,
    pub source: String,
    pub active: bool,
}

impl LocalCoreView {
    pub(crate) fn from_core(core: &pp_client::LocalCore, active_binary: &std::path::Path) -> Self {
        Self {
            core_type: crate::commands::core_type_str(core.core_type),
            version: core.version.clone(),
            path: core.path.to_string_lossy().into_owned(),
            source: match core.source {
                pp_client::CoreSource::Downloaded => "downloaded",
                pp_client::CoreSource::System => "system",
            }
            .to_string(),
            active: core.path == active_binary,
        }
    }
}

/// Merge installed and system-detected cores (dedup by path, system first).
pub(crate) fn merge_cores(
    mut installed: Vec<pp_client::LocalCore>,
    system: Vec<pp_client::LocalCore>,
) -> Vec<pp_client::LocalCore> {
    for s in system {
        if !installed.iter().any(|c| c.path == s.path) {
            installed.push(s);
        }
    }
    installed
}

/// Current active core binary path from config (empty if config not saved).
pub(crate) fn active_binary(data_dir: &std::path::Path) -> std::path::PathBuf {
    pp_client::ClientConfig::load(data_dir)
        .map(|c| c.core_binary)
        .unwrap_or_default()
}

/// List local available cores (installed + system-detected, with active flag).
#[tauri::command]
pub async fn list_cores(state: State<'_, AppState>) -> Result<Vec<LocalCoreView>, String> {
    #[cfg(target_os = "android")]
    {
        let _ = state;
        return require_desktop("core management");
    }
    #[cfg(not(target_os = "android"))]
    {
        let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
        let cores = merge_cores(inv.list_installed(), inv.detect_system_cores());
        let active = active_binary(&state.data_dir);
        Ok(cores
            .iter()
            .map(|c| LocalCoreView::from_core(c, &active))
            .collect())
    }
}

/// List recent 10 remote releases (GitHub releases, `v` prefix stripped).
#[tauri::command(rename_all = "snake_case")]
pub async fn list_remote_core_versions(
    state: State<'_, AppState>,
    core_type: String,
) -> Result<Vec<String>, String> {
    #[cfg(target_os = "android")]
    {
        let _ = (state, core_type);
        return require_desktop("core version listing");
    }
    #[cfg(not(target_os = "android"))]
    {
        let ct = core_type_from_str(&core_type)?;
        let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
        inv.list_remote_versions(ct)
            .await
            .map_err(|e| format!("拉取远端版本失败: {e}"))
    }
}

/// List downloaded versions for a core type (semantic version descending).
#[tauri::command(rename_all = "snake_case")]
pub async fn list_downloaded_versions(
    state: State<'_, AppState>,
    core_type: String,
) -> Result<Vec<String>, String> {
    let ct = core_type_from_str(&core_type)?;
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    Ok(inv.list_downloaded_versions(ct))
}

/// Auto-select downloaded core when type matches current config (desktop only).
#[cfg(not(target_os = "android"))]
pub(crate) fn auto_select_downloaded_core(
    data_dir: &std::path::Path,
    core_type: CoreType,
    core_path: &std::path::Path,
) {
    let Ok(mut config) = pp_client::ClientConfig::load(data_dir) else {
        return;
    };
    if config.core_type != core_type {
        return;
    }
    config.core_binary = core_path.to_path_buf();
    if let Err(e) = config.save() {
        tracing::warn!("保存自动选中核心配置失败: {e}");
    }
}

/// Download a specific core version and return its view.
#[tauri::command(rename_all = "snake_case")]
pub async fn download_core(
    state: State<'_, AppState>,
    core_type: String,
    version: String,
) -> Result<LocalCoreView, String> {
    #[cfg(target_os = "android")]
    {
        let _ = (state, core_type, version);
        return require_desktop("core download");
    }
    #[cfg(not(target_os = "android"))]
    {
        let ct = core_type_from_str(&core_type)?;
        let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
        let core = inv
            .download(ct, &version)
            .await
            .map_err(|e| format!("下载核心失败: {e}"))?;
        auto_select_downloaded_core(&state.data_dir, ct, &core.path);
        let active = active_binary(&state.data_dir);
        Ok(LocalCoreView::from_core(&core, &active))
    }
}

/// Set a path as the active core binary.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_active_core(state: State<'_, AppState>, path: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let _ = (state, path);
        return require_desktop("core selection");
    }
    #[cfg(not(target_os = "android"))]
    {
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
        let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
        let core_type = merge_cores(inv.list_installed(), inv.detect_system_cores())
            .into_iter()
            .find(|c| c.path == bin)
            .map(|c| c.core_type)
            .or_else(|| pp_client::infer_core_type(&bin))
            .ok_or_else(|| format!("无法识别核心类型: {path}，请在设置页手动选择核心类型"))?;
        let mut config = match pp_client::ClientConfig::load(&state.data_dir) {
            Ok(cfg) => cfg,
            Err(_) => pp_client::ClientConfig::new(
                state.data_dir.clone(),
                String::new(),
                String::new(),
                CoreType::SingBox,
                PathBuf::new(),
            ),
        };
        config.core_binary = bin;
        config.core_type = core_type;
        config.save().map_err(|e| format!("保存配置失败: {e}"))
    }
}

/// Refresh system core detection.
#[tauri::command]
pub async fn detect_system_cores(state: State<'_, AppState>) -> Result<Vec<LocalCoreView>, String> {
    #[cfg(target_os = "android")]
    {
        let _ = state;
        return require_desktop("system core detection");
    }
    #[cfg(not(target_os = "android"))]
    {
        let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
        let active = active_binary(&state.data_dir);
        Ok(inv
            .detect_system_cores()
            .iter()
            .map(|c| LocalCoreView::from_core(c, &active))
            .collect())
    }
}

/// Delete core implementation (testable pure logic).
pub(crate) fn delete_core_impl(data_dir: &std::path::Path, path: &str) -> Result<(), String> {
    let bin = PathBuf::from(path);
    let inv = pp_client::ClientCoreInventory::new(data_dir.to_path_buf());
    let matched = merge_cores(inv.list_installed(), inv.detect_system_cores())
        .into_iter()
        .find(|c| c.path == bin);
    if matched.is_some_and(|c| c.source == pp_client::CoreSource::System) {
        return Err("系统核心不可删除：仅支持删除已下载的核心".to_string());
    }
    let active = active_binary(data_dir);
    if bin == active {
        return Err("正在使用的核心不可删除：请先切换其他核心".to_string());
    }
    inv.delete(&bin, &active)
        .map_err(|e| format!("删除核心失败: {e}"))
}

/// Delete a downloaded core (system source / currently active cores cannot be deleted).
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_core(state: State<'_, AppState>, path: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let _ = (state, path);
        return require_desktop("core deletion");
    }
    #[cfg(not(target_os = "android"))]
    {
        delete_core_impl(&state.data_dir, &path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pp-client-ui-test-{}-{}",
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
        write_core(dir.path(), "mihomo", "1.19.29");

        let inv = pp_client::ClientCoreInventory::new(dir.path().to_path_buf());
        let versions = inv.list_downloaded_versions(pp_common::CoreType::SingBox);
        assert_eq!(versions, vec!["1.14.0", "1.14.0-beta.4", "1.13.15"]);

        let mihomo = inv.list_downloaded_versions(pp_common::CoreType::Mihomo);
        assert_eq!(mihomo, vec!["1.19.29"]);
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn auto_select_downloaded_core_matching_type_updates_core_binary() {
        let dir = TestDir::new();
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        let downloaded = dir.path().join("cores/sing-box/1.14.0/sing-box");
        auto_select_downloaded_core(dir.path(), CoreType::SingBox, &downloaded);

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_type, CoreType::SingBox);
        assert_eq!(saved.core_binary, downloaded);
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn auto_select_downloaded_core_mismatched_type_keeps_binary_untouched() {
        let dir = TestDir::new();
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        let downloaded = dir.path().join("cores/mihomo/1.19.29/mihomo");
        auto_select_downloaded_core(dir.path(), CoreType::Mihomo, &downloaded);

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_binary, dir.path().join("cores/sing-box/1.13.15/sing-box"));
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn auto_select_downloaded_core_without_config_does_not_create_one() {
        let dir = TestDir::new();
        auto_select_downloaded_core(
            dir.path(),
            CoreType::SingBox,
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
            CoreType::SingBox,
            bin.clone(),
        );
        cfg.save().unwrap();

        let err = with_empty_path(|| delete_core_impl(dir.path(), &bin.to_string_lossy()).unwrap_err());
        assert!(err.contains("正在使用的核心不可删除"), "{err}");
        assert!(bin.exists());
    }

    #[test]
    fn delete_core_rejects_system_source() {
        let dir = TestDir::new();
        let system_bin = dir.path().join("bin/sing-box");
        std::fs::create_dir_all(system_bin.parent().unwrap()).unwrap();
        std::fs::write(&system_bin, b"#!/bin/sh\necho 'sing-box version 1.19.9'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&system_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        fn with_patched_path<T>(path: &std::path::Path, f: impl FnOnce() -> T) -> T {
            let _guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let old = std::env::var_os("PATH");
            unsafe {
                std::env::set_var("PATH", path);
            }
            let result = f();
            match old {
                Some(v) => unsafe { std::env::set_var("PATH", v) },
                None => unsafe { std::env::remove_var("PATH") },
            }
            result
        }

        let err = with_patched_path(&dir.path().join("bin"), || {
            delete_core_impl(dir.path(), &system_bin.to_string_lossy())
        })
        .unwrap_err();
        assert!(err.contains("系统核心不可删除"), "{err}");
        assert!(system_bin.exists());
    }

    #[test]
    fn delete_core_rejects_nonexistent_path() {
        let dir = TestDir::new();
        let missing = dir.path().join("cores/sing-box/9.9.9/sing-box");
        let err = with_empty_path(|| {
            delete_core_impl(dir.path(), &missing.to_string_lossy()).unwrap_err()
        });
        assert!(err.contains("不存在"), "{err}");
    }
}
