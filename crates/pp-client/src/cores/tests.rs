//! Main tests module for core management.

use std::path::{Path, PathBuf};

use pp_common::CoreType;

use crate::config::ClientConfig;
use crate::cores::{ClientCoreInventory, CoreSource, version};

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

/// Global PATH lock: environment variables are process-level state, parallel tests must be
/// mutually exclusive to avoid interfering with each other.
static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Execute closure under specified PATH (mutually exclusive serialization, single-threaded
/// modification/restoration of environment variables within test).
fn with_patched_path<T>(path: &Path, f: impl FnOnce() -> T) -> T {
    let _guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old = std::env::var_os("PATH");
    // Under Rust 2024 std::env's set_var/remove_var is marked unsafe (concurrent modification
    // of environment variables is undefined behavior); PATH_LOCK guarantees serialized access
    // within the test process.
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

/// Construct tar.gz with several entries.
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

/// Gzip compress single file (mihomo non-Windows asset form).
fn gzip_bytes(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut out = Vec::new();
    let mut enc = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
    enc.write_all(data).unwrap();
    enc.finish().unwrap();
    out
}

// ---------- ① list_installed: scan directory structure ----------

#[test]
fn list_installed_scans_versioned_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());

    write_executable(&dir.path().join("cores/sing-box/1.13.15/sing-box"), b"fake");
    write_executable(&dir.path().join("cores/mihomo/1.19.29/mihomo"), b"fake");
    // Version directories without binary files should be skipped.
    std::fs::create_dir_all(dir.path().join("cores/sing-box/1.12.0")).unwrap();

    let cores = inv.list_installed();
    assert_eq!(cores.len(), 2);
    let sb = cores
        .iter()
        .find(|c| c.core_type == CoreType::SingBox)
        .unwrap();
    assert_eq!(sb.version, "1.13.15");
    assert_eq!(sb.source, CoreSource::Downloaded);
    assert_eq!(sb.path, dir.path().join("cores/sing-box/1.13.15/sing-box"));
    let mh = cores
        .iter()
        .find(|c| c.core_type == CoreType::Mihomo)
        .unwrap();
    assert_eq!(mh.version, "1.19.29");
    assert_eq!(mh.source, CoreSource::Downloaded);
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
    // Other core types are not counted.
    write_executable(&dir.path().join("cores/mihomo/1.19.29/mihomo"), b"fake");

    assert_eq!(
        inv.list_downloaded_versions(CoreType::SingBox),
        vec!["1.14.0", "1.14.0-beta.4", "1.13.15"]
    );
    assert_eq!(
        inv.list_downloaded_versions(CoreType::Mihomo),
        vec!["1.19.29"]
    );
    assert!(
        inv.list_downloaded_versions(CoreType::SingBox)
            .into_iter()
            .all(|v| !v.starts_with("1.12"))
    );
}

// ---------- ② list_remote_versions: mock releases API ----------
#[tokio::test]
async fn list_remote_versions_parses_both_cores() {
    let singbox_releases = serde_json::json!([
        { "tag_name": "v1.13.15" },
        { "tag_name": "v1.13.14" },
        { "tag_name": "v1.12.0-alpha.1" },
    ]);
    let mihomo_releases = serde_json::json!([
        { "tag_name": "v1.19.29" },
        { "tag_name": "Alpha-1.19.30" },
        { "tag_name": "v1.19.28" },
    ]);
    let app = axum::Router::new()
        .route(
            "/repos/SagerNet/sing-box/releases",
            axum::routing::get(move || async move { singbox_releases.to_string() }),
        )
        .route(
            "/repos/MetaCubeX/mihomo/releases",
            axum::routing::get(move || async move { mihomo_releases.to_string() }),
        );
    let base = spawn_server(app).await;
    let inv = ClientCoreInventory::with_api_base(PathBuf::new(), &base);

    let sb = inv.list_remote_versions(CoreType::SingBox).await.unwrap();
    assert_eq!(sb, vec!["1.13.15", "1.13.14", "1.12.0-alpha.1"]);

    let mh = inv.list_remote_versions(CoreType::Mihomo).await.unwrap();
    assert_eq!(mh, vec!["1.19.29", "Alpha-1.19.30", "1.19.28"]);
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
    let core = inv.download(CoreType::SingBox, "1.13.15").await.unwrap();

    assert_eq!(core.core_type, CoreType::SingBox);
    assert_eq!(core.version, "1.13.15");
    assert_eq!(core.source, CoreSource::Downloaded);
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
    let again = inv.download(CoreType::SingBox, "1.13.15").await.unwrap();
    assert_eq!(again.path, core.path);
}

