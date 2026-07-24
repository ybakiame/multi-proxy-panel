//! pp-db — Database layer: Sea-ORM entities, migrations and DAOs.

pub mod connection;
pub mod entities;
pub mod migration;
pub mod upgrade;

use sea_orm::{DatabaseConnection, DbErr};
use tracing::info;

/// Initialize database connection.
pub async fn init_db(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    info!("connecting to database: {}", database_url);
    let conn = connection::connect(database_url).await?;
    info!("database connected");
    Ok(conn)
}

/// Run pending migrations.
pub async fn run_migrations(conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    use sea_orm_migration::MigratorTrait;
    migration::Migrator::up(conn, None).await
}
