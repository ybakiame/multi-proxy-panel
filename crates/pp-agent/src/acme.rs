//! Built-in ACME (Let's Encrypt) client.
//!
//! Issues and renews certificates into a unified `<data_dir>/certs/`
//! directory that every core's generated config can reference. HTTP-01 is
//! served by a temporary port-80 listener that only lives for the duration
//! of the challenge; the proxy cores on 443 are untouched.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt,
    NewAccount, NewOrder, OrderStatus, RetryPolicy,
};
use pp_common::CoreType;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;

const ACCOUNT_FILE: &str = "acme-account.json";
const CERT_DIR: &str = "certs";
const RENEW_AFTER_SECS: u64 = 60 * 24 * 3600;
const RENEW_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 3600);

pub fn cert_file_paths(data_dir: &Path, domain: &str) -> (PathBuf, PathBuf) {
    let dir = data_dir.join(CERT_DIR);
    (
        dir.join(format!("{domain}.crt")),
        dir.join(format!("{domain}.key")),
    )
}

fn cert_meta_path(data_dir: &Path, domain: &str) -> PathBuf {
    data_dir.join(CERT_DIR).join(format!("{domain}.meta.json"))
}

async fn set_private(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(path).await?.permissions();
        perms.set_mode(0o600);
        tokio::fs::set_permissions(path, perms).await?;
    }
    Ok(())
}

async fn load_or_create_account(data_dir: &Path) -> anyhow::Result<Account> {
    let path = data_dir.join(ACCOUNT_FILE);
    if let Ok(raw) = tokio::fs::read(&path).await {
        match serde_json::from_slice::<AccountCredentials>(&raw) {
            Ok(credentials) => {
                return Ok(Account::builder()?.from_credentials(credentials).await?);
            }
            Err(e) => tracing::warn!("corrupt acme account file, recreating: {}", e),
        }
    }

    let (account, credentials) = Account::builder()?
        .create(
            &NewAccount {
                contact: &[],
                terms_of_service_agreed: true,
                only_return_existing: false,
            },
            LetsEncrypt::Production.url().to_owned(),
            None,
        )
        .await?;
    tokio::fs::write(&path, serde_json::to_vec_pretty(&credentials)?).await?;
    set_private(&path).await?;
    tracing::info!("created new ACME account");
    Ok(account)
}

/// Issue or renew a certificate for `domain` via HTTP-01.
/// Persists `<data_dir>/certs/<domain>.{crt,key}` and the hub cert_id
/// mapping, returning the expiry unix timestamp.
pub async fn issue_certificate(
    data_dir: &Path,
    cert_id: &str,
    domain: &str,
) -> anyhow::Result<i64> {
    let account = load_or_create_account(data_dir).await?;

    let identifiers = vec![Identifier::Dns(domain.to_string())];
    let mut order = account.new_order(&NewOrder::new(&identifiers)).await?;

    let store = ChallengeStore::default();
    let server = Http01Server::start(store.clone()).await;

    let result: anyhow::Result<()> = (async {
        let mut authorizations = order.authorizations();
        while let Some(entry) = authorizations.next().await {
            let mut authz = entry?;
            match authz.status {
                AuthorizationStatus::Pending => {}
                AuthorizationStatus::Valid => continue,
                other => anyhow::bail!("unexpected authorization status: {:?}", other),
            }

            let mut challenge = authz
                .challenge(ChallengeType::Http01)
                .ok_or_else(|| anyhow::anyhow!("no http-01 challenge offered"))?;
            let key_auth = challenge.key_authorization().as_str().to_string();
            store.insert(challenge.token.clone(), key_auth).await;
            challenge.set_ready().await?;
        }

        let status = order.poll_ready(&RetryPolicy::default()).await?;
        if status != OrderStatus::Ready {
            anyhow::bail!("order not ready: {:?}", status);
        }

        let private_key_pem = order.finalize().await?;
        let cert_chain_pem = order.poll_certificate(&RetryPolicy::default()).await?;

        let (crt_path, key_path) = cert_file_paths(data_dir, domain);
        if let Some(dir) = crt_path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(&crt_path, &cert_chain_pem).await?;
        tokio::fs::write(&key_path, &private_key_pem).await?;
        set_private(&key_path).await?;

        let meta = serde_json::json!({ "cert_id": cert_id, "domain": domain });
        tokio::fs::write(cert_meta_path(data_dir, domain), meta.to_string()).await?;
        Ok(())
    })
    .await;

    if let Ok(server) = server {
        server.stop().await;
    }
    result.as_ref().map_err(|e| anyhow::anyhow!("{}", e))?;

    let expires_at = chrono::Utc::now().timestamp() + 90 * 24 * 3600;
    tracing::info!("issued certificate for {}", domain);
    Ok(expires_at)
}

