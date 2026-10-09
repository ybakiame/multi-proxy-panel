use thiserror::Error;

/// Global error type used across all crates.
#[derive(Error, Debug)]
pub enum PanelError {
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    /// gRPC 错误。tonic::Status 为 176 字节（实测），是 PanelError 唯一的
    /// 大 variant；装箱使 PanelError 降至 64 字节，满足 clippy::result_large_err
    /// （128 字节阈值），各 crate 不再需要 lint 豁免。
    ///
    /// 手工 `From` 实现保持 `?` 转换 ergonomics（`#[from]` 只能生成
    /// `From<Box<Status>>`，会破坏所有 `Result<_, Status>` 的 `?` 用法）。
    #[error("gRPC error: {0}")]
    Grpc(Box<tonic::Status>),

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

    #[error("script error: {0}")]
    Script(String),

    #[error("mitm error: {0}")]
    Mitm(String),

    #[error("client error: {0}")]
    Client(String),

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

impl From<tonic::Status> for PanelError {
    fn from(status: tonic::Status) -> Self {
        Self::Grpc(Box::new(status))
    }
}
