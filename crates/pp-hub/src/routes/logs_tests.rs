use super::*;
use sea_orm::{ActiveModelTrait, Set};

#[tokio::test]
async fn queries_persisted_logs_with_filters_pagination_and_redaction() {
    let db = crate::integration_tests::setup_db().await;
    for (level, source) in [("info", "startup"), ("warn", "scheduler")] {
        system_log::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            level: Set(level.into()),
            source: Set(source.into()),
            message: Set("event token=private".into()),
            metadata: Set(None),
            created_at: Set(chrono::Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert");
    }
    let state = crate::integration_tests::test_state(db);
    let result = query_logs(State(state.clone()), Query(HashMap::new()))
        .await
        .expect("query");
    assert_eq!(result.total, 2);
    assert!(
        result
            .data
            .iter()
            .all(|r| !r["message"].as_str().expect("message").contains("private"))
    );
    let params = [
        ("level", "WARN"),
        ("source", "sched"),
        ("page", "1"),
        ("per_page", "1"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect();
    let result = query_logs(State(state.clone()), Query(params))
        .await
        .expect("filtered");
    assert_eq!(result.total, 1);
    assert_eq!(result.data[0]["source"], "scheduler");
    let result = query_logs(
        State(state),
        Query(HashMap::from([("kind".into(), "invalid".into())])),
    )
    .await
    .expect_err("invalid kind");
    assert_eq!(result.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn agent_query_returns_node_identity_and_filters_by_node_name() {
    let db = crate::integration_tests::setup_db().await;
    let id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    node::ActiveModel {
        id: Set(id),
        name: Set("test-node".into()),
        hostname: Set("test".into()),
        address: Set("127.0.0.1".into()),
        domain: Set(None),
        token_hash: Set("never-return-this".into()),
        cores_available: Set(json!([])),
        labels: Set(None),
        usage_coefficient: Set(1.0),
        status: Set("online".into()),
        parent_id: Set(None),
        last_seen_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .expect("node");
    agent_log::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        node_id: Set(id),
        level: Set("info".into()),
        target: Set("pp_core".into()),
        message: Set("core started".into()),
        fields: Set(None),
        created_at: Set(now),
    }
    .insert(&db)
    .await
    .expect("agent log");
    let state = crate::integration_tests::test_state(db);
    let params = HashMap::from([
        ("kind".into(), "agent".into()),
        ("source".into(), "test-node".into()),
    ]);
    let result = query_logs(State(state.clone()), Query(params))
        .await
        .expect("agent query");
    assert_eq!(result.total, 1);
    assert_eq!(result.data[0]["node_name"], "test-node");
    assert_eq!(result.data[0]["source"], "pp_core");
    assert!(
        !serde_json::to_string(&result.data)
            .expect("json")
            .contains("never-return-this")
    );
    let result = query_logs(State(state), Query(HashMap::new()))
        .await
        .expect("system query");
    assert!(result.data.is_empty());
}
