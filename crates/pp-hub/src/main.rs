//! ProxyPanel Hub — Central management panel.

#![allow(clippy::result_large_err)]

use axum::{
    Router,
    http::header,
    routing::{delete, get, post, put},
};
use clap::Parser;
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tower::Service;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod config;
mod grpc;
mod middleware;
mod rate_limiter;
mod response;
mod routes;
mod service;
mod state;

use config::{ConfigOverrides, HubConfig};
use grpc::HubAgentService;
use middleware::api_key::scopes;
use pp_db::entities::api_key as api_key_entity;
use rate_limiter::RateLimiter;
use routes::{
    api_key, bindings, certificates, client, core_version, health, inbound_host, login, logs,
    metrics, metrics_export, node_group, nodes, onlines, protocol, protocol_preset, relay_rule,
    subscription, traffic, usage, webhook,
};
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

    #[arg(long, env = "PROXYPANEL_AUTO_REGISTER_AGENTS")]
    auto_register_agents: Option<bool>,

    #[arg(
        long,
        env = "PROXYPANEL_STATIC_DIR",
        default_value = "crates/pp-web/dist"
    )]
    static_dir: PathBuf,

    #[arg(long, env = "PROXYPANEL_GRPC_TLS_CERT")]
    grpc_tls_cert: Option<PathBuf>,

    #[arg(long, env = "PROXYPANEL_GRPC_TLS_KEY")]
    grpc_tls_key: Option<PathBuf>,

    #[arg(long, env = "PROXYPANEL_HTTP_TLS_CERT")]
    http_tls_cert: Option<PathBuf>,

    #[arg(long, env = "PROXYPANEL_HTTP_TLS_KEY")]
    http_tls_key: Option<PathBuf>,
}

