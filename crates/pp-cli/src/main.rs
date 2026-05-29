//! ProxyPanel CLI — Management utilities.

use clap::{Parser, Subcommand};

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
    /// Database diagnostic check
    Diagnose {
        #[arg(long, env = "PROXYPANEL_DATABASE_URL")]
        database_url: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        Commands::Diagnose { database_url } => {
            println!("Checking database connection...");
            let db = pp_db::init_db(&database_url).await?;
            db.ping().await?;
            println!("Database connection: OK");
        }
    }

    Ok(())
}
