//! Main tests module for core management.

use std::path::{Path, PathBuf};

use crate::cores::{ClientCoreInventory, version};

async fn spawn_server(app: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn write_executable(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// Construct tar.gz with several entries.
#[cfg(unix)]
fn build_tgz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let enc = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_path(name).unwrap();
            tar.append_data(&mut header, name, *data).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }
    out
}

// ---------- ① list_installed: scan directory structure ----------

#[test]
fn list_installed_scans_versioned_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());

    write_executable(&dir.path().join("cores/sing-box/1.13.15/sing-box"), b"fake");
    // Version directories without binary files should be skipped.
    std::fs::create_dir_all(dir.path().join("cores/sing-box/1.12.0")).unwrap();

    let cores = inv.list_installed();
    assert_eq!(cores.len(), 1);
    let sb = &cores[0];
    assert_eq!(sb.version, "1.13.15");
    assert_eq!(sb.path, dir.path().join("cores/sing-box/1.13.15/sing-box"));
}

/// 存量清理：构造 inventory 时 best-effort 删除遗留的 `cores/mihomo/` 目录（不影响 sing-box）。
#[test]
fn inventory_new_removes_legacy_mihomo_cores_dir() {
    let dir = tempfile::tempdir().unwrap();
    write_executable(&dir.path().join("cores/mihomo/1.19.29/mihomo"), b"fake");
    write_executable(&dir.path().join("cores/sing-box/1.13.15/sing-box"), b"fake");

    let inv = ClientCoreInventory::new(dir.path().to_path_buf());

    assert!(
        !dir.path().join("cores/mihomo").exists(),
        "遗留 mihomo 目录应被清理"
    );
    assert!(
        dir.path().join("cores/sing-box/1.13.15/sing-box").exists(),
        "sing-box 目录不受影响"
    );
    // 清理后扫描不到 mihomo。
    assert_eq!(inv.list_installed().len(), 1);
}

#[test]
fn list_downloaded_versions_sorts_semantically_descending() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());

    // Semantic descending: 1.14.0 > 1.14.0-beta.4 > 1.13.15.
    for v in ["1.13.15", "1.14.0-beta.4", "1.14.0"] {
        write_executable(
            &dir.path().join(format!("cores/sing-box/{v}/sing-box")),
            b"fake",
        );
    }
    // Version directories without binary files are not counted.
    std::fs::create_dir_all(dir.path().join("cores/sing-box/1.12.0")).unwrap();

    assert_eq!(
        inv.list_downloaded_versions(),
        vec!["1.14.0", "1.14.0-beta.4", "1.13.15"]
    );
}

// ---------- ② list_remote_channels: mock releases API ----------
#[tokio::test]
async fn list_remote_channels_picks_latest_per_channel() {
    // GitHub releases 按创建时间倒序（最新在前）；每个通道取首个命中。
    let singbox_releases = serde_json::json!([
        { "tag_name": "v1.14.0-alpha.2", "prerelease": true },
        { "tag_name": "v1.14.0-alpha.1", "prerelease": true },
        { "tag_name": "v1.14.0-beta.4", "prerelease": true },
        { "tag_name": "v1.14.0-rc.1", "prerelease": true },
        { "tag_name": "v1.13.15", "prerelease": false },
        { "tag_name": "v1.13.14", "prerelease": false },
    ]);
    let app = axum::Router::new().route(
        "/repos/SagerNet/sing-box/releases",
        axum::routing::get(move || async move { singbox_releases.to_string() }),
    );
    let base = spawn_server(app).await;
    let inv = ClientCoreInventory::with_api_base(PathBuf::new(), &base);

    let channels = inv.list_remote_channels().await.unwrap();
    let pairs: Vec<(crate::cores::CoreChannel, String)> = channels
        .into_iter()
        .map(|c| (c.channel, c.version))
        .collect();
    assert_eq!(
        pairs,
        vec![
            (
                crate::cores::CoreChannel::Prerelease,
                "1.14.0-alpha.2".to_string()
            ),
            (crate::cores::CoreChannel::Beta, "1.14.0-beta.4".to_string()),
            (crate::cores::CoreChannel::Stable, "1.13.15".to_string()),
        ]
    );
}

