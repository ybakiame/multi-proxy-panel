use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::{HubConfig, rate_limiter::RateLimiter, routes::metrics_export::MetricsHandle};

/// Connection handle for a single Agent.
pub struct AgentConnection {
    #[allow(dead_code)]
    pub agent_id: Uuid,
    pub sender: mpsc::Sender<pp_proto::HubMessage>,
}

/// Application state shared across HTTP handlers and gRPC services.
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: HubConfig,
    pub rate_limiter: RateLimiter,
    pub agents: Arc<RwLock<HashMap<Uuid, AgentConnection>>>,
    pub metrics_handle: Option<Arc<MetricsHandle>>,
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
        })
    }

    /// Register a new agent connection.
    pub async fn register_agent(&self,
        agent_id: Uuid,
        sender: mpsc::Sender<pp_proto::HubMessage>,
    ) {
        let mut agents = self.agents.write().await;
        agents.insert(agent_id, AgentConnection { agent_id, sender });
        tracing::info!(
            "agent {} registered, total agents: {}",
            agent_id,
            agents.len()
        );
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
