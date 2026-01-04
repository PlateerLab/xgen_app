//! Background Services
//!
//! Long-running services for inference, MCP config, and model management.
//!
//! ## Architecture (mistral.rs centric)
//! - InferenceEngine: Manages model loading and generation via mistral.rs
//! - McpConfigManager: Manages MCP server configurations (actual MCP client via mistralrs_mcp)
//! - ModelManager: Manages local model files (download, list, delete)

pub mod inference;
pub mod mcp_manager;
pub mod model_manager;

pub use inference::{
    EmbedRequest, EmbedResponse, GenerateRequest, GenerateResponse, InferenceEngine, ModelConfig,
    ModelStatus,
};
pub use mcp_manager::{McpConfigManager, McpConnectionType, McpServerConfig, McpServerStatus};
pub use model_manager::{ModelInfo, ModelManager, ModelType};
