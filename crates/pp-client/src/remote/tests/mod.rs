use crate::remote::*;
use axum::http::StatusCode;

/// Start local HTTP server (no external network):
/// - `/snippet`: snippet closure generates snippet content based on server address
/// - `/hook.js` / `/task.js` / `/script.js`: fixed script content
/// - `/missing.js`: 404
async fn spawn_remote_server(snippet: impl Fn(&str) -> String) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let snippet = snippet(&base);
    let app = axum::Router::new()
        .route(
            "/snippet",
            axum::routing::get(move || async move { snippet }),
        )
        .route(
            "/hook.js",
            axum::routing::get(|| async { "const hook = 1;" }),
        )
        .route(
            "/task.js",
            axum::routing::get(|| async { "const task = 2;" }),
        )
        .route(
            "/script.js",
            axum::routing::get(|| async { "const script = 3;" }),
        )
        .route(
            "/missing.js",
            axum::routing::get(|| async { (StatusCode::NOT_FOUND, "not found") }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    base
}

#[cfg(test)]
mod args;
#[cfg(test)]
mod crud;
#[cfg(test)]
mod fetch;
#[cfg(test)]
mod icon;
#[cfg(test)]
mod import;
