use thiserror::Error;

/// Global error type used across all crates.
#[derive(Error, Debug)]
pub enum PanelError {
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("authentication error: {0}")]
    Auth(String),

    #[error("authorization error: {0}")]
    Authorization(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("core management error: {0}")]
    Core(String),

    #[error("subscription error: {0}")]
    Subscription(String),

    #[error("traffic error: {0}")]
    Traffic(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("yaml serialization error: {0}")]
    YamlSerialization(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type PanelResult<T> = Result<T, PanelError>;
