//! ProxyPanel Agent — Runs on each node, manages xray/sing-box cores.

use clap::Parser;
use std::path::PathBuf;

mod client;

use client::AgentStreamClient;

#[derive(Parser, Debug)]
#[command(name = "proxy-panel-agent", version, about = "ProxyPanel Node Agent")]
struct Args {
    #[arg(long, env = "PROXYPANEL_HUB_URL")]
    hub_url: String,

    #[arg(long, env = "PROXYPANEL_AGENT_TOKEN")]
    token: Option<String>,

    #[arg(long, env = "PROXYPANEL_AGENT_NAME", default_value = "")]
    name: String,

    #[arg(long, default_value = "/var/lib/proxy-panel/agent")]
    data_dir: PathBuf,

    #[arg(long, default_value = "/usr/local/bin")]
    bin_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "proxy_panel_agent=info".into()),
        )
        .init();

    let args = Args::parse();
    tracing::info!("ProxyPanel Agent starting. Hub = {}", args.hub_url);

    // Ensure data directory exists
    tokio::fs::create_dir_all(&args.data_dir).await?;

    // Discover available cores
    let supervisor = pp_core::CoreSupervisor::new();
    let discovered = supervisor.discover(&args.data_dir).await?;
    tracing::info!("discovered cores: {:?}", discovered);

    // Load or generate agent token
    let token = load_or_register_token(&args.data_dir, args.token).await?;

    // Start gRPC stream client with reconnect loop
    let hostname = if args.name.is_empty() {
        gethostname::gethostname()
            .into_string()
            .unwrap_or_else(|_| "unknown".to_string())
    } else {
        args.name
    };

    let mut client = AgentStreamClient::new(token, hostname);

    tokio::select! {
        res = client.run(args.hub_url, supervisor) => {
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

async fn load_or_register_token(
    data_dir: &PathBuf,
    provided: Option<String>,
) -> anyhow::Result<String> {
    let token_path = data_dir.join(".agent_token");

    if let Some(token) = provided {
        tokio::fs::write(&token_path, &token).await?;
        return Ok(token);
    }

    if token_path.exists() {
        let token = tokio::fs::read_to_string(&token_path).await?;
        return Ok(token.trim().to_string());
    }

    let token = pp_common::generate_secure_token();
    tokio::fs::write(&token_path, &token).await?;
    tracing::info!("generated new agent token (auto-register mode)");
    Ok(token)
}