/// 通道分类：无后缀 = 稳定版；beta / rc = 测试版；alpha 及其它后缀 = 预发布版。
#[test]
fn channel_of_version_classifies_prerelease_markers() {
    use crate::cores::{CoreChannel, channel_of_version};
    assert_eq!(channel_of_version("1.13.15"), CoreChannel::Stable);
    assert_eq!(channel_of_version("v1.13.15"), CoreChannel::Stable);
    assert_eq!(channel_of_version("1.14.0-beta.4"), CoreChannel::Beta);
    assert_eq!(channel_of_version("1.14.0-rc.1"), CoreChannel::Beta);
    assert_eq!(
        channel_of_version("1.14.0-alpha.2"),
        CoreChannel::Prerelease
    );
    assert_eq!(
        channel_of_version("1.14.0-nightly.1"),
        CoreChannel::Prerelease
    );
}

// ---------- ③ download: mock asset download + extract + chmod + --version ----------
//
// Fake binary is a shell script, only runnable on Unix; mock release response uses axum
// `Host` extractor to fill back `browser_download_url`, avoiding base URL closure capture
// ordering issues.

#[cfg(unix)]
#[tokio::test]
async fn download_singbox_targz_extracts_and_verifies() {
    let (arch_hint, is_windows) = version::target_spec().unwrap();
    let ext = if is_windows { "zip" } else { "tar.gz" };
    let asset = format!("sing-box-1.13.15-{arch_hint}.{ext}");
    let fake: &[u8] = b"#!/bin/sh\necho 'sing-box version 1.13.15'\n";
    let body = build_tgz(&[("sing-box", fake)]);

    let asset_for_release = asset.clone();
    let app = axum::Router::new()
        .route(
            "/repos/SagerNet/sing-box/releases/tags/v1.13.15",
            axum::routing::get(move |headers: axum::http::HeaderMap| async move {
                let host = headers
                    .get("host")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("127.0.0.1");
                serde_json::json!({
                    "tag_name": "v1.13.15",
                    "assets": [
                        { "name": asset_for_release.clone(),
                          "browser_download_url":
                              format!("http://{host}/assets/{asset_for_release}") },
                    ],
                })
                .to_string()
            }),
        )
        .route(
            &format!("/assets/{asset}"),
            axum::routing::get(move || async move { body.clone() }),
        );
    let base = spawn_server(app).await;

    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::with_api_base(dir.path().to_path_buf(), &base);
    let core = inv.download("1.13.15").await.unwrap();

    assert_eq!(core.version, "1.13.15");
    assert_eq!(
        core.path,
        dir.path().join("cores/sing-box/1.13.15/sing-box")
    );
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&core.path).unwrap().permissions().mode();
        assert_ne!(mode & 0o111, 0, "binary should be executable");
    }
    // Second download hits cache.
    let again = inv.download("1.13.15").await.unwrap();
    assert_eq!(again.path, core.path);
}

// ---------- ⑤ Version probe: `version` / `--version` / `-v` three forms ----------
//
// Two fake binaries: one only supports `version` subcommand (sing-box 1.14+, multi-line output),
// one only supports `--version` flag (old sing-box), asserting both detection and parsing succeed.

#[test]
fn version_probe_supports_subcommand_and_flags() {
    let dir = tempfile::tempdir().unwrap();

    // sing-box 1.14+: only supports `version` subcommand, `--version` exits non-zero;
    // subcommand output contains multiple lines (version line + environment info).
    let subcmd = dir.path().join("sing-box-subcmd");
    write_executable(
        &subcmd,
        b"#!/bin/sh\n\
              [ \"$1\" = \"version\" ] || { echo 'Error: unknown flag: --version' >&2; exit 1; }\n\
              echo 'sing-box version 1.14.0-beta.4'\n\
              echo\n\
              echo 'Environment:'\n\
              echo '  go version go1.24.3'\n",
    );

    // Old sing-box: only supports `--version` flag.
    let flag = dir.path().join("sing-box-flag");
    write_executable(&flag, b"#!/bin/sh\necho 'sing-box version 1.13.15'\n");

    // After download both forms are detected successfully.
    version::verify_version(&subcmd, "1.14.0-beta.4").unwrap();
    version::verify_version(&flag, "1.13.15").unwrap();

    // 版本探测输出解析正确（同一解析曾服务于系统核心探测，已移除）。
    assert_eq!(
        version::parse_version_from_output(&version::binary_output(&subcmd)),
        Some("1.14.0-beta.4".to_string())
    );
    assert_eq!(
        version::parse_version_from_output(&version::binary_output(&flag)),
        Some("1.13.15".to_string())
    );
}

// ---------- ⑦ infer_core_type: file name inference ----------

