//! XGEN App Error Types
//!
//! Centralized error handling for the XGEN desktop application.

use thiserror::Error;

/// Application-wide error type
#[derive(Error, Debug)]
pub enum AppError {
    #[error("GPU error: {0}")]
    Gpu(String),

    #[error("Model error: {0}")]
    Model(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Document error: {0}")]
    Document(String),

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Tauri error: {0}")]
    Tauri(String),

    #[error("CLI error: {0}")]
    Cli(String),

    #[error("LLM API error: {0}")]
    LlmApi(String),

    #[error("XGEN API error: {0}")]
    XgenApi(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

// Implement Serialize for Tauri command returns
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Convenience type alias for Results with AppError
pub type Result<T> = std::result::Result<T, AppError>;

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        AppError::Tauri(err.to_string())
    }
}
