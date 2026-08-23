//! HTTP API integration tests for ProxyPanel Hub.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

use crate::rate_limiter::RateLimiter;
use crate::{AppState, HubConfig, build_app, ensure_bootstrap_api_key};

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

async fn bootstrap_app() -> (Router, Arc<AppState>, String) {
    let db = setup_db().await;
    let state = test_state(db);
    ensure_bootstrap_api_key(&state.db)
        .await
        .expect("bootstrap key");

    // Retrieve the bootstrap key hash so we can authenticate. Since the raw key
    // is printed to stderr, we recreate it by hashing the generated token.
    // Instead, we insert a known test key directly for predictable tests.
    let test_key = "test_api_key_for_integration_tests_only";
    let key_hash = pp_common::hash_secret(test_key).expect("hash");
    let key = pp_db::entities::api_key::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        name: Set("integration-test".to_string()),
        key_hash: Set(key_hash),
        scopes: Set(json!(["*"])),
        ip_allowlist: Set(None),
        rate_limit: Set(None),
        expires_at: Set(None),
        is_active: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };
    key.insert(&state.db).await.expect("insert test key");

    let app = build_app(state.clone(), &state.config);
    (app, state, test_key.to_string())
}

fn api_request(method: &str, uri: &str, key: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-api-key", key);

    match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            builder
                .body(Body::from(value.to_string()))
                .expect("request")
        }
        None => builder.body(Body::empty()).expect("request"),
    }
}

async fn response_json(response: axum::response::Response) -> Value {
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let mut value: Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        json!({
            "_status": status.as_u16(),
            "_body": String::from_utf8_lossy(&bytes).to_string(),
        })
    });
    if let Some(obj) = value.as_object_mut() {
        obj.insert("_status".to_string(), json!(status.as_u16()));
    }
    value
}

#[tokio::test]
async fn health_endpoint_returns_healthy() {
    let (app, _state, _key) = bootstrap_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["data"]["status"], "healthy");
}