#[cfg(unix)]
#[tokio::test]
async fn download_mihomo_single_gz_extracts_and_verifies() {
    let (arch_hint, is_windows) = version::target_spec().unwrap();
    let ext = if is_windows { "zip" } else { "gz" };
    let asset = format!("mihomo-{arch_hint}-v1.19.29.{ext}");
    let fake: &[u8] = b"#!/bin/sh\necho 'Mihomo Meta v1.19.29'\n";
    let body = gzip_bytes(fake);

    let asset_for_release = asset.clone();
    let app = axum::Router::new()
        .route(
            "/repos/MetaCubeX/mihomo/releases/tags/v1.19.29",
            axum::routing::get(move |headers: axum::http::HeaderMap| async move {
                let host = headers
                    .get("host")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("127.0.0.1");
                serde_json::json!({
                    "tag_name": "v1.19.29",
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
    let core = inv.download(CoreType::Mihomo, "1.19.29").await.unwrap();

    assert_eq!(core.version, "1.19.29");
    assert_eq!(core.path, dir.path().join("cores/mihomo/1.19.29/mihomo"));
    assert!(core.path.is_file());
}

// ---------- ④ detect_system_cores: fake binary in temp directory added to PATH ----------

#[test]
fn detect_system_cores_finds_core_on_path() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("sing-box");
    write_executable(&bin, b"#!/bin/sh\necho 'sing-box version 1.19.9'\n");

    // PATH is modified/restored by with_patched_path with lock, avoiding race with parallel tests.
    let result = with_patched_path(dir.path(), || {
        let inv = ClientCoreInventory::new(PathBuf::new());
        inv.detect_system_cores()
    });

    let found = result
        .iter()
        .find(|c| c.core_type == CoreType::SingBox && c.path == bin);
    assert!(found.is_some(), "should find fake sing-box in PATH");
    assert_eq!(found.unwrap().version, "1.19.9");
    assert_eq!(found.unwrap().source, CoreSource::System);
}

// ---------- ⑤ Version probe: `version` / `--version` / `-v` three forms ----------
//
// Three fake binaries each only support `version` subcommand (sing-box 1.14+, multi-line output),
// `--version` flag (old sing-box), `-v` (mihomo), asserting both detection and parsing succeed.

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

    // mihomo: only supports `-v`.
    let mihomo = dir.path().join("mihomo-v");
    write_executable(
        &mihomo,
        b"#!/bin/sh\n\
              [ \"$1\" = \"-v\" ] || exit 1\n\
              echo 'Mihomo Meta v1.19.29 linux/amd64 go1.23.4'\n",
    );

    // After download all three forms are detected successfully.
    version::verify_version(&subcmd, CoreType::SingBox, "1.14.0-beta.4").unwrap();
    version::verify_version(&flag, CoreType::SingBox, "1.13.15").unwrap();
    version::verify_version(&mihomo, CoreType::Mihomo, "1.19.29").unwrap();

    // detect_system_cores path: output parsing is correct.
    assert_eq!(
        version::parse_version_from_output(CoreType::SingBox, &version::binary_output(&subcmd)),
        Some("1.14.0-beta.4".to_string())
    );
    assert_eq!(
        version::parse_version_from_output(CoreType::SingBox, &version::binary_output(&flag)),
        Some("1.13.15".to_string())
    );
    assert_eq!(
        version::parse_version_from_output(CoreType::Mihomo, &version::binary_output(&mihomo)),
        Some("1.19.29".to_string())
    );
}

// ---------- ⑥ active_core: match by config.core_binary ----------

#[test]
fn active_core_matches_config_binary() {
    let dir = tempfile::tempdir().unwrap();
    write_executable(
        &dir.path().join("cores/sing-box/1.13.15/sing-box"),
        b"#!/bin/sh\necho 'sing-box version 1.13.15'\n",
    );
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());
    let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");

    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        "http://127.0.0.1:50052",
        "tok",
        CoreType::SingBox,
        bin.clone(),
    );
    let active = inv.active_core(&cfg);
    assert!(active.is_some());
    assert_eq!(active.unwrap().path, bin);

    // Non-matching path → None.
    cfg.core_binary = PathBuf::from("/nonexistent/sing-box");
    assert!(inv.active_core(&cfg).is_none());

    // Empty path → None.
    cfg.core_binary = PathBuf::new();
    assert!(inv.active_core(&cfg).is_none());
}

// ---------- ⑦ infer_core_type: file name inference ----------

#[test]
fn infers_core_type_from_file_name() {
    assert_eq!(
        version::infer_core_type(Path::new("/usr/local/bin/sing-box")),
        Some(CoreType::SingBox)
    );
    assert_eq!(
        version::infer_core_type(Path::new("C:\\cores\\sing-box.exe")),
        Some(CoreType::SingBox)
    );
    assert_eq!(
        version::infer_core_type(Path::new("/usr/local/bin/singbox")),
        Some(CoreType::SingBox)
    );
    assert_eq!(
        version::infer_core_type(Path::new("/usr/local/bin/mihomo")),
        Some(CoreType::Mihomo)
    );
    assert_eq!(
        version::infer_core_type(Path::new("C:\\cores\\clash.exe")),
        Some(CoreType::Mihomo)
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
        version::parse_version_from_output(CoreType::SingBox, "sing-box version 1.13.15"),
        Some("1.13.15".to_string())
    );
    assert_eq!(
        version::parse_version_from_output(
            CoreType::SingBox,
            "sing-box version 1.14.0-beta.4\n\nEnvironment:\n  go version go1.24.3"
        ),
        Some("1.14.0-beta.4".to_string())
    );
    assert_eq!(
        version::parse_version_from_output(
            CoreType::Mihomo,
            "Mihomo Meta v1.19.29 linux/amd64 go1.23.4"
        ),
        Some("1.19.29".to_string())
    );
}

// ---------- ⑧ preferred_binary: downloaded version sorting + system fallback ----------

#[test]
fn preferred_binary_picks_newest_downloaded_version() {
    let dir = tempfile::tempdir().unwrap();
    write_executable(
        &dir.path().join("cores/sing-box/1.13.15/sing-box"),
        b"#!/bin/sh\necho 'sing-box version 1.13.15'\n",
    );
    write_executable(
        &dir.path().join("cores/sing-box/1.14.0-beta.4/sing-box"),
        b"#!/bin/sh\necho 'sing-box version 1.14.0-beta.4'\n",
    );
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());

    let bin = inv.preferred_binary(CoreType::SingBox);
    // Semantic version sorting: 1.14.0-beta.4 (base 1.14.0) > 1.13.15.
    assert_eq!(
        bin,
        Some(dir.path().join("cores/sing-box/1.14.0-beta.4/sing-box"))
    );
}

