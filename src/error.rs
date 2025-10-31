// Error types for the Crash application
// Provides comprehensive error handling with context-rich messages

use thiserror::Error;

/// Main error type for the Crash application
#[derive(Error, Debug)]
pub enum CrashError {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Process management errors
    #[error("Process error: {0}")]
    Process(String),

    /// Download-related errors
    #[error("Download error: {0}")]
    Download(String),

    /// Platform-specific operation errors
    #[error("Platform error: {0}")]
    Platform(String),

    /// Logging system errors
    #[error("Logging error: {0}")]
    Log(String),

    /// I/O errors from standard library
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// HTTP request errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Parse errors for integers
    #[error("Parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    /// UTF-8 conversion errors
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, CrashError>;
