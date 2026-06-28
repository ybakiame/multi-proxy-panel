//! ProxyPanel CLI — Management utilities.

use clap::{Parser, Subcommand};
use sea_orm::ActiveModelTrait;

#[derive(Parser, Debug)]
#[command(name = "proxy-panel", version, about = "ProxyPanel management CLI")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize database and run migrations
    InitDb {
        #[arg(long, env = "PROXYPANEL_DATABASE_URL")]
        database_url: String,
    },
    /// Generate a secure random token
    GenToken,
    /// Generate agent registration token
    AgentToken {
        #[arg(long)]
        node_name: String,
    },
    /// Create an API key directly in the database
    CreateApiKey {
        #[arg(long, env = "PROXYPANEL_DATABASE_URL")]
        database_url: String,
        #[arg(long, default_value = "cli-admin")]
        name: String,
        #[arg(long, value_delimiter = ',', default_value = "*")]
        scopes: Vec<String>,
    },
    /// Provision a node with a secure agent token
    ProvisionNode {
        #[arg(long, env = "PROXYPANEL_DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        hostname: Option<String>,
        #[arg(long)]
        address: Option<String>,
    },
    /// Database diagnostic check
    Diagnose {
        #[arg(long, env = "PROXYPANEL_DATABASE_URL")]
        database_url: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();

    match args.command {
        Commands::InitDb { database_url } => {
            println!("Initializing database...");
            let db = pp_db::init_db(&database_url).await?;
            pp_db::run_migrations(&db).await?;
            println!("Database initialized and migrations applied.");
        }
        Commands::GenToken => {
            let token = pp_common::generate_secure_token();
            println!("{}", token);
        }
        Commands::AgentToken { node_name } => {
            let token = pp_common::generate_secure_token();
            println!("Agent token for '{}': {}", node_name, token);
            println!("Store this token securely — it will not be shown again.");
        }
        Commands::CreateApiKey {
            database_url,
            name,
            scopes,
        } => {
            let db = pp_db::init_db(&database_url).await?;
            let raw_key = format!("ck_{}", uuid::Uuid::new_v4().simple());
            let key_hash = pp_common::hash_secret(&raw_key)
                .map_err(|e| anyhow::anyhow!("failed to hash API key: {}", e))?;

            use sea_orm::Set;
            use pp_db::entities::api_key;
            let active = api_key::ActiveModel {
                id: Set(uuid::Uuid::new_v4()),
                name: Set(name),
                key_hash: Set(key_hash),
                scopes: Set(serde_json::json!(scopes)),
                ip_allowlist: Set(None),
                rate_limit: Set(None),
                expires_at: Set(None),
                is_active: Set(true),
                created_at: Set(chrono::Utc::now().into()),
                updated_at: Set(chrono::Utc::now().into()),
            };
            active.insert(&db).await?;
            println!("{}", raw_key);
        }
        Commands::ProvisionNode {
            database_url,
            name,
            hostname,
            address,
        } => {
            let db = pp_db::init_db(&database_url).await?;
            let raw_token = pp_common::generate_secure_token();
            let token_hash = pp_common::hash_secret(&raw_token)
                .map_err(|e| anyhow::anyhow!("failed to hash agent token: {}", e))?;

            use sea_orm::Set;
            use pp_db::entities::node;
            let active = node::ActiveModel {
                id: Set(uuid::Uuid::new_v4()),
                name: Set(name),
                hostname: Set(hostname.unwrap_or_default()),
                address: Set(address.unwrap_or_default()),
                token_hash: Set(token_hash),
                cores_available: Set(serde_json::json!([])),
                labels: Set(None),
                usage_coefficient: Set(1.0),
                status: Set("connecting".to_string()),
                parent_id: Set(None),
                last_seen_at: Set(None),
                created_at: Set(chrono::Utc::now().into()),
                updated_at: Set(chrono::Utc::now().into()),
            };
            let inserted = active.insert(&db).await?;
            println!("node_id: {}", inserted.id);
            println!("token: {}", raw_token);
        }
        Commands::Diagnose { database_url } => {
            println!("Checking database connection...");
            let db = pp_db::init_db(&database_url).await?;
            db.ping().await?;
            println!("Database connection: OK");
        }
    }

    Ok(())
}
