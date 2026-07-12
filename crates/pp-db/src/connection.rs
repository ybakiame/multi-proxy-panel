//! Database connection abstraction supporting multiple backends.

use sea_orm::{Database, DatabaseConnection, DbErr};
use tracing::info;

/// Supported database backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseBackend {
    PostgreSQL,
    SQLite,
    #[allow(dead_code)]
    Turso, // Reserved for future libSQL support
}

/// Parse database URL and determine backend.
pub fn detect_backend(database_url: &str) -> DatabaseBackend {
    if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
        DatabaseBackend::PostgreSQL
    } else if database_url.starts_with("sqlite://") || database_url.starts_with("sqlite:") {
        DatabaseBackend::SQLite
    } else if database_url.starts_with("libsql://") || database_url.starts_with("http://") {
        DatabaseBackend::Turso
    } else {
        // Default fallback
        DatabaseBackend::SQLite
    }
}

/// Connect to database with backend-specific settings.
pub async fn connect(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let backend = detect_backend(database_url);
    info!("detected database backend: {:?}", backend);

    match backend {
        DatabaseBackend::PostgreSQL => {
            info!("connecting to PostgreSQL");
            Database::connect(database_url).await
        }
        DatabaseBackend::SQLite => {
            info!("connecting to SQLite");
            // Ensure SQLite uses WAL mode for better concurrency
            let conn = Database::connect(database_url).await?;
            let pragmas = [
                "PRAGMA journal_mode = WAL;",
                "PRAGMA synchronous = NORMAL;",
                "PRAGMA busy_timeout = 5000;",
            ];
            for pragma in &pragmas {
                let _ = sea_orm::ConnectionTrait::execute(
                    &conn,
                    sea_orm::Statement::from_string(
                        sea_orm::DatabaseBackend::Sqlite,
                        pragma.to_string(),
                    ),
                )
                .await;
            }
            Ok(conn)
        }
        DatabaseBackend::Turso => {
            // TODO: Implement Turso/libSQL connection using libsql crate
            // For now, fall back to SQLite local
            tracing::warn!("Turso backend not yet fully implemented, falling back to SQLite");
            let conn = Database::connect(database_url).await?;
            let _ = sea_orm::ConnectionTrait::execute(
                &conn,
                sea_orm::Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    "PRAGMA busy_timeout = 5000;".to_string(),
                ),
            )
            .await;
            Ok(conn)
        }
    }
}

/// Returns true if the backend supports `gen_random_uuid()` or equivalent.
pub fn supports_native_uuid(backend: DatabaseBackend) -> bool {
    matches!(backend, DatabaseBackend::PostgreSQL)
}
