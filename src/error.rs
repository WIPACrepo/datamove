// error.rs

pub type OtherError = Box<dyn std::error::Error + Send + Sync>;

pub type Result<T> = core::result::Result<T, DatamoveError>;

#[derive(thiserror::Error, Debug)]
pub enum DatamoveError {
    #[error("E-Mail Address error: {0}")]
    AddressError(#[from] lettre::address::AddressError),

    #[error("Checksum error:\nExpected:{expected}\nActual:  {actual}")]
    ChecksumError { expected: String, actual: String },

    #[error("Critical system error occurred: {0}")]
    Critical(String),

    #[error("Database Error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("I/O Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON Serialization Error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("E-Mail error: {0}")]
    MailError(#[from] lettre::error::Error),

    #[error("Template Error: {0}")]
    TemplateError(#[from] tera::Error),

    #[error("Time Error (ComponentRange): {0}")]
    TimeError(#[from] time::error::ComponentRange),

    #[error("SMTP Transport error: {0}")]
    TransportError(#[from] lettre::transport::smtp::Error),

    #[error(transparent)]
    Other(#[from] OtherError),
}
