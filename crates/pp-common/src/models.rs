use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shared lightweight DTOs used across API boundaries.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDto {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub address: String,
    pub cores_available: Vec<super::CoreType>,
    pub labels: serde_json::Value,
    pub status: super::NodeStatus,
    pub last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfigDto {
    pub id: Uuid,
    pub name: String,
    pub protocol_type: super::ProtocolType,
    pub core_type: super::CoreType,
    pub listen_port: u16,
    pub listen_address: String,
    pub settings: serde_json::Value,
    pub tls_settings: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub traffic_limit_bytes: i64,
    pub traffic_used_bytes: i64,
    pub expiry_date: Option<chrono::DateTime<chrono::Utc>>,
    pub reset_day: Option<i32>,
    pub status: super::UserStatus,
    pub group_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGroupDto {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: serde_json::Value,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