/// Build the full HTTP router including protected API, public routes, CORS, and tracing.
pub fn build_app(state: Arc<AppState>, hub_config: &HubConfig) -> Router {
    // Static file serving for the React web app.
    // API routes take precedence; anything else falls back to the SPA.
    let static_service = tower_http::services::ServeDir::new(&hub_config.static_dir).fallback(
        tower_http::services::ServeFile::new(hub_config.static_dir.join("index.html")),
    );

    // Protected API routes (require API Key)
    // Note: axum's MethodRouter::route_layer applies to all methods on the same
    // path, so read/write endpoints must be registered as separate routes to
    // enforce different scopes.
    let protected_api = Router::new()
        // Nodes
        .route(
            "/api/v1/nodes",
            get(nodes::list_nodes)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_READ)),
        )
        .route(
            "/api/v1/nodes",
            post(nodes::create_node)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        .route(
            "/api/v1/nodes/{id}",
            get(nodes::get_node).route_layer(middleware::api_key::scope_layer(scopes::NODES_READ)),
        )
        .route(
            "/api/v1/nodes/{id}",
            put(nodes::update_node)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        .route(
            "/api/v1/nodes/{id}",
            delete(nodes::delete_node)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        .route("/api/v1/nodes/{id}/push", post(nodes::push_config))
        .route(
            "/api/v1/nodes/{id}/binaries",
            get(nodes::list_core_binaries),
        )
        .route(
            "/api/v1/nodes/{id}/binaries/{file}",
            delete(nodes::delete_core_binary),
        )
        .route(
            "/api/v1/nodes/{id}/logs",
            get(nodes::query_node_logs)
                .route_layer(middleware::api_key::scope_layer(scopes::LOGS_READ)),
        )
        // Node Groups
        .route(
            "/api/v1/groups",
            get(node_group::list_groups)
                .route_layer(middleware::api_key::scope_layer(scopes::GROUPS_READ)),
        )
        .route(
            "/api/v1/groups",
            post(node_group::create_group)
                .route_layer(middleware::api_key::scope_layer(scopes::GROUPS_WRITE)),
        )
        .route(
            "/api/v1/groups/{id}",
            get(node_group::get_group)
                .route_layer(middleware::api_key::scope_layer(scopes::GROUPS_READ)),
        )
        .route(
            "/api/v1/groups/{id}",
            put(node_group::update_group)
                .route_layer(middleware::api_key::scope_layer(scopes::GROUPS_WRITE)),
        )
        .route(
            "/api/v1/groups/{id}",
            delete(node_group::delete_group)
                .route_layer(middleware::api_key::scope_layer(scopes::GROUPS_WRITE)),
        )
        // Protocol Configs
        .route(
            "/api/v1/protocols",
            get(protocol::list_configs)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_READ)),
        )
        .route(
            "/api/v1/protocols",
            post(protocol::create_config)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_WRITE)),
        )
        .route(
            "/api/v1/protocols/{id}",
            get(protocol::get_config)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_READ)),
        )
        .route(
            "/api/v1/protocols/{id}",
            put(protocol::update_config)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_WRITE)),
        )
        .route(
            "/api/v1/protocols/{id}",
            delete(protocol::delete_config)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_WRITE)),
        )
        .route(
            "/api/v1/utils/generate-reality-keys",
            get(protocol::generate_reality_keys)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_READ)),
        )
        .route(
            "/api/v1/protocols/presets",
            get(protocol_preset::list_available_presets)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_READ)),
        )
        .route(
            "/api/v1/protocols/presets",
            post(protocol_preset::apply_preset)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_WRITE)),
        )
        // Core Versions
        .route(
            "/api/v1/core-versions",
            get(core_version::list_core_versions)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_READ)),
        )
        .route(
            "/api/v1/core-versions",
            post(core_version::save_core_versions)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_WRITE)),
        )
        .route(
            "/api/v1/core-versions/upstream",
            get(core_version::list_upstream_versions)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_READ)),
        )
        .route(
            "/api/v1/core-versions/{id}",
            delete(core_version::delete_core_version)
                .route_layer(middleware::api_key::scope_layer(scopes::PROTOCOLS_WRITE)),
        )
        // Certificates
        .route(
            "/api/v1/certificates",
            get(certificates::list_certificates)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_READ)),
        )
        .route(
            "/api/v1/certificates",
            post(certificates::create_certificate)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        .route(
            "/api/v1/certificates/{id}/renew",
            post(certificates::renew_certificate)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        .route(
            "/api/v1/certificates/{id}",
            delete(certificates::delete_certificate)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        // Node Bindings
        .route(
            "/api/v1/bindings",
            get(bindings::list_bindings)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_READ)),
        )
        .route(
            "/api/v1/bindings",
            post(bindings::create_binding)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_WRITE)),
        )
        .route(
            "/api/v1/bindings/{id}",
            put(bindings::update_binding)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_WRITE)),
        )
        .route(
            "/api/v1/bindings/{id}",
            delete(bindings::delete_binding)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_WRITE)),
        )
        // Relay Rules
        .route(
            "/api/v1/relay-rules",
            get(relay_rule::list_relay_rules)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_READ)),
        )
        .route(
            "/api/v1/relay-rules/library",
            get(relay_rule::library)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_READ)),
        )
        .route(
            "/api/v1/relay-rules",
            post(relay_rule::create_relay_rule)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        .route(
            "/api/v1/relay-rules/{id}",
            put(relay_rule::update_relay_rule)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        .route(
            "/api/v1/relay-rules/{id}",
            delete(relay_rule::delete_relay_rule)
                .route_layer(middleware::api_key::scope_layer(scopes::NODES_WRITE)),
        )
        // Inbound Hosts (user-facing address overrides)
        .route(
            "/api/v1/hosts",
            get(inbound_host::list_hosts)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_READ)),
        )
        .route(
            "/api/v1/hosts",
            post(inbound_host::create_host)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_WRITE)),
        )
        .route(
            "/api/v1/hosts/{id}",
            get(inbound_host::get_host)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_READ)),
        )
        .route(
            "/api/v1/hosts/{id}",
            put(inbound_host::update_host)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_WRITE)),
        )
        .route(
            "/api/v1/hosts/{id}",
            delete(inbound_host::delete_host)
                .route_layer(middleware::api_key::scope_layer(scopes::BINDINGS_WRITE)),
        )
        // Clients
        .route(
            "/api/v1/clients",
            get(client::list_clients)
                .route_layer(middleware::api_key::scope_layer(scopes::CLIENTS_READ)),
        )
        .route(
            "/api/v1/clients",
            post(client::create_client)
                .route_layer(middleware::api_key::scope_layer(scopes::CLIENTS_WRITE)),
        )
        .route(
            "/api/v1/clients/{id}",
            get(client::get_client)
                .route_layer(middleware::api_key::scope_layer(scopes::CLIENTS_READ)),
        )
        .route(
            "/api/v1/clients/{id}",
            put(client::update_client)
                .route_layer(middleware::api_key::scope_layer(scopes::CLIENTS_WRITE)),
        )
        .route(
            "/api/v1/clients/{id}",
            delete(client::delete_client)
                .route_layer(middleware::api_key::scope_layer(scopes::CLIENTS_WRITE)),
        )
        // Subscription Templates
        .route(
            "/api/v1/templates",
            get(subscription::list_templates)
                .route_layer(middleware::api_key::scope_layer(scopes::TEMPLATES_READ)),
        )
        .route(
            "/api/v1/templates",
            post(subscription::create_template)
                .route_layer(middleware::api_key::scope_layer(scopes::TEMPLATES_WRITE)),
        )
        .route(
            "/api/v1/templates/{id}",
            put(subscription::update_template)
                .route_layer(middleware::api_key::scope_layer(scopes::TEMPLATES_WRITE)),
        )
        .route(
            "/api/v1/templates/{id}",
            delete(subscription::delete_template)
                .route_layer(middleware::api_key::scope_layer(scopes::TEMPLATES_WRITE)),
        )
        // Subscriptions
        .route(
            "/api/v1/subscriptions",
            get(subscription::list_subscriptions)
                .route_layer(middleware::api_key::scope_layer(scopes::SUBSCRIPTIONS_READ)),
        )
        .route(
            "/api/v1/subscriptions",
            post(subscription::create_subscription).route_layer(middleware::api_key::scope_layer(
                scopes::SUBSCRIPTIONS_WRITE,
            )),
        )
        .route(
            "/api/v1/subscriptions/{id}",
            put(subscription::update_subscription).route_layer(middleware::api_key::scope_layer(
                scopes::SUBSCRIPTIONS_WRITE,
            )),
        )
        .route(
            "/api/v1/subscriptions/{id}",
            delete(subscription::delete_subscription).route_layer(
                middleware::api_key::scope_layer(scopes::SUBSCRIPTIONS_WRITE),
            ),
        )
        // API Keys
        .route(
            "/api/v1/api-keys",
            get(api_key::list_keys)
                .route_layer(middleware::api_key::scope_layer(scopes::API_KEYS_READ)),
        )
        .route(
            "/api/v1/api-keys",
            post(api_key::create_key)
                .route_layer(middleware::api_key::scope_layer(scopes::API_KEYS_WRITE)),
        )
        .route(
            "/api/v1/api-keys/{id}",
            put(api_key::update_key)
                .route_layer(middleware::api_key::scope_layer(scopes::API_KEYS_WRITE)),
        )
        .route(
            "/api/v1/api-keys/{id}",
            delete(api_key::delete_key)
                .route_layer(middleware::api_key::scope_layer(scopes::API_KEYS_WRITE)),
        )
        // Traffic
        .route(
            "/api/v1/traffic",
            get(traffic::query_traffic)
                .route_layer(middleware::api_key::scope_layer(scopes::TRAFFIC_READ)),
        )
        // Usage
        .route(
            "/api/v1/usage",
            get(usage::query_usage)
                .route_layer(middleware::api_key::scope_layer(scopes::TRAFFIC_READ)),
        )
        .route(
            "/api/v1/usage/summary",
            get(usage::usage_summary)
                .route_layer(middleware::api_key::scope_layer(scopes::TRAFFIC_READ)),
        )
        .route(
            "/api/v1/clients/{id}/reset-traffic",
            post(client::reset_client_traffic)
                .route_layer(middleware::api_key::scope_layer(scopes::CLIENTS_WRITE)),
        )
        // Metrics
        .route(
            "/api/v1/metrics",
            get(metrics::query_metrics)
                .route_layer(middleware::api_key::scope_layer(scopes::METRICS_READ)),
        )
        .route(
            "/api/v1/metrics/{node_id}/latest",
            get(metrics::latest_metrics)
                .route_layer(middleware::api_key::scope_layer(scopes::METRICS_READ)),
        )
        // Onlines
        .route(
            "/api/v1/onlines",
            get(onlines::list_onlines)
                .route_layer(middleware::api_key::scope_layer(scopes::ONLINES_READ)),
        )
        .route(
            "/api/v1/onlines/count",
            get(onlines::get_online_count)
                .route_layer(middleware::api_key::scope_layer(scopes::ONLINES_READ)),
        )
        .route(
            "/api/v1/clients/{id}/ips",
            get(onlines::get_client_ips)
                .route_layer(middleware::api_key::scope_layer(scopes::ONLINES_READ)),
        )
        // Webhooks
        .route(
            "/api/v1/webhooks",
            get(webhook::list_webhooks)
                .route_layer(middleware::api_key::scope_layer(scopes::WEBHOOKS_READ)),
        )
        .route(
            "/api/v1/webhooks",
            post(webhook::create_webhook)
                .route_layer(middleware::api_key::scope_layer(scopes::WEBHOOKS_WRITE)),
        )
        .route(
            "/api/v1/webhooks/{id}",
            put(webhook::update_webhook)
                .route_layer(middleware::api_key::scope_layer(scopes::WEBHOOKS_WRITE)),
        )
        .route(
            "/api/v1/webhooks/{id}",
            delete(webhook::delete_webhook)
                .route_layer(middleware::api_key::scope_layer(scopes::WEBHOOKS_WRITE)),
        )
        // Logs
        .route(
            "/api/v1/logs",
            get(logs::query_logs).route_layer(middleware::api_key::scope_layer(scopes::LOGS_READ)),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::api_key::require_api_key,
        ))
        .with_state(state.clone());

    // Admin API routes (JWT required)
    let admin_api = Router::new()
        .route("/api/v1/me", get(login::me))
        .route("/api/v1/users", post(login::create_user))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::require_auth,
        ))
        .with_state(state.clone());

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/metrics", get(metrics_export::prometheus_metrics))
        .route("/sub/{token}", get(subscription::serve_subscription))
        .route("/sub/{token}/qr", get(subscription::serve_subscription_qr))
        .route("/api/v1/login", post(login::login))
        .with_state(state.clone());

    let cors_layer = build_cors_layer(hub_config);

    Router::new()
        .merge(protected_api)
        .merge(admin_api)
        .merge(public_routes)
        .fallback_service(static_service)
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer)
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

    let overrides = ConfigOverrides {
        listen: Some(args.listen.clone()),
        grpc_listen: Some(args.grpc_listen.clone()),
        database_url: args.database_url,
        static_dir: Some(args.static_dir.clone()),
        auto_register_agents: args.auto_register_agents,
    };

    let mut hub_config = HubConfig::load(PathBuf::from(&args.config).as_path(), overrides)?;

    // CLI args override TLS paths (higher priority than config file)
    if args.http_tls_cert.is_some() {
        hub_config.http_tls_cert = args.http_tls_cert;
    }
    if args.http_tls_key.is_some() {
        hub_config.http_tls_key = args.http_tls_key;
    }

    if hub_config.database_url.is_empty() {
        anyhow::bail!(
            "database_url is required. Set PROXYPANEL_DATABASE_URL or add database_url to {}.",
            args.config
        );
    }

    tracing::info!("starting ProxyPanel Hub with config: {}", args.config);

    let db = pp_db::init_db(&hub_config.database_url).await?;
    pp_db::run_migrations(&db).await?;
    pp_db::upgrade::run_versioned_upgrades(&db, env!("CARGO_PKG_VERSION")).await?;

    if let Err(e) = ensure_bootstrap_api_key(&db).await {
        tracing::warn!("failed to ensure bootstrap api key: {}", e);
    }

    let state = AppState::new(
        db,
        hub_config.clone(),
        RateLimiter::default(),
        Some(Arc::new(metrics_export::MetricsHandle::new())),
    );

    // Start periodic background scheduler
    let scheduler_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(e) = crate::service::scheduler::run_periodic_checks(&scheduler_state).await {
                tracing::error!("periodic check failed: {}", e);
            }
        }
    });

    let app = build_app(state.clone(), &hub_config);

    let http_addr: SocketAddr = hub_config.listen.parse()?;
    let grpc_addr: SocketAddr = hub_config.grpc_listen.parse()?;

    let grpc_tls = grpc_tls_config(&args.grpc_tls_cert, &args.grpc_tls_key).await?;

    if grpc_tls.is_some() {
        tracing::info!("gRPC listening on https://{} (TLS enabled)", grpc_addr);
    } else {
        tracing::warn!(
            "gRPC listening on http://{} (TLS disabled - not recommended in production)",
            grpc_addr
        );
    }

    if hub_config.auto_register_agents {
        tracing::warn!(
            "AGENT AUTO-REGISTER is enabled. Any agent with a valid token can register as a new node. Do NOT use this in production."
        );
    }

    // Build HTTP server (plain or TLS)
    let http_server: Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> =
        if let (Some(cert_path), Some(key_path)) =
            (&hub_config.http_tls_cert, &hub_config.http_tls_key)
        {
            tracing::info!("HTTP API listening on https://{} (TLS enabled)", http_addr);
            let cert = tokio::fs::read(cert_path).await?;
            let key = tokio::fs::read(key_path).await?;

            let cert_chain = rustls_pemfile::certs(&mut cert.as_slice())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!("failed to parse HTTP TLS cert: {}", e))?;
            let mut keys = rustls_pemfile::pkcs8_private_keys(&mut key.as_slice())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!("failed to parse HTTP TLS key: {}", e))?;

            if keys.is_empty() {
                anyhow::bail!("no private keys found in HTTP TLS key file");
            }

            let server_config = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    cert_chain.into_iter().collect(),
                    rustls::pki_types::PrivateKeyDer::from(keys.remove(0)),
                )
                .map_err(|e| anyhow::anyhow!("failed to build HTTP TLS config: {}", e))?;

            let acceptor = std::sync::Arc::new(server_config);
            let listener = tokio::net::TcpListener::bind(http_addr).await?;

            Box::pin(async move {
                let shutdown = shutdown_signal();
                futures_util::pin_mut!(shutdown);
                loop {
                    tokio::select! {
                        _ = &mut shutdown => {
                            tracing::info!("HTTP server stopped by shutdown signal");
                            return Ok(());
                        }
                        result = listener.accept() => {
                            let (stream, _) = result?;
                            let app = app.clone();
                            let acceptor = acceptor.clone();
                            tokio::spawn(async move {
                                let tls_acceptor = tokio_rustls::TlsAcceptor::from(acceptor);
                                let tls_stream = match tls_acceptor.accept(stream).await {
                                    Ok(s) => s,
                                    Err(e) => {
                                        tracing::warn!("TLS handshake error: {}", e);
                                        return;
                                    }
                                };
                                let io = hyper_util::rt::TokioIo::new(tls_stream);
                                let svc = hyper::service::service_fn(move |req| {
                                    let mut app = app.clone();
                                    async move { app.call(req).await }
                                });
                                if let Err(e) = hyper::server::conn::http1::Builder::new()
                                    .serve_connection(io, svc)
                                    .await
                                {
                                    tracing::warn!("HTTPS connection error: {}", e);
                                }
                            });
                        }
                    }
                }
            })
        } else {
            tracing::info!("HTTP API listening on http://{}", http_addr);
            let http_listener = tokio::net::TcpListener::bind(http_addr).await?;
            Box::pin(async move {
                axum::serve(http_listener, app)
                    .with_graceful_shutdown(shutdown_signal())
                    .await?;
                Ok(())
            })
        };

    // Build gRPC server
    let grpc_state = state.clone();
    let grpc_server: Pin<Box<dyn Future<Output = Result<(), tonic::transport::Error>> + Send>> =
        if let Some(config) = grpc_tls {
            Box::pin(
                tonic::transport::Server::builder()
                    .tls_config(config)?
                    .add_service(pp_proto::hub_agent_server::HubAgentServer::new(
                        HubAgentService::new(grpc_state),
                    ))
                    .serve(grpc_addr),
            )
        } else {
            Box::pin(
                tonic::transport::Server::builder()
                    .add_service(pp_proto::hub_agent_server::HubAgentServer::new(
                        HubAgentService::new(grpc_state),
                    ))
                    .serve(grpc_addr),
            )
        };

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

