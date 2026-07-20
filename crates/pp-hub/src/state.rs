use chrono::{DateTime, FixedOffset};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::{HubConfig, rate_limiter::RateLimiter, routes::metrics_export::MetricsHandle};

/// TTL for cached API key authentication entries.
const API_KEY_CACHE_TTL: Duration = Duration::from_secs(3600);

/// Cached result of a successful API key verification.
#[derive(Clone)]
pub struct CachedApiKey {
    pub key_id: Uuid,
    pub name: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<FixedOffset>>,
    pub ip_allowlist: Option<Value>,
    pub rate_limit: Option<i32>,
    pub cached_at: Instant,
}

impl CachedApiKey {
    pub fn is_expired(&self) -> bool {
        Instant::now().duration_since(self.cached_at) > API_KEY_CACHE_TTL
    }
}

/// In-memory cache for verified API keys to avoid expensive Argon2 verification
/// on every request.
#[derive(Clone, Default)]
pub struct ApiKeyCache {
    entries: Arc<std::sync::Mutex<HashMap<String, CachedApiKey>>>,
}

impl ApiKeyCache {
    pub fn get(&self, cache_key: &str) -> Option<CachedApiKey> {
        let entries = self.entries.lock().unwrap();
        entries.get(cache_key).cloned().filter(|e| !e.is_expired())
    }

    pub fn insert(&self, cache_key: String, entry: CachedApiKey) {
        let mut entries = self.entries.lock().unwrap();
        entries.insert(cache_key, entry);
    }

    pub fn invalidate(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
    }

    /// Compute a cache key from the raw API key string using SHA-256.
    pub fn compute_key(raw_key: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(raw_key.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Connection handle for a single Agent.
pub struct AgentConnection {
    #[allow(dead_code)]
    pub agent_id: Uuid,
    pub sender: mpsc::Sender<pp_proto::HubMessage>,
    /// Config version the agent is running per core (core_type -> version),
    /// reported at register and updated after each successful push.
    pub config_versions: std::collections::HashMap<String, String>,
}

/// Application state shared across HTTP handlers and gRPC services.
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: HubConfig,
    pub rate_limiter: RateLimiter,
    pub agents: Arc<RwLock<HashMap<Uuid, AgentConnection>>>,
    pub metrics_handle: Option<Arc<MetricsHandle>>,
    pub api_key_cache: ApiKeyCache,
}

impl AppState {
    pub fn new(
        db: DatabaseConnection,
        config: HubConfig,
        rate_limiter: RateLimiter,
        metrics_handle: Option<Arc<MetricsHandle>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            config,
            rate_limiter,
            agents: Arc::new(RwLock::new(HashMap::new())),
            metrics_handle,
            api_key_cache: ApiKeyCache::default(),
        })
    }

    /// Register a new agent connection.
    pub async fn register_agent(
        &self,
        agent_id: Uuid,
        sender: mpsc::Sender<pp_proto::HubMessage>,
        config_versions: std::collections::HashMap<String, String>,
    ) {
        let mut agents = self.agents.write().await;
        agents.insert(
            agent_id,
            AgentConnection {
                agent_id,
                sender,
                config_versions,
            },
        );
        tracing::info!(
            "agent {} registered, total agents: {}",
            agent_id,
            agents.len()
        );
    }

    /// Config version the agent reported (or was last pushed) for a core.
    pub async fn agent_config_version(&self, agent_id: &Uuid, core_type: &str) -> Option<String> {
        let agents = self.agents.read().await;
        agents
            .get(agent_id)?
            .config_versions
            .get(core_type)
            .cloned()
    }

    /// Record the config version an agent is expected to be running.
    pub async fn set_agent_config_version(
        &self,
        agent_id: &Uuid,
        core_type: &str,
        version: String,
    ) {
        let mut agents = self.agents.write().await;
        if let Some(conn) = agents.get_mut(agent_id) {
            conn.config_versions.insert(core_type.to_string(), version);
        }
    }

    /// Unregister an agent connection.
    pub async fn unregister_agent(&self, agent_id: Uuid) {
        let mut agents = self.agents.write().await;
        agents.remove(&agent_id);
        tracing::info!(
            "agent {} unregistered, total agents: {}",
            agent_id,
            agents.len()
        );
    }

    /// Send a message to a specific agent.
    pub async fn send_to_agent(
        &self,
        agent_id: Uuid,
        message: pp_proto::HubMessage,
    ) -> anyhow::Result<()> {
        let agents = self.agents.read().await;
        if let Some(conn) = agents.get(&agent_id) {
            conn.sender
                .send(message)
                .await
                .map_err(|e| anyhow::anyhow!("agent channel closed: {}", e))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("agent {} not connected", agent_id))
        }
    }
}
