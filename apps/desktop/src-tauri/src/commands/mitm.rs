//! MITM commands: traffic recording and CA certificate.

use pp_mitm::{CaStore, TrafficRecorder};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;
#[cfg(target_os = "android")]
use super::require_desktop;

/// External view of a traffic record.
#[derive(Debug, Clone, Serialize)]
pub struct TrafficRecordView {
    pub id: String,
    pub method: String,
    pub url: String,
    pub request_headers: Vec<(String, String)>,
    pub request_body: Option<String>,
    pub response_status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_body: Option<String>,
    pub timestamp: String,
    pub duration_ms: u64,
}

impl TrafficRecordView {
    pub(crate) fn from_record(rec: &pp_mitm::TrafficRecord) -> Self {
        Self {
            id: rec.id.to_string(),
            method: rec.method.clone(),
            url: rec.url.clone(),
            request_headers: rec.request_headers.clone(),
            request_body: rec.request_body.clone(),
            response_status: rec.response_status,
            response_headers: rec.response_headers.clone(),
            response_body: rec.response_body.clone(),
            timestamp: rec.timestamp.to_rfc3339(),
            duration_ms: rec.duration_ms,
        }
    }
}

/// List MITM traffic records.
#[tauri::command]
pub async fn list_traffic(state: State<'_, AppState>) -> Result<Vec<TrafficRecordView>, String> {
    #[cfg(target_os = "android")]
    {
        let _ = state;
        return require_desktop("MITM traffic recording");
    }
    #[cfg(not(target_os = "android"))]
    {
        let lock = state.client.lock().await;
        let Some(client) = lock.as_ref() else {
            return Ok(Vec::new());
        };
        let records = client.recorder().list();
        Ok(records.iter().map(TrafficRecordView::from_record).collect())
    }
}

/// External view of MITM CA certificate.
#[derive(Debug, Clone, Serialize)]
pub struct MitmCaView {
    /// Absolute path to `ca.crt` (for system/browser trust import).
    pub path: String,
    /// PEM-encoded root certificate content.
    pub pem: String,
}

/// Get MITM CA certificate (`data_dir/certs/ca.{crt,key}`).
#[tauri::command]
pub fn get_mitm_ca(state: State<'_, AppState>) -> Result<MitmCaView, String> {
    #[cfg(target_os = "android")]
    {
        let _ = state;
        return require_desktop("MITM CA certificate");
    }
    #[cfg(not(target_os = "android"))]
    {
        get_mitm_ca_impl(&state.data_dir)
    }
}

/// Implementation of `get_mitm_ca` (testable pure logic).
pub(crate) fn get_mitm_ca_impl(data_dir: &std::path::Path) -> Result<MitmCaView, String> {
    let store = pp_mitm::FileCaStore::new(data_dir.join("certs"));
    let material = store
        .load_or_generate()
        .map_err(|e| format!("读取 MITM CA 失败: {e}"))?;
    Ok(MitmCaView {
        path: data_dir
            .join("certs")
            .join("ca.crt")
            .to_string_lossy()
            .into_owned(),
        pem: material.cert_pem,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(std::path::PathBuf);

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

    #[test]
    fn get_mitm_ca_generates_and_reports_path() {
        let dir = TestDir::new();
        let view = get_mitm_ca_impl(dir.path()).unwrap();
        assert!(view.pem.contains("BEGIN CERTIFICATE"), "pem should contain cert block: {}", view.pem);
        assert!(view.path.ends_with("ca.crt"), "path should end with ca.crt: {}", view.path);
        assert!(std::path::Path::new(&view.path).is_file(), "CA cert should be on disk: {}", view.path);

        let again = get_mitm_ca_impl(dir.path()).unwrap();
        assert_eq!(view.pem, again.pem, "idempotent: should not regenerate");
    }
}
