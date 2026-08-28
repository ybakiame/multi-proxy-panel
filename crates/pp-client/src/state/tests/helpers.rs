//! Shared test helpers for state tests.

#![allow(dead_code)]

pub use std::net::SocketAddr;
pub use std::os::unix::fs::PermissionsExt;
pub use std::path::{Path, PathBuf};
pub use std::sync::Arc;
pub use std::time::Duration;

pub use axum::http::StatusCode;
pub use base64::Engine as _;
pub use pp_common::CoreType;
pub use tempfile::TempDir;

pub use crate::config::ClientConfig;
pub use crate::remote::{RemoteKind, RemoteResource};
pub use crate::state::ClientState;
pub use crate::subscription;
pub use crate::sysproxy::{MockSystemProxy, SysProxyCall};

/// Notifier that records notifications (verify injection chain: ClientState → ScriptHost → `$notify`).
#[derive(Debug, Default)]
pub struct RecordingNotifier {
    calls: std::sync::Mutex<Vec<(String, String, String)>>,
}

impl RecordingNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl pp_script::Notifier for RecordingNotifier {
    fn notify(&self, title: &str, subtitle: &str, body: &str, _options: Option<serde_json::Value>) {
        self.calls.lock().unwrap().push((
            title.to_string(),
            subtitle.to_string(),
            body.to_string(),
        ));
    }
}

/// Start a local axum server that returns `(status, body)` for all paths (for testing, no external requests).
pub async fn spawn_server(status: StatusCode, body: &'static str) -> SocketAddr {
    let app = axum::Router::new().fallback(move || async move { (status, body) });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

/// Write a fake core script that "ignores arguments and keeps running".
pub fn fake_core_script(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("fake-core.sh");
    std::fs::write(&path, "#!/bin/sh\nsleep 5\n").unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    path
}

/// Write a fake core script that copies the config file corresponding to `-c <config>` argument to `capture`
/// (used to assert the composed config actually received by core, verifying MITM starts before core).
pub fn fake_core_capturing_args(dir: &TempDir, capture: &Path) -> PathBuf {
    let path = dir.path().join("fake-core-capture.sh");
    let script = format!(
        "#!/bin/sh\n\
             prev=\"\"\n\
             for arg in \"$@\"; do\n\
               if [ \"$prev\" = \"-c\" ]; then\n\
                 cp \"$arg\" {}\n\
               fi\n\
               prev=\"$arg\"\n\
             done\n\
             sleep 5\n",
        capture.display()
    );
    std::fs::write(&path, script).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    path
}

pub fn test_config(dir: &TempDir, hub_url: String) -> ClientConfig {
    let cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        hub_url,
        "tok",
        CoreType::SingBox,
        fake_core_script(dir),
    );
    // start now always reloads config from disk: test config must first write client.json to disk.
    cfg.save().unwrap();
    cfg
}