#[tokio::test]
async fn protected_endpoint_requires_api_key() {
    let (app, _state, _key) = bootstrap_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn nodes_crud_lifecycle() {
    let (app, _state, key) = bootstrap_app().await;

    // Create
    let create_response = app
        .clone()
        .oneshot(api_request(
            "POST",
            "/api/v1/nodes",
            &key,
            Some(json!({ "name": "test-node" })),
        ))
        .await
        .expect("create");
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let node_id = create_body["data"]["id"].as_str().expect("node id");
    assert!(create_body["data"]["token"].as_str().is_some());

    // List
    let list_response = app
        .clone()
        .oneshot(api_request("GET", "/api/v1/nodes", &key, None))
        .await
        .expect("list");
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = response_json(list_response).await;
    assert_eq!(list_body["data"].as_array().unwrap().len(), 1);
    assert_eq!(list_body["meta"]["total"], 1);

    // Get
    let get_response = app
        .clone()
        .oneshot(api_request(
            "GET",
            &format!("/api/v1/nodes/{}", node_id),
            &key,
            None,
        ))
        .await
        .expect("get");
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = response_json(get_response).await;
    assert_eq!(get_body["data"]["name"], "test-node");

    // Update
    let update_response = app
        .clone()
        .oneshot(api_request(
            "PUT",
            &format!("/api/v1/nodes/{}", node_id),
            &key,
            Some(json!({ "name": "renamed-node" })),
        ))
        .await
        .expect("update");
    assert_eq!(update_response.status(), StatusCode::OK);

    // Delete
    let delete_response = app
        .clone()
        .oneshot(api_request(
            "DELETE",
            &format!("/api/v1/nodes/{}", node_id),
            &key,
            None,
        ))
        .await
        .expect("delete");
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // Get after delete
    let get_response = app
        .oneshot(api_request(
            "GET",
            &format!("/api/v1/nodes/{}", node_id),
            &key,
            None,
        ))
        .await
        .expect("get after delete");
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn protocols_crud_with_validation() {
    let (app, _state, key) = bootstrap_app().await;

    // Create with invalid core/protocol combo
    let bad_response = app
        .clone()
        .oneshot(api_request(
            "POST",
            "/api/v1/protocols",
            &key,
            Some(json!({
                "name": "bad",
                "protocol_type": "vless_xhttp",
                "core_type": "sing-box",
                "listen_port": 10086,
            })),
        ))
        .await
        .expect("bad create");
    assert_eq!(bad_response.status(), StatusCode::BAD_REQUEST);

    // Create valid protocol
    let create_response = app
        .clone()
        .oneshot(api_request(
            "POST",
            "/api/v1/protocols",
            &key,
            Some(json!({
                "name": "vless-reality",
                "protocol_type": "vless_reality",
                "core_type": "sing-box",
                "listen_port": 443,
                "settings": {
                    "clients": [],
                    "reality": {
                        "enabled": true,
                        "dest": "example.com:443",
                        "serverNames": ["example.com"],
                        "privateKey": "test",
                        "publicKey": "test",
                        "shortIds": ["test"]
                    }
                }
            })),
        ))
        .await
        .expect("create");
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let config_id = create_body["data"]["id"].as_str().expect("config id");

    // List
    let list_response = app
        .clone()
        .oneshot(api_request("GET", "/api/v1/protocols", &key, None))
        .await
        .expect("list");
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = response_json(list_response).await;
    assert_eq!(list_body["meta"]["total"], 1);

    // Delete
    let delete_response = app
        .oneshot(api_request(
            "DELETE",
            &format!("/api/v1/protocols/{}", config_id),
            &key,
            None,
        ))
        .await
        .expect("delete");
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn api_key_scope_enforcement() {
    let db = setup_db().await;
    let state = test_state(db);

    // Key with only read scope
    let read_key = "read_only_key";
    let key_hash = pp_common::hash_secret(read_key).expect("hash");
    let key = pp_db::entities::api_key::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        name: Set("read-only".to_string()),
        key_hash: Set(key_hash),
        scopes: Set(json!(["nodes:read"])),
        ip_allowlist: Set(None),
        rate_limit: Set(None),
        expires_at: Set(None),
        is_active: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };
    key.insert(&state.db).await.expect("insert key");

    let app = build_app(state, &HubConfig::default());

    // Read should succeed
    let list_response = app
        .clone()
        .oneshot(api_request("GET", "/api/v1/nodes", read_key, None))
        .await
        .expect("list");
    assert_eq!(list_response.status(), StatusCode::OK);

    // Write should fail
    let create_response = app
        .oneshot(api_request(
            "POST",
            "/api/v1/nodes",
            read_key,
            Some(json!({ "name": "forbidden" })),
        ))
        .await
        .expect("create");
    assert_eq!(create_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn usage_endpoints_smoke() {
    let (app, _state, key) = bootstrap_app().await;

    // Empty database: both endpoints return empty lists
    for uri in [
        "/api/v1/usage",
        "/api/v1/usage?limit=5",
        "/api/v1/usage/summary",
        "/api/v1/usage/summary?group_by=node",
        "/api/v1/traffic?start=2024-01-01T00:00:00Z&end=2025-01-01T00:00:00Z&limit=10",
    ] {
        let response = app
            .clone()
            .oneshot(api_request("GET", uri, &key, None))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK, "uri: {}", uri);
        let body = response_json(response).await;
        assert_eq!(body["data"].as_array().expect("array").len(), 0);
    }

    // Invalid group_by is rejected
    let response = app
        .oneshot(api_request(
            "GET",
            "/api/v1/usage/summary?group_by=bogus",
            &key,
            None,
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn install_script_endpoint_returns_shell_script() {
    let (app, _state, _key) = bootstrap_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/install.sh")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/x-shellscript"),
        "content-type: {}",
        content_type
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("proxy-panel-agent"),
        "script should mention proxy-panel-agent"
    );
    assert!(
        !text.contains("__PROXYPANEL_RELEASE_REPO__"),
        "placeholder should be replaced"
    );
}

#[tokio::test]
async fn node_install_command_rotates_token() {
    let (app, _state, key) = bootstrap_app().await;

    // Create a node
    let create_response = app
        .clone()
        .oneshot(api_request(
            "POST",
            "/api/v1/nodes",
            &key,
            Some(json!({ "name": "install-test-node" })),
        ))
        .await
        .expect("create");
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let node_id = create_body["data"]["id"].as_str().expect("node id");

    // First install command call
    let resp1 = app
        .clone()
        .oneshot(api_request(
            "POST",
            &format!("/api/v1/nodes/{}/install-command", node_id),
            &key,
            None,
        ))
        .await
        .expect("install-command 1");
    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = response_json(resp1).await;
    let token1 = body1["data"]["token"].as_str().expect("token 1");
    let command1 = body1["data"]["command"].as_str().expect("command 1");
    assert!(!token1.is_empty());
    assert!(command1.contains("curl -fsSL"));
    assert!(command1.contains(&format!("--agent-id '{}'", node_id)));

    // Second install command call — token should be different
    let resp2 = app
        .clone()
        .oneshot(api_request(
            "POST",
            &format!("/api/v1/nodes/{}/install-command", node_id),
            &key,
            None,
        ))
        .await
        .expect("install-command 2");
    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = response_json(resp2).await;
    let token2 = body2["data"]["token"].as_str().expect("token 2");
    assert_ne!(token1, token2, "token should be rotated");
}

#[tokio::test]
async fn node_install_command_not_found() {
    let (app, _state, key) = bootstrap_app().await;

    let response = app
        .oneshot(api_request(
            "POST",
            "/api/v1/nodes/00000000-0000-0000-0000-000000000000/install-command",
            &key,
            None,
        ))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