/// Certificates older than this are re-issued by the renewal loop.
async fn due_cert_domains(data_dir: &Path) -> Vec<(String, String)> {
    let mut due = Vec::new();
    let cert_dir = data_dir.join(CERT_DIR);
    let Ok(mut entries) = tokio::fs::read_dir(&cert_dir).await else {
        return due;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(domain) = name.strip_suffix(".meta.json") else {
            continue;
        };
        let Ok(raw) = tokio::fs::read(entry.path()).await else {
            continue;
        };
        let Ok(meta) = serde_json::from_slice::<serde_json::Value>(&raw) else {
            continue;
        };
        let Some(cert_id) = meta.get("cert_id").and_then(|v| v.as_str()) else {
            continue;
        };

        let (crt_path, _) = cert_file_paths(data_dir, domain);
        let age_secs = tokio::fs::metadata(&crt_path)
            .await
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|e| e.as_secs())
            .unwrap_or(u64::MAX);
        if age_secs >= RENEW_AFTER_SECS {
            due.push((cert_id.to_string(), domain.to_string()));
        }
    }
    due
}

/// Periodic renewal: re-issue certs older than RENEW_AFTER_SECS, restart
/// non-mihomo cores whose config references the cert (mihomo hot-reloads
/// cert files on its own), and report status through `report`.
pub async fn run_renewal_loop(
    data_dir: PathBuf,
    supervisor: Arc<pp_core::CoreSupervisor>,
    report: tokio::sync::mpsc::Sender<pp_proto::AgentMessage>,
) {
    let mut ticker = tokio::time::interval(RENEW_CHECK_INTERVAL);
    loop {
        ticker.tick().await;
        for (cert_id, domain) in due_cert_domains(&data_dir).await {
            tracing::info!("renewing certificate for {}", domain);
            match issue_certificate(&data_dir, &cert_id, &domain).await {
                Ok(expires_at) => {
                    restart_cores_using(&supervisor, &data_dir, &domain).await;
                    send_cert_status(&report, &cert_id, "active", expires_at, "").await;
                }
                Err(e) => {
                    tracing::warn!("renewal for {} failed: {}", domain, e);
                    send_cert_status(&report, &cert_id, "failed", 0, &e.to_string()).await;
                }
            }
        }
    }
}

async fn restart_cores_using(supervisor: &pp_core::CoreSupervisor, data_dir: &Path, domain: &str) {
    let needle = format!("{}/{}/{}", CERT_DIR, domain, domain);
    for (core_type, snapshot) in crate::persist::load_last_configs(data_dir).await {
        if core_type == CoreType::Mihomo {
            continue;
        }
        let serialized = serde_json::to_string(&snapshot.config).unwrap_or_default();
        if !serialized.contains(&needle) {
            continue;
        }
        if let Some(manager) = supervisor.get(core_type).await {
            match manager.restart(&snapshot.config).await {
                Ok(()) => tracing::info!("restarted {:?} after cert renewal", core_type),
                Err(e) => tracing::warn!("failed to restart {:?} after renewal: {}", core_type, e),
            }
        }
    }
}

/// Report a certificate status to the Hub.
pub async fn send_cert_status(
    report: &tokio::sync::mpsc::Sender<pp_proto::AgentMessage>,
    cert_id: &str,
    status: &str,
    expires_at: i64,
    error: &str,
) {
    let msg = pp_proto::AgentMessage {
        payload: Some(pp_proto::agent_message::Payload::CertStatus(
            pp_proto::CertStatusReport {
                cert_id: cert_id.to_string(),
                status: status.to_string(),
                expires_at,
                error: error.to_string(),
            },
        )),
    };
    if report.send(msg).await.is_err() {
        tracing::warn!("failed to report cert status for {}", cert_id);
    }
}

#[derive(Clone, Default)]
struct ChallengeStore {
    tokens: Arc<RwLock<HashMap<String, String>>>,
}

impl ChallengeStore {
    async fn insert(&self, token: String, key_auth: String) {
        self.tokens.write().await.insert(token, key_auth);
    }

    async fn lookup(&self, token: &str) -> Option<String> {
        self.tokens.read().await.get(token).cloned()
    }
}

struct Http01Server {
    handle: tokio::task::JoinHandle<()>,
}

impl Http01Server {
    async fn start(store: ChallengeStore) -> anyhow::Result<Self> {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:80").await?;
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let store = store.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let Ok(n) = socket.read(&mut buf).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buf[..n]);
                    let token = request
                        .split_whitespace()
                        .nth(1)
                        .and_then(|p| p.strip_prefix("/.well-known/acme-challenge/"))
                        .map(|s| s.to_string());
                    let Some(token) = token else {
                        return;
                    };
                    let Some(body) = store.lookup(&token).await else {
                        return;
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        Ok(Self { handle })
    }

    async fn stop(self) {
        self.handle.abort();
    }
}
