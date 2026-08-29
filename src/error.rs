use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML serialization/deserialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Telegram API error: {0}")]
    Telegram(#[from] teloxide::RequestError),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Database pool error: {0}")]
    DatabasePool(String),

    #[error("Database pool connection error: {0}")]
    PoolError(#[from] diesel_async::pooled_connection::PoolError),

    #[error("Scraper error: {0}")]
    Scraper(String),

    #[error("Email error: {0}")]
    Email(String),

    #[error("SMTP transport error: {0}")]
    Smtp(#[from] lettre::transport::smtp::Error),

    #[error("Email address error: {0}")]
    EmailAddress(#[from] lettre::address::AddressError),

    #[error("Lettre email building error: {0}")]
    LettreEmail(#[from] lettre::error::Error),

    #[allow(dead_code)]
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Storage error: {0}")]
    Storage(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl<E: std::fmt::Debug> From<bb8::RunError<E>> for AppError {
    fn from(err: bb8::RunError<E>) -> Self {
        Self::DatabasePool(format!("{:?}", err))
    }
}
