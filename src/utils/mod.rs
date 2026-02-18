pub mod logger;
pub mod scheduler;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BsxError {
    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("数据库错误: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("网络请求错误: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("解析错误: {0}")]
    ParseError(String),

    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("翻译API错误: {0}")]
    TranslationError(String),

    #[error("PDF处理错误: {0}")]
    PdfError(String),

    #[error("未知错误: {0}")]
    Unknown(String),
}

pub type BsxResult<T> = Result<T, BsxError>;