#[test]
fn preferred_binary_falls_back_to_system_core() {
    let dir = tempfile::tempdir().unwrap();
    // No downloaded cores → fallback to system PATH detection.
    let system_bin = dir.path().join("mihomo");
    write_executable(&system_bin, b"#!/bin/sh\necho 'Mihomo Meta v1.19.29'\n");
    let inv = ClientCoreInventory::new(dir.path().join("cores"));

    let result = with_patched_path(dir.path(), || inv.preferred_binary(CoreType::Mihomo));
    assert_eq!(result, Some(system_bin));
}

#[test]
fn preferred_binary_none_when_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());
    let result = with_patched_path(Path::new("/nonexistent-bin-dir"), || {
        inv.preferred_binary(CoreType::SingBox)
    });
    assert_eq!(result, None);
}

// ---------- ⑨ delete: local core deletion ----------

#[test]
fn delete_removes_version_dir_keeps_other_versions() {
    let dir = tempfile::tempdir().unwrap();
    let inv = ClientCoreInventory::new(dir.path().to_path_buf());

    let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");
    write_executable(&bin, b"fake");
    // Other version of same type is kept; active points to it (not the deletion target).
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
    let bin = dir.path().join("cores/mihomo/1.19.29/mihomo");
    write_executable(&bin, b"fake");

    inv.delete(&bin, Path::new("/nonexistent/other")).unwrap();

    // Version directory and type directory are both cleaned up; cores directory itself is kept.
    assert!(!dir.path().join("cores/mihomo").exists());
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
    std::fs::create_dir_all(dir.path().join("cores/mihomo/1.19.29")).unwrap();

    // Type directory (no binary) cannot be deleted.
    let err = inv
        .delete(
            &dir.path().join("cores/mihomo"),
            Path::new("/nonexistent/active"),
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("Invalid core binary path"),
        "should reject directory: {err}"
    );
    assert!(dir.path().join("cores/mihomo").is_dir());

    // Version directory (no binary) cannot be deleted.
    let err = inv
        .delete(
            &dir.path().join("cores/mihomo/1.19.29"),
            Path::new("/nonexistent/active"),
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("Invalid core binary path"),
        "should reject directory: {err}"
    );
    assert!(dir.path().join("cores/mihomo/1.19.29").is_dir());
}
