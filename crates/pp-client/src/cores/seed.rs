//! Bundled seed core first-run release (ADR-0008 D5).
//!
//! Windows installers bundle a pinned sing-box core + `wintun.dll` under
//! `<resource_dir>/seed/` (produced by `apps/client/scripts/fetch-seed-core.ts`,
//! packed via `tauri.windows.conf.json` `bundle.resources`). On first run — when no
//! downloaded core exists yet — the seed is copied into the standard versioned
//! directory `data_dir/cores/sing-box/<version>/`, making the app usable offline
//! without depending on GitHub reachability.
//!
//! Seed semantics:
//!
//! - **补白不覆盖**：仅当无任何已下载核心时释放；用户已装核心（下载 / 系统）时跳过；
//! - 释放结果与运行时下载的核心**无差别**（`CoreSource::Downloaded`），后续升级仍走
//!   [`ClientCoreInventory::download`] 通道；
//! - 缺失 seed 资源（开发构建、Linux/macOS 包）→ `Ok(None)`，不是错误；
//! - 释放后版本探测失败（如杀软拦截）→ 清理半成品目录并 `Ok(None)`，回退运行时
//!   下载通道，不阻塞启动。

use std::path::Path;

use pp_common::{PanelError, PanelResult};
use serde::Deserialize;

use super::{ClientCoreInventory, CoreSource, LocalCore, download, version};

/// Runtime seed manifest (`<resource_dir>/seed/manifest.json`), written by
/// `fetch-seed-core.ts` from the pinned `src-tauri/seed-manifest.json`.
#[derive(Debug, Deserialize)]
struct SeedManifest {
    sing_box_version: String,
}

