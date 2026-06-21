#[cfg(test)]
mod tests {
    use crate::config::HubConfig;
    use crate::grpc::HubAgentService;
    use crate::rate_limiter::RateLimiter;
    use crate::state::AppState;
    use pp_db::entities::node;
    use pp_proto::{HubMessage, RegisterRequest, hub_message};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect");
        pp_db::run_migrations(&db).await.expect("migrate");
        db
    }

    fn test_state(db: sea_orm::DatabaseConnection) -> Arc<AppState> {
        AppState::new(db, HubConfig::default(), RateLimiter::default(), None)
    }

    fn test_state_auto_register(db: sea_orm::DatabaseConnection) -> Arc<AppState> {
        let config = HubConfig {
            auto_register_agents: true,
            ..Default::default()
        };
        AppState::new(db, config, RateLimiter::default(), None)
    }

    /// Create a pre-provisioned node in the database (simulates CLI provision-node).
    async fn create_provisioned_node(
        db: &sea_orm::DatabaseConnection,
        agent_id: uuid::Uuid,
        token: &str,
    ) {
        let token_hash = pp_common::hash_secret(token).expect("hash");
        let node = node::ActiveModel {
            id: Set(agent_id),
            name: Set("test-node".to_string()),
            hostname: Set("test-host".to_string()),
            address: Set("127.0.0.1".to_string()),
            token_hash: Set(token_hash),
            cores_available: Set(json!(["xray", "sing-box"])),
            labels: Set(None),
            usage_coefficient: Set(1.0),
            status: Set("offline".to_string()),
            last_seen_at: Set(None),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        };
        node.insert(db).await.expect("insert node");
    }

    #[tokio::test]
    async fn grpc_register_rejects_unknown_agent() {
        let db = setup_db().await;
        let state = test_state(db);

        let service = HubAgentService::new(state);
        let (tx, _rx) = mpsc::channel::<HubMessage>(1);

        let result = HubAgentService::test_handle_register(
            &service,
            RegisterRequest {
                agent_id: uuid::Uuid::new_v4().to_string(),
                token: "bad-token".to_string(),
                hostname: "test".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![],
                labels: Default::default(),
            },
            tx,
            None,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn grpc_register_accepts_provisioned_node() {
        let db = setup_db().await;
        let agent_id = uuid::Uuid::new_v4();
        let token = "valid-agent-token-for-test";

        create_provisioned_node(&db, agent_id, token).await;

        let state = test_state(db.clone());
        let service = HubAgentService::new(state);
        let (tx, mut rx) = mpsc::channel::<HubMessage>(1);

        let result = HubAgentService::test_handle_register(
            &service,
            RegisterRequest {
                agent_id: agent_id.to_string(),
                token: token.to_string(),
                hostname: "test-node".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec!["xray".to_string(), "sing-box".to_string()],
                labels: Default::default(),
            },
            tx,
            Some("10.0.0.1".to_string()),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), agent_id);

        // Check we got a RegisterResponse back
        let resp = rx.try_recv().expect("should receive register response");
        match resp.payload {
            Some(hub_message::Payload::RegisterResp(reg_resp)) => {
                assert!(reg_resp.success);
                assert_eq!(reg_resp.assigned_agent_id, agent_id.to_string());
            }
            _ => panic!("expected RegisterResponse"),
        }

        // Check node is now online
        let node_record = node::Entity::find_by_id(agent_id)
            .one(&db)
            .await
            .expect("query node")
            .expect("node exists");
        assert_eq!(node_record.status, "online");
        assert_eq!(node_record.hostname, "test-node");
    }

    #[tokio::test]
    async fn grpc_register_rejects_wrong_token() {
        let db = setup_db().await;
        let agent_id = uuid::Uuid::new_v4();

        create_provisioned_node(&db, agent_id, "real-token").await;

        let state = test_state(db);
        let service = HubAgentService::new(state);
        let (tx, _rx) = mpsc::channel::<HubMessage>(1);

        let result = HubAgentService::test_handle_register(
            &service,
            RegisterRequest {
                agent_id: agent_id.to_string(),
                token: "wrong-token".to_string(),
                hostname: "test".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![],
                labels: Default::default(),
            },
            tx,
            None,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn grpc_register_provisioned_node_twice_succeeds() {
        let db = setup_db().await;
        let agent_id = uuid::Uuid::new_v4();
        let token = "reuse-token";

        create_provisioned_node(&db, agent_id, token).await;

        let state = test_state(db);
        let service = HubAgentService::new(state.clone());

        // Register twice — both should succeed (idempotent)
        for i in 0u32..2 {
            let (tx, _rx) = mpsc::channel::<HubMessage>(1);
            let result = HubAgentService::test_handle_register(
                &service,
                RegisterRequest {
                    agent_id: agent_id.to_string(),
                    token: token.to_string(),
                    hostname: format!("reg-{}", i),
                    version: "0.1.0".to_string(),
                    capabilities: vec![],
                    labels: Default::default(),
                },
                tx,
                None,
            )
            .await;

            assert!(result.is_ok(), "registration {} should succeed", i);
        }
    }

    /// Test that auto-register creates a new node when enabled.
    #[tokio::test]
    async fn grpc_register_auto_register_creates_node() {
        let db = setup_db().await;
        let agent_id = uuid::Uuid::new_v4();
        let token = "auto-register-token";

        let state = test_state_auto_register(db.clone());
        let service = HubAgentService::new(state);
        let (tx, mut rx) = mpsc::channel::<HubMessage>(1);

        let result = HubAgentService::test_handle_register(
            &service,
            RegisterRequest {
                agent_id: agent_id.to_string(),
                token: token.to_string(),
                hostname: "auto-node".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec!["xray".to_string()],
                labels: Default::default(),
            },
            tx,
            Some("192.168.1.1".to_string()),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), agent_id);

        let resp = rx.try_recv().expect("should receive register response");
        match resp.payload {
            Some(hub_message::Payload::RegisterResp(reg_resp)) => {
                assert!(reg_resp.success);
                assert_eq!(reg_resp.assigned_agent_id, agent_id.to_string());
            }
            _ => panic!("expected RegisterResponse"),
        }

        let node_record = node::Entity::find_by_id(agent_id)
            .one(&db)
            .await
            .expect("query node")
            .expect("node exists");
        assert_eq!(node_record.status, "online");
        assert_eq!(node_record.hostname, "auto-node");
        assert_eq!(node_record.address, "192.168.1.1");
    }

    /// Test that auto-register is disabled by default.
    #[tokio::test]
    async fn grpc_register_rejects_unknown_agent_when_auto_register_disabled() {
        let db = setup_db().await;
        let state = test_state(db);
        let service = HubAgentService::new(state);
        let (tx, _rx) = mpsc::channel::<HubMessage>(1);

        let result = HubAgentService::test_handle_register(
            &service,
            RegisterRequest {
                agent_id: uuid::Uuid::new_v4().to_string(),
                token: "some-token".to_string(),
                hostname: "test".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![],
                labels: Default::default(),
            },
            tx,
            None,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }
}
