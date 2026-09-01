use super::*;
use axum::http::HeaderValue;

/// Local axum server verification: PATCH body is `{"mode": ...}`, secret non-empty with Bearer
/// auth; non-2xx returns Err.
#[tokio::test]
async fn push_clash_mode_patches_configs_with_bearer_and_checks_status() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let captured_body = std::sync::Arc::new(std::sync::Mutex::new(None));
    let captured_auth = std::sync::Arc::new(std::sync::Mutex::new(None));
    let body_ref = std::sync::Arc::clone(&captured_body);
    let auth_ref = std::sync::Arc::clone(&captured_auth);
    let app = axum::Router::new().route(
        "/configs",
        axum::routing::patch(
            move |req: axum::http::Request<axum::body::Body>| async move {
                *auth_ref.lock().unwrap() = req.headers().get("authorization").cloned();
                let bytes = axum::body::to_bytes(req.into_body(), 1024).await.unwrap();
                *body_ref.lock().unwrap() = Some(bytes.to_vec());
                axum::http::StatusCode::NO_CONTENT
            },
        ),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    push_clash_mode(addr.port(), "sekret", "direct")
        .await
        .unwrap();
    assert_eq!(
        captured_body.lock().unwrap().as_ref().unwrap(),
        &br#"{"mode":"direct"}"#.to_vec()
    );
    assert_eq!(
        captured_auth.lock().unwrap().as_ref(),
        Some(&HeaderValue::from_static("Bearer sekret"))
    );

    // secret empty string -> no auth header.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr2 = listener.local_addr().unwrap();
    let captured_auth2 = std::sync::Arc::new(std::sync::Mutex::new(None));
    let auth_ref2 = std::sync::Arc::clone(&captured_auth2);
    let app2 = axum::Router::new().route(
        "/configs",
        axum::routing::patch(
            move |req: axum::http::Request<axum::body::Body>| async move {
                *auth_ref2.lock().unwrap() = req.headers().get("authorization").cloned();
                axum::http::StatusCode::NO_CONTENT
            },
        ),
    );
    tokio::spawn(async move {
        axum::serve(listener, app2).await.unwrap();
    });
    push_clash_mode(addr2.port(), "", "rule").await.unwrap();
    assert!(
        captured_auth2.lock().unwrap().as_ref().is_none(),
        "empty secret should not have Authorization header"
    );

    // non-2xx -> Err.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr3 = listener.local_addr().unwrap();
    let app3 = axum::Router::new().route(
        "/configs",
        axum::routing::patch(|| async { axum::http::StatusCode::BAD_REQUEST }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app3).await.unwrap();
    });
    assert!(push_clash_mode(addr3.port(), "", "direct").await.is_err());
}

/// Retry semantics: Clash API may not be ready when core just started (first two 500s), third success ->
/// returns Ok after retry, and request count = 3.
#[tokio::test]
async fn push_clash_mode_retries_transient_failure_until_success() {
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_ref = std::sync::Arc::clone(&attempts);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new().route(
        "/configs",
        axum::routing::patch(move || async move {
            let n = attempts_ref.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n < 2 {
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            } else {
                axum::http::StatusCode::NO_CONTENT
            }
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    push_clash_mode(addr.port(), "", "global").await.unwrap();
    assert_eq!(
        attempts.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "should retry and succeed on third attempt after two failures"
    );
}

/// Retry semantics: all attempts fail -> Err (caller best-effort logs warning without blocking).
#[tokio::test]
async fn push_clash_mode_returns_err_when_all_retries_fail() {
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_ref = std::sync::Arc::clone(&attempts);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new().route(
        "/configs",
        axum::routing::patch(move || async move {
            attempts_ref.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    assert!(push_clash_mode(addr.port(), "", "global").await.is_err());
    assert_eq!(
        attempts.load(std::sync::atomic::Ordering::SeqCst),
        5,
        "should retry at most 5 times (backoff) when all fail"
    );
}