/// Seed the bundled core from installer resources on first run.
///
/// `resource_dir` is Tauri's resource directory (`app.path().resource_dir()`);
/// the seed lives under its `seed/` subdirectory. Returns the seeded core, or
/// `None` when seeding did not apply (no seed resources / cores already present /
/// probe failure after cleanup).
pub fn seed_bundled_core(data_dir: &Path, resource_dir: &Path) -> PanelResult<Option<LocalCore>> {
    let seed_dir = resource_dir.join("seed");
    let manifest_path = seed_dir.join("manifest.json");
    let binary = seed_dir.join(version::binary_name_on_disk());
    if !manifest_path.is_file() || !binary.is_file() {
        // 非种子构建（dev / Linux / macOS 包）：静默跳过。
        return Ok(None);
    }
    let manifest: SeedManifest = serde_json::from_slice(&std::fs::read(&manifest_path)?)
        .map_err(|e| PanelError::Core(format!("Invalid seed manifest: {e}")))?;
    let version_str = manifest.sing_box_version.trim().to_string();
    if version_str.is_empty() {
        return Err(PanelError::Core(
            "Invalid seed manifest: empty sing_box_version".to_string(),
        ));
    }

    let inventory = ClientCoreInventory::new(data_dir.to_path_buf());
    if !inventory.list_installed().is_empty() {
        tracing::debug!("已存在本地核心，跳过种子释放");
        return Ok(None);
    }

    let dir = inventory.core_dir(&version_str);
    let dest = dir.join(version::binary_name_on_disk());
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(&binary, &dest)?;
    download::set_executable(&dest)?;
    // wintun.dll 仅 Windows 种子携带；缺失不阻塞（TUN 启动时报错信息足够明确，
    // 且运行时 ensure_wintun 在核心下载时仍会补齐）。
    let wintun = seed_dir.join("wintun.dll");
    if wintun.is_file() {
        std::fs::copy(&wintun, dir.join("wintun.dll"))?;
    }

    // 版本探测复核：副本与清单版本不一致（打包错误）或被环境拦截时，清理半成品
    // 并回退运行时下载通道，不阻塞启动。
    if let Err(e) = version::verify_version(&dest, &version_str) {
        tracing::warn!(
            error = %e,
            "种子核心版本探测失败，清理并回退运行时下载"
        );
        let _ = std::fs::remove_dir_all(&dir);
        return Ok(None);
    }

    tracing::info!(
        version = %version_str,
        path = %dest.display(),
        "已从安装包释放种子核心"
    );
    Ok(Some(LocalCore {
        version: version_str,
        path: dest,
        source: CoreSource::Downloaded,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a fake sing-box core whose `version` probe prints the given version.
    #[cfg(not(windows))]
    fn write_fake_core(path: &Path, version: &str) {
        std::fs::write(
            path,
            format!("#!/bin/sh\necho \"sing-box version {version}\"\n"),
        )
        .unwrap();
    }

    #[cfg(windows)]
    fn write_fake_core(path: &Path, version: &str) {
        // cmd batch probe: `version` / `--version` / `-v` all echo the version line.
        std::fs::write(path, format!("@echo sing-box version {version}\n")).unwrap();
    }

    fn write_seed(resource_dir: &Path, version: &str) {
        let seed = resource_dir.join("seed");
        std::fs::create_dir_all(&seed).unwrap();
        write_fake_core(&seed.join(version::binary_name_on_disk()), version);
        std::fs::write(seed.join("wintun.dll"), b"fake-dll").unwrap();
        std::fs::write(
            seed.join("manifest.json"),
            format!("{{\"sing_box_version\": \"{version}\", \"arch\": \"amd64\"}}"),
        )
        .unwrap();
    }

    // 版本探测需真实执行核心二进制：Windows 上 fake core 不可执行，仅在非
    // Windows 宿主验证成功路径（失败路径经 `cleans_up_and_skips_on_probe_failure`
    // 全平台覆盖）。
    #[cfg(not(windows))]
    #[test]
    fn seeds_core_on_first_run() {
        let data = tempfile::tempdir().unwrap();
        let res = tempfile::tempdir().unwrap();
        write_seed(res.path(), "1.14.2");

        let core = seed_bundled_core(data.path(), res.path())
            .unwrap()
            .expect("seed should apply");
        assert_eq!(core.version, "1.14.2");
        assert_eq!(core.source, CoreSource::Downloaded);
        assert!(core.path.is_file());
        assert!(
            core.path
                .starts_with(data.path().join("cores").join("sing-box").join("1.14.2"))
        );
        // wintun.dll 随核心同目录释放。
        assert!(core.path.parent().unwrap().join("wintun.dll").is_file());

        // 幂等：已装核心时再次调用跳过。
        assert!(
            seed_bundled_core(data.path(), res.path())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn skips_without_seed_resources() {
        let data = tempfile::tempdir().unwrap();
        let res = tempfile::tempdir().unwrap();
        assert!(
            seed_bundled_core(data.path(), res.path())
                .unwrap()
                .is_none()
        );
        assert!(!data.path().join("cores").exists());
    }

    #[test]
    fn skips_when_core_already_installed() {
        let data = tempfile::tempdir().unwrap();
        let res = tempfile::tempdir().unwrap();
        write_seed(res.path(), "1.14.2");
        // 预置一个已装核心。
        let existing = data.path().join("cores").join("sing-box").join("1.15.0");
        std::fs::create_dir_all(&existing).unwrap();
        write_fake_core(&existing.join(version::binary_name_on_disk()), "1.15.0");

        assert!(
            seed_bundled_core(data.path(), res.path())
                .unwrap()
                .is_none()
        );
        // 种子版本目录未被创建。
        assert!(!data.path().join("cores/sing-box/1.14.2").exists());
    }

    #[test]
    fn cleans_up_and_skips_on_probe_failure() {
        let data = tempfile::tempdir().unwrap();
        let res = tempfile::tempdir().unwrap();
        // 清单声明 1.14.2，但种子二进制报告 1.13.0（模拟打包错误）。
        write_seed(res.path(), "1.13.0");
        let seed = res.path().join("seed");
        std::fs::write(
            seed.join("manifest.json"),
            "{\"sing_box_version\": \"1.14.2\", \"arch\": \"amd64\"}",
        )
        .unwrap();

        assert!(
            seed_bundled_core(data.path(), res.path())
                .unwrap()
                .is_none()
        );
        assert!(!data.path().join("cores/sing-box/1.14.2").exists());
    }

    #[test]
    fn errors_on_empty_manifest_version() {
        let data = tempfile::tempdir().unwrap();
        let res = tempfile::tempdir().unwrap();
        let seed = res.path().join("seed");
        std::fs::create_dir_all(&seed).unwrap();
        write_fake_core(&seed.join(version::binary_name_on_disk()), "1.14.2");
        std::fs::write(
            seed.join("manifest.json"),
            "{\"sing_box_version\": \"\", \"arch\": \"amd64\"}",
        )
        .unwrap();

        assert!(seed_bundled_core(data.path(), res.path()).is_err());
    }
}
