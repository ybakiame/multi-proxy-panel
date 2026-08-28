use super::*;
use axum::http::StatusCode;

/// Start local HTTP server for icon tests.
async fn spawn_icon_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/icon.png",
            axum::routing::get(|| async {
                (
                    StatusCode::OK,
                    [("content-type", "image/png")],
                    vec![0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
                )
            }),
        )
        .route(
            "/icon.svg",
            axum::routing::get(|| async {
                (
                    StatusCode::OK,
                    [("content-type", "image/svg+xml")],
                    "<svg></svg>",
                )
            }),
        )
        .route(
            "/noext",
            axum::routing::get(|| async {
                (
                    StatusCode::OK,
                    vec![0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
                )
            }),
        )
        .route(
            "/empty",
            axum::routing::get(|| async { (StatusCode::OK, Vec::<u8>::new()) }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn cache_icon_downloads_infers_extension() {
    let base = spawn_icon_server().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());

    // URL with explicit extension -> preserve extension.
    let png_path = manager
        .cache_icon("rules", &format!("{base}/icon.png"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(png_path.file_name().unwrap().to_str().unwrap(), "rules.png");
    assert_eq!(manager.icon_file("rules").unwrap(), png_path);

    // URL with explicit extension -> preserve extension.
    let svg_path = manager
        .cache_icon("rules", &format!("{base}/icon.svg"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(svg_path.file_name().unwrap().to_str().unwrap(), "rules.svg");
    assert!(!dir.path().join("icons/rules.png").exists());
    assert_eq!(manager.icon_file("rules").unwrap(), svg_path);

    // URL without extension -> infer as png by byte signature.
    let sniffed = manager
        .cache_icon("weird", &format!("{base}/noext"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sniffed.file_name().unwrap().to_str().unwrap(), "weird.png");

    // Empty response -> None, not written to disk.
    assert!(
        manager
            .cache_icon("empty", &format!("{base}/empty"))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(manager.icon_file("empty"), None);
}
