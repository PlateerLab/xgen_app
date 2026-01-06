//! Background Services
//!
//! Long-running services for inference, MCP config, model management, and sidecar processes.
//!
//! ## Architecture (mistral.rs centric)
//! - InferenceEngine: Manages model loading and generation via mistral.rs
//! - McpConfigManager: Manages MCP server configurations (actual MCP client via mistralrs_mcp)
//! - ModelManager: Manages local model files (download, list, delete)
//! - SidecarManager: Manages Python sidecar processes (xgen-workflow, etc.)

pub mod inference;
pub mod mcp_manager;
pub mod model_manager;
pub mod sidecar_manager;

pub use inference::{
    EmbedRequest, EmbedResponse, GenerateRequest, GenerateResponse, InferenceEngine, ModelConfig,
    ModelStatus,
};
pub use mcp_manager::{McpConfigManager, McpConnectionType, McpServerConfig, McpServerStatus};
pub use model_manager::{ModelInfo, ModelManager, ModelType};
pub use sidecar_manager::{SidecarConfig, SidecarManager, SidecarStatus};