fn build_cors_layer(config: &HubConfig) -> CorsLayer {
    match config.cors_allowed_origins() {
        Some(origins) if !origins.is_empty() => {
            let mut layer = CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    header::AUTHORIZATION,
                    header::CONTENT_TYPE,
                    header::ACCEPT,
                    header::HeaderName::from_static("x-api-key"),
                ]);
            layer = layer.allow_credentials(true);
            layer
        }
        _ => {
            if cfg!(debug_assertions) {
                CorsLayer::permissive()
            } else {
                tracing::warn!("No CORS origins configured. Allowing same-origin requests only.");
                CorsLayer::new()
            }
        }
    }
}

async fn grpc_tls_config(
    cert_path: &Option<PathBuf>,
    key_path: &Option<PathBuf>,
) -> anyhow::Result<Option<tonic::transport::ServerTlsConfig>> {
    match (cert_path, key_path) {
        (Some(cert), Some(key)) => {
            let cert = tokio::fs::read(cert).await?;
            let key = tokio::fs::read(key).await?;
            let identity = tonic::transport::Identity::from_pem(&cert, &key);
            Ok(Some(
                tonic::transport::ServerTlsConfig::new().identity(identity),
            ))
        }
        (None, None) => Ok(None),
        _ => anyhow::bail!("both --grpc-tls-cert and --grpc-tls-key must be provided together"),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
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

/// Ensure at least one API key exists on a fresh database.
/// - Honors `PROXYPANEL_BOOTSTRAP_API_KEY` or `PROXYPANEL_BOOTSTRAP_API_KEY_FILE` if set.
/// - Otherwise generates a one-time key and prints it to stderr (not logs).
async fn ensure_bootstrap_api_key(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

    let existing = api_key_entity::Entity::find()
        .filter(api_key_entity::Column::IsActive.eq(true))
        .one(db)
        .await?;

    if existing.is_some() {
        return Ok(());
    }

    let raw_key = if let Ok(path) = std::env::var("PROXYPANEL_BOOTSTRAP_API_KEY_FILE") {
        tokio::fs::read_to_string(path).await?.trim().to_string()
    } else {
        std::env::var("PROXYPANEL_BOOTSTRAP_API_KEY")
            .unwrap_or_else(|_| pp_common::generate_secure_token())
    };

    if raw_key.is_empty() {
        anyhow::bail!("bootstrap API key is empty");
    }

    let key_hash = pp_common::hash_secret_async(raw_key.clone())
        .await
        .map_err(|e| anyhow::anyhow!("failed to hash bootstrap key: {}", e))?;

    let active = api_key_entity::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        name: Set("bootstrap".to_string()),
        key_hash: Set(key_hash),
        scopes: Set(serde_json::json!(["*"])),
        ip_allowlist: Set(None),
        rate_limit: Set(None),
        expires_at: Set(None),
        is_active: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };
    active.insert(db).await?;

    eprintln!("=================================================================");
    eprintln!("BOOTSTRAP API KEY created (one-time, not logged):");
    eprintln!("  {}", raw_key);
    eprintln!("Store it securely and rotate via /api/v1/api-keys.");
    eprintln!("=================================================================");

    Ok(())
}

#[cfg(test)]
#[path = "integration_tests.rs"]
mod integration_tests;
