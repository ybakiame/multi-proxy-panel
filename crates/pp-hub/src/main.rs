//! ProxyPanel Hub — Central management panel.

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::path::PathBuf;
use clap::Parser;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod grpc;
mod middleware;
mod routes;
mod service;
mod state;

use grpc::HubAgentService;
use routes::{bindings, client, health, logs, metrics, nodes, protocol, subscription, traffic};
use state::AppState;

#[derive(Parser, Debug)]
#[command(name = "proxy-panel-hub", version, about = "ProxyPanel Hub server")]
struct Args {
    #[arg(short, long, default_value = "config/hub.toml")]
    config: String,

    #[arg(long, env = "PROXYPANEL_HUB_LISTEN", default_value = "0.0.0.0:8081")]
    listen: String,

    #[arg(long, env = "PROXYPANEL_GRPC_LISTEN", default_value = "0.0.0.0:50052")]
    grpc_listen: String,

    #[arg(long, env = "PROXYPANEL_DATABASE_URL")]
    database_url: Option<String>,

    #[arg(long, env = "PROXYPANEL_STATIC_DIR", default_value = "crates/pp-web/dist")]
    static_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "proxy_panel_hub=info,tower_http=debug".into()),
        )
        .init();

    let args = Args::parse();
    tracing::info!("starting ProxyPanel Hub with config: {}", args.config);

    let db_url = args.database_url.unwrap_or_else(|| {
        "postgres://proxypanel:proxypanel@localhost/proxypanel".to_string()
    });

    let db = pp_db::init_db(&db_url).await?;
    pp_db::run_migrations(&db).await?;

    let state = AppState::new(db);

    // HTTP API server
    // Static file serving for the Dioxus web app.
    // API routes take precedence; anything else falls back to the SPA.
    let static_service = tower_http::services::ServeDir::new(&args.static_dir)
        .fallback(tower_http::services::ServeFile::new(
            args.static_dir.join("index.html"),
        ));

    let app = Router::new()
        .route("/health", get(health::health))
        // Nodes
        .route("/api/v1/nodes", get(nodes::list_nodes).post(nodes::create_node))
        .route("/api/v1/nodes/{id}", get(nodes::get_node).delete(nodes::delete_node))
        .route("/api/v1/nodes/{id}/push", post(nodes::push_config))
        // Protocol Configs
        .route("/api/v1/protocols", get(protocol::list_configs).post(protocol::create_config))
        .route("/api/v1/protocols/{id}", get(protocol::get_config).put(protocol::update_config).delete(protocol::delete_config))
        .route("/api/v1/utils/generate-reality-keys", get(protocol::generate_reality_keys))
        // Node Bindings
        .route("/api/v1/bindings", get(bindings::list_bindings).post(bindings::create_binding))
        .route("/api/v1/bindings/{id}", delete(bindings::delete_binding))
        // Clients
        .route("/api/v1/clients", get(client::list_clients).post(client::create_client))
        .route("/api/v1/clients/{id}", get(client::get_client).put(client::update_client).delete(client::delete_client))
        // Subscription Templates
        .route("/api/v1/templates", get(subscription::list_templates).post(subscription::create_template))
        .route("/api/v1/templates/{id}", delete(subscription::delete_template))
        // Subscriptions
        .route("/api/v1/subscriptions", get(subscription::list_subscriptions).post(subscription::create_subscription))
        .route("/api/v1/subscriptions/{id}", delete(subscription::delete_subscription))
        // Public subscription endpoint
        .route("/sub/{token}", get(subscription::serve_subscription))
        // Traffic
        .route("/api/v1/traffic", get(traffic::query_traffic))
        // Metrics
        .route("/api/v1/metrics", get(metrics::query_metrics))
        .route("/api/v1/metrics/{node_id}/latest", get(metrics::latest_metrics))
        // Logs
        .route("/api/v1/logs", get(logs::query_logs))
        .fallback_service(static_service)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    let http_addr: SocketAddr = args.listen.parse()?;
    let grpc_addr: SocketAddr = args.grpc_listen.parse()?;

    tracing::info!("HTTP API listening on http://{}", http_addr);
    tracing::info!("gRPC listening on http://{}", grpc_addr);

    let http_listener = tokio::net::TcpListener::bind(http_addr).await?;
    let http_server = axum::serve(http_listener, app).with_graceful_shutdown(shutdown_signal());

    // gRPC server
    let grpc_server = tonic::transport::Server::builder()
        .add_service(pp_proto::hub_agent_server::HubAgentServer::new(
            HubAgentService::new(state),
        ))
        .serve(grpc_addr);

    tokio::select! {
        res = http_server => {
            if let Err(e) = res {
                tracing::error!("HTTP server error: {}", e);
            }
        }
        res = grpc_server => {
            if let Err(e) = res {
                tracing::error!("gRPC server error: {}", e);
            }
        }
        _ = shutdown_signal() => {
            tracing::info!("shutdown signal received, stopping servers");
        }
    }

    tracing::info!("ProxyPanel Hub shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        sigterm.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
