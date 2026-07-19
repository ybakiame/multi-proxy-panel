//! ProxyPanel Agent — Runs on each node, manages xray/sing-box cores.

use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing_subscriber::prelude::*;

mod client;
mod logger;
mod persist;
mod reporter;

use client::AgentStreamClient;
use logger::AgentLogger;

#[derive(Parser, Debug)]
#[command(name = "proxy-panel-agent", version, about = "ProxyPanel Node Agent")]
struct Args {
    #[arg(long, env = "PROXYPANEL_HUB_URL")]
    hub_url: String,

    #[arg(long, env = "PROXYPANEL_AGENT_TOKEN")]
    token: Option<String>,

    #[arg(long, env = "PROXYPANEL_AGENT_NAME", default_value = "")]
    name: String,

    #[arg(long, env = "PROXYPANEL_AGENT_ID")]
    agent_id: Option<String>,

    #[arg(long, default_value = "/var/lib/proxy-panel/agent")]
    data_dir: PathBuf,

    #[arg(long, default_value = "/usr/local/bin")]
    bin_dir: PathBuf,

    #[arg(long, env = "PROXYPANEL_AGENT_TLS_CA")]
    tls_ca: Option<PathBuf>,

    #[arg(long, env = "PROXYPANEL_AGENT_TLS_DOMAIN")]
    tls_domain: Option<String>,

    #[arg(long, env = "PROXYPANEL_AGENT_DOMAIN")]
    domain: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent_logger = AgentLogger::new();
    let log_sender = agent_logger.sender();

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "proxy_panel_agent=info".into());
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .finish()
        .with(logger::GrpcLogLayer::new(log_sender))
        .init();

    let args = Args::parse();
    tracing::info!("ProxyPanel Agent starting. Hub = {}", args.hub_url);

    // Ensure data directory exists
    tokio::fs::create_dir_all(&args.data_dir).await?;

    let supervisor = Arc::new(pp_core::CoreSupervisor::new());
    let discovered = supervisor.discover(&args.bin_dir, &args.data_dir).await?;
    tracing::info!("discovered cores: {:?}", discovered);

    restore_last_config(&supervisor, &args.data_dir).await;

    // Load or generate agent token
    let token = load_or_register_token(&args.data_dir, args.token).await?;

    // Load or generate a stable agent id so restarts don't create duplicate nodes.
    let agent_id = load_or_register_agent_id(&args.data_dir, args.agent_id).await?;

    // Start gRPC stream client with reconnect loop
    let hostname = if args.name.is_empty() {
        gethostname::gethostname()
            .into_string()
            .unwrap_or_else(|_| "unknown".to_string())
    } else {
        args.name
    };

    let tls_config = build_tls_config(args.tls_ca, args.tls_domain).await?;
    let mut client = AgentStreamClient::new(
        agent_id,
        token,
        hostname,
        args.domain.unwrap_or_default(),
        tls_config,
        agent_logger,
        args.data_dir.clone(),
    );

    tokio::select! {
        res = client.run(args.hub_url, supervisor.clone()) => {
            if let Err(e) = res {
                tracing::error!("client error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutdown signal received");
        }
    }

    tracing::info!("ProxyPanel Agent shutdown complete");
    Ok(())
}

/// Restart every core recorded in the per-core config snapshots.
///
/// Runs before connecting to the Hub so the node serves traffic even when
/// the Hub is temporarily unreachable. Failures are logged, never fatal —
/// the Hub can still push a fresh config later.
async fn restore_last_config(supervisor: &pp_core::CoreSupervisor, data_dir: &Path) {
    for (core_type, snapshot) in persist::load_last_configs(data_dir).await {
        let manager = match supervisor.get(core_type).await {
            Some(manager) => manager,
            None => match supervisor
                .ensure_manager_from_discovered(core_type, None)
                .await
            {
                Ok(manager) => manager,
                Err(e) => {
                    tracing::warn!("cannot restore {:?}: {}", core_type, e);
                    continue;
                }
            },
        };

        if manager.is_running().await {
            tracing::info!("{:?} already running, skipping config restore", core_type);
            continue;
        }

        match manager.start(&snapshot.config).await {
            Ok(()) => tracing::info!("restored {:?} from last applied config", core_type),
            Err(e) => tracing::warn!("failed to restore {:?} from snapshot: {}", core_type, e),
        }
    }
}

async fn load_or_register_token(
    data_dir: &Path,
    provided: Option<String>,
) -> anyhow::Result<String> {
    let token_path = data_dir.join(".agent_token");

    if let Some(token) = provided {
        tokio::fs::write(&token_path, &token).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&token_path).await?.permissions();
            perms.set_mode(0o600);
            tokio::fs::set_permissions(&token_path, perms).await?;
        }
        return Ok(token);
    }

    if token_path.exists() {
        let token = tokio::fs::read_to_string(&token_path).await?;
        return Ok(token.trim().to_string());
    }

    let token = pp_common::generate_secure_token();
    tokio::fs::write(&token_path, &token).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&token_path).await?.permissions();
        perms.set_mode(0o600);
        tokio::fs::set_permissions(&token_path, perms).await?;
    }
    tracing::info!("generated new agent token (auto-register mode)");
    Ok(token)
}

async fn load_or_register_agent_id(
    data_dir: &Path,
    provided: Option<String>,
) -> anyhow::Result<String> {
    let id_path = data_dir.join(".agent_id");

    if let Some(id) = provided {
        tokio::fs::write(&id_path, &id).await?;
        return Ok(id);
    }

    if id_path.exists() {
        let id = tokio::fs::read_to_string(&id_path).await?;
        let id = id.trim().to_string();
        if !id.is_empty() {
            return Ok(id);
        }
    }

    let id = pp_common::generate_uuid();
    tokio::fs::write(&id_path, &id).await?;
    tracing::info!("generated new agent_id: {}", id);
    Ok(id)
}

async fn build_tls_config(
    ca_path: Option<PathBuf>,
    domain: Option<String>,
) -> anyhow::Result<Option<tonic::transport::ClientTlsConfig>> {
    match (ca_path, domain) {
        (Some(path), domain) => {
            let ca = tokio::fs::read(&path).await?;
            let cert = tonic::transport::Certificate::from_pem(&ca);
            let mut tls = tonic::transport::ClientTlsConfig::new().ca_certificate(cert);
            if let Some(domain) = domain {
                tls = tls.domain_name(domain);
            }
            Ok(Some(tls))
        }
        (None, Some(domain)) => {
            // Use system root certificates when only a domain is provided.
            let tls = tonic::transport::ClientTlsConfig::new()
                .domain_name(domain)
                .with_native_roots();
            Ok(Some(tls))
        }
        (None, None) => Ok(None),
    }
}
