//! MITM commands: traffic recording and CA certificate.

use pp_mitm::{CaStore, CaTrustView, TrafficRecorder};
use serde::Serialize;
use tauri::State;

use crate::desktop::state::AppState;

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
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(Vec::new());
    };
    let records = client.recorder().list();
    Ok(records.iter().map(TrafficRecordView::from_record).collect())
}

/// 派生白名单条目视图。
#[derive(Debug, Clone, Serialize)]
pub struct DerivedHostnameView {
    pub hostname: String,
    pub sources: Vec<String>,
}

/// MITM 白名单视图：脚本/重写派生（只读）+ 手动配置（高级补充）。
#[derive(Debug, Clone, Serialize)]
pub struct MitmWhitelistView {
    /// 来自远程 Snippet / 本地导入的派生域名（含来源标注）。
    pub derived: Vec<DerivedHostnameView>,
    /// 手动配置的包含项。
    pub manual: Vec<String>,
    /// 手动配置的排除项（保留 `-` / `!` 前缀原样）。
    pub excluded: Vec<String>,
}

/// 获取 MITM 白名单视图：派生部分读取远程缓存（与实际生效范围一致），
/// 手动部分来自客户端配置。客户端未初始化时返回空视图。
#[tauri::command]
pub async fn get_mitm_whitelist(state: State<'_, AppState>) -> Result<MitmWhitelistView, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(MitmWhitelistView {
            derived: Vec::new(),
            manual: Vec::new(),
            excluded: Vec::new(),
        });
    };
    let (excluded, manual): (Vec<String>, Vec<String>) = client
        .config
        .mitm
        .hostnames
        .iter()
        .cloned()
        .partition(|h| h.starts_with('-') || h.starts_with('!'));
    let derived = pp_client::remote::collect_hostname_sources(&client.config.data_dir)
        .into_iter()
        .map(|d| DerivedHostnameView {
            hostname: d.hostname,
            sources: d.sources,
        })
        .collect();
    Ok(MitmWhitelistView {
        derived,
        manual,
        excluded,
    })
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
    get_mitm_ca_impl(&state.data_dir)
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

/// 确保 CA 已生成并返回 `ca.crt` 绝对路径（install / status 命令共用）。
fn ensure_ca_path(data_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let view = get_mitm_ca_impl(data_dir)?;
    Ok(std::path::PathBuf::from(view.path))
}

/// Install MITM CA into the system trust store.
///
/// Windows 写 CurrentUser Root 存储（免 UAC）；macOS / Linux 弹系统授权框。
/// 返回安装后的信任状态（供前端直接回写缓存）。
#[tauri::command]
pub fn install_mitm_ca(state: State<'_, AppState>) -> Result<CaTrustView, String> {
    let ca_path = ensure_ca_path(&state.data_dir)?;
    pp_mitm::install_ca(&ca_path).map_err(|e| format!("安装 CA 到系统信任库失败: {e}"))?;
    Ok(pp_mitm::ca_trust_status(&ca_path))
}

/// Detect whether the MITM CA is trusted by the system trust store.
///
/// 检测失败返回 status=unknown 而非报错。
#[tauri::command]
pub fn mitm_ca_trust_status(state: State<'_, AppState>) -> Result<CaTrustView, String> {
    let ca_path = ensure_ca_path(&state.data_dir)?;
    Ok(pp_mitm::ca_trust_status(&ca_path))
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
        assert!(
            view.pem.contains("BEGIN CERTIFICATE"),
            "pem should contain cert block: {}",
            view.pem
        );
        assert!(
            view.path.ends_with("ca.crt"),
            "path should end with ca.crt: {}",
            view.path
        );
        assert!(
            std::path::Path::new(&view.path).is_file(),
            "CA cert should be on disk: {}",
            view.path
        );

        let again = get_mitm_ca_impl(dir.path()).unwrap();
        assert_eq!(view.pem, again.pem, "idempotent: should not regenerate");
    }
}
