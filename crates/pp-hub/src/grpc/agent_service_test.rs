#[cfg(test)]
mod tests {
    use crate::config::HubConfig;
    use crate::grpc::HubAgentService;
    use crate::rate_limiter::RateLimiter;
    use crate::state::AppState;
    use pp_db::entities::{
        client, client_online_session, node, node_user_usage_record, traffic_record,
    };
    use pp_proto::{
        HubMessage, InboundTraffic, OnlineUser, OnlineUsersReport, RegisterRequest, TrafficReport,
        UserTraffic, hub_message,
    };
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
            domain: Set(None),
            token_hash: Set(token_hash),
            cores_available: Set(json!(["sing-box"])),
            labels: Set(None),
            usage_coefficient: Set(1.0),
            status: Set("offline".to_string()),
            parent_id: Set(None),
            last_seen_at: Set(None),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        };
        node.insert(db).await.expect("insert node");
    }

    /// Create a client row with the given email.
    async fn create_test_client(
        db: &sea_orm::DatabaseConnection,
        email: Option<&str>,
    ) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        let c = client::ActiveModel {
            id: Set(id),
            user_id: Set(uuid::Uuid::new_v4()),
            name: Set("test-client".to_string()),
            email: Set(email.map(|e| e.to_string())),
            traffic_limit_bytes: Set(0),
            traffic_used_bytes: Set(0),
            all_time_used_bytes: Set(0),
            expiry_date: Set(None),
            reset_day: Set(None),
            data_limit_reset_strategy: Set("no_reset".to_string()),
            last_traffic_reset_time: Set(None),
            max_devices: Set(None),
            status: Set("active".to_string()),
            on_hold_expire_duration_secs: Set(None),
            on_hold_timeout: Set(None),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        };
        c.insert(db).await.expect("insert client");
        id
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
                domain: Default::default(),
                core_config_versions: Default::default(),
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
                capabilities: vec!["sing-box".to_string()],
                labels: Default::default(),
                domain: Default::default(),
                core_config_versions: Default::default(),
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
                domain: Default::default(),
                core_config_versions: Default::default(),
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
                    domain: Default::default(),
                    core_config_versions: Default::default(),
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
                capabilities: vec!["sing-box".to_string()],
                labels: Default::default(),
                domain: Default::default(),
                core_config_versions: Default::default(),
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
                domain: Default::default(),
                core_config_versions: Default::default(),
            },
            tx,
            None,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn grpc_traffic_resolves_email_and_records_usage() {
        let db = setup_db().await;
        let agent_id = uuid::Uuid::new_v4();
        create_provisioned_node(&db, agent_id, "tok").await;
        let client_id = create_test_client(&db, Some("user@example.com")).await;

        let state = test_state(db.clone());
        let service = HubAgentService::new(state);

        let report = TrafficReport {
            timestamp: chrono::Utc::now().timestamp(),
            inbounds: vec![InboundTraffic {
                tag: "in-1".to_string(),
                upload_bytes: 100,
                download_bytes: 200,
            }],
            users: vec![
                UserTraffic {
                    // Reported by email instead of UUID — must be resolved
                    client_id: "user@example.com".to_string(),
                    email: "user@example.com".to_string(),
                    upload_bytes: 10,
                    download_bytes: 20,
                },
                UserTraffic {
                    // Unknown identifier — must be skipped
                    client_id: "nobody@example.com".to_string(),
                    email: "nobody@example.com".to_string(),
                    upload_bytes: 999,
                    download_bytes: 999,
                },
            ],
        };
        service
            .test_handle_traffic(agent_id, report)
            .await
            .expect("handle traffic");

        // User-level usage recorded against the resolved client
        let usage = node_user_usage_record::Entity::find()
            .one(&db)
            .await
            .expect("query usage")
            .expect("usage record exists");
        assert_eq!(usage.node_id, agent_id);
        assert_eq!(usage.client_id, client_id);
        assert_eq!(usage.upload_bytes, 10);
        assert_eq!(usage.download_bytes, 20);

        // Client traffic counter updated (skipped for unknown identifier)
        let c = client::Entity::find_by_id(client_id)
            .one(&db)
            .await
            .expect("query client")
            .expect("client exists");
        assert_eq!(c.traffic_used_bytes, 30);

        // Inbound-level traffic recorded
        let rec = traffic_record::Entity::find()
            .one(&db)
            .await
            .expect("query traffic")
            .expect("traffic record exists");
        assert_eq!(rec.node_id, Some(agent_id));
        assert_eq!(rec.protocol_config_id, None);
        assert_eq!(rec.client_id, None);
        assert_eq!(rec.upload_bytes, 100);
        assert_eq!(rec.download_bytes, 200);
    }

    #[tokio::test]
    async fn grpc_traffic_inbound_records_upsert_per_hour() {
        let db = setup_db().await;
        let agent_id = uuid::Uuid::new_v4();
        create_provisioned_node(&db, agent_id, "tok").await;

        let state = test_state(db.clone());
        let service = HubAgentService::new(state);

        for (tag, up, down) in [("in-a", 100i64, 200i64), ("in-b", 50, 60)] {
            let report = TrafficReport {
                timestamp: chrono::Utc::now().timestamp(),
                inbounds: vec![InboundTraffic {
                    tag: tag.to_string(),
                    upload_bytes: up,
                    download_bytes: down,
                }],
                users: vec![],
            };
            service
                .test_handle_traffic(agent_id, report)
                .await
                .expect("handle traffic");
        }

        // Both reports in the same hour aggregate into a single record
        let records = traffic_record::Entity::find()
            .all(&db)
            .await
            .expect("query traffic");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].upload_bytes, 150);
        assert_eq!(records[0].download_bytes, 260);
    }

    #[tokio::test]
    async fn grpc_online_users_upsert_refreshes_and_prunes() {
        let db = setup_db().await;
        let agent_id = uuid::Uuid::new_v4();
        create_provisioned_node(&db, agent_id, "tok").await;
        let client_id = create_test_client(&db, Some("user@example.com")).await;

        let state = test_state(db.clone());
        let service = HubAgentService::new(state);

        let t0 = chrono::Utc::now().timestamp() - 300;
        let first = OnlineUsersReport {
            timestamp: t0,
            users: vec![
                OnlineUser {
                    client_id: "user@example.com".to_string(),
                    email: "user@example.com".to_string(),
                    ip_address: "1.1.1.1".to_string(),
                    inbound_tag: "in-1".to_string(),
                },
                OnlineUser {
                    client_id: client_id.to_string(),
                    email: "user@example.com".to_string(),
                    ip_address: "2.2.2.2".to_string(),
                    inbound_tag: String::new(),
                },
            ],
        };
        service
            .test_handle_online_users(agent_id, first)
            .await
            .expect("handle online users");

        let sessions = client_online_session::Entity::find()
            .all(&db)
            .await
            .expect("query sessions");
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().all(|s| s.client_id == client_id));

        // Second report only contains 1.1.1.1 — session refresh + prune
        let second = OnlineUsersReport {
            timestamp: chrono::Utc::now().timestamp(),
            users: vec![OnlineUser {
                client_id: "user@example.com".to_string(),
                email: "user@example.com".to_string(),
                ip_address: "1.1.1.1".to_string(),
                inbound_tag: "in-1".to_string(),
            }],
        };
        service
            .test_handle_online_users(agent_id, second)
            .await
            .expect("handle online users");

        let sessions = client_online_session::Entity::find()
            .all(&db)
            .await
            .expect("query sessions");
        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.ip_address, "1.1.1.1");
        // connected_at preserved from the first report
        assert_eq!(session.connected_at.timestamp(), t0);
        // last_active_at refreshed
        assert!(session.last_active_at.timestamp() > t0);
    }
}