#[test]
fn infers_core_type_from_file_name() {
    assert_eq!(
        version::infer_core_type(Path::new("/usr/local/bin/sing-box")),
        Some(pp_common::CoreType::SingBox)
    );
    assert_eq!(
        version::infer_core_type(Path::new("C:\\cores\\sing-box.exe")),
        Some(pp_common::CoreType::SingBox)
    );
    assert_eq!(
        version::infer_core_type(Path::new("/usr/local/bin/singbox")),
        Some(pp_common::CoreType::SingBox)
    );
    // mihomo / clash 文件名不再识别（mihomo 支持已移除）。
    assert_eq!(
        version::infer_core_type(Path::new("/usr/local/bin/mihomo")),
        None
    );
    assert_eq!(
        version::infer_core_type(Path::new("C:\\cores\\clash.exe")),
        None
    );
    assert_eq!(
        version::infer_core_type(Path::new("/usr/bin/unknown")),
        None
    );
}

// ---------- Auxiliary: version parsing ----------

#[test]
fn parses_version_from_output() {
    assert_eq!(
        version::parse_version_from_output("sing-box version 1.13.15"),
        Some("1.13.15".to_string())
    );
    assert_eq!(
        version::parse_version_from_output(
            "sing-box version 1.14.0-beta.4\n\nEnvironment:\n  go version go1.24.3"
        ),
        Some("1.14.0-beta.4".to_string())
    );
}

// ---------- ⑨ delete: local core deletion ----------

#[test]
fn delete_removes_version_dir_keeps_other_versions() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());

    let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");
    write_executable(&bin, b"fake");
    // Other version is kept; active points to it (not the deletion target).
    let other = dir.path().join("cores/sing-box/1.14.0/sing-box");
    write_executable(&other, b"fake");

    inv.delete(&bin, &other).unwrap();

    assert!(!bin.exists(), "binary should be deleted");
    assert!(
        !dir.path().join("cores/sing-box/1.13.15").exists(),
        "version directory should be removed"
    );
    // Type directory is kept (other versions exist), other version is unaffected.
    assert!(other.exists(), "other version should be kept");
    assert!(dir.path().join("cores/sing-box").is_dir());
}

#[test]
fn delete_prunes_empty_type_dir() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());
    let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");
    write_executable(&bin, b"fake");

    inv.delete(&bin, Path::new("/nonexistent/other")).unwrap();

    // Version directory and type directory are both cleaned up; cores directory itself is kept.
    assert!(!dir.path().join("cores/sing-box").exists());
    assert!(dir.path().join("cores").is_dir());
}

#[test]
fn delete_rejects_path_outside_cores_dir() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());
    // cores directory exists, but target is outside it (system core semantics).
    std::fs::create_dir_all(dir.path().join("cores")).unwrap();
    let system_bin = dir.path().join("bin/sing-box");
    write_executable(&system_bin, b"fake");

    let err = inv
        .delete(&system_bin, Path::new("/nonexistent/active"))
        .unwrap_err();
    assert!(
        err.to_string().contains("System core cannot be deleted"),
        "should reject system path: {err}"
    );
    assert!(system_bin.exists(), "system core should not be deleted");
}

#[test]
fn delete_rejects_active_binary() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());
    let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");
    write_executable(&bin, b"fake");

    let err = inv.delete(&bin, &bin).unwrap_err();
    assert!(
        err.to_string().contains("Active core cannot be deleted"),
        "should reject active core: {err}"
    );
    assert!(bin.exists(), "active core should not be deleted");
}

#[test]
fn delete_rejects_nonexistent_path() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());
    let missing = dir.path().join("cores/sing-box/9.9.9/sing-box");

    let err = inv
        .delete(&missing, Path::new("/nonexistent/active"))
        .unwrap_err();
    assert!(
        err.to_string().contains("does not exist"),
        "should report path does not exist: {err}"
    );
}

#[test]
fn delete_rejects_directory_under_cores_dir() {
    // Passing type directory/version directory itself (not binary file) should be rejected,
    // preventing accidental deletion of larger scope.
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());
    let version_dir = dir.path().join("cores/sing-box/1.13.15/sing-box");
    write_executable(&version_dir, b"fake");
    std::fs::create_dir_all(dir.path().join("cores/sing-box/1.12.0")).unwrap();

    // Type directory (no binary) cannot be deleted.
    let err = inv
        .delete(
            &dir.path().join("cores/sing-box"),
            Path::new("/nonexistent/active"),
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("Invalid core binary path"),
        "should reject directory: {err}"
    );
    assert!(dir.path().join("cores/sing-box").is_dir());

    // Version directory (no binary) cannot be deleted.
    let err = inv
        .delete(
            &dir.path().join("cores/sing-box/1.12.0"),
            Path::new("/nonexistent/active"),
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("Invalid core binary path"),
        "should reject directory: {err}"
    );
    assert!(dir.path().join("cores/sing-box/1.12.0").is_dir());
}
