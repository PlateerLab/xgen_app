//! XGEN Application State
//!
//! Manages global application state including hardware info, models, and mode.
//!
//! ## Architecture (mistral.rs centric)
//! - GPU detection: Simple system info (mistral.rs handles device mapping)
//! - Inference: mistral.rs with automatic device selection
//! - MCP: mistralrs_mcp client (configuration managed here)

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::gpu::SystemInfo;
use crate::services::{InferenceEngine, McpConfigManager, ModelManager};

/// Global application state shared across all Tauri commands
pub struct AppState {
    /// System hardware information
    pub system_info: Arc<RwLock<Option<SystemInfo>>>,

    /// Model manager for downloading and managing models
    pub model_manager: Arc<RwLock<ModelManager>>,

    /// LLM inference engine (mistral.rs)
    pub inference_engine: Arc<RwLock<InferenceEngine>>,

    /// MCP server configuration manager
    pub mcp_config: Arc<RwLock<McpConfigManager>>,

    /// Current application mode (Standalone or Connected)
    pub app_mode: Arc<RwLock<AppMode>>,

    /// Gateway URL for Connected mode
    pub gateway_url: Arc<RwLock<Option<String>>>,
}

/// Application operation mode
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum AppMode {
    /// Fully offline mode with local LLM
    #[default]
    Standalone,

    /// Connected to xgen-backend-gateway
    Connected {
        server_url: String,
    },
}

impl AppState {
    /// Create a new AppState with default values
    pub fn new() -> Self {
        Self {
            system_info: Arc::new(RwLock::new(None)),
            model_manager: Arc::new(RwLock::new(ModelManager::new())),
            inference_engine: Arc::new(RwLock::new(InferenceEngine::new())),
            mcp_config: Arc::new(RwLock::new(McpConfigManager::with_defaults())),
            app_mode: Arc::new(RwLock::new(AppMode::default())),
            gateway_url: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if app is in standalone mode
    pub async fn is_standalone(&self) -> bool {
        matches!(*self.app_mode.read().await, AppMode::Standalone)
    }

    /// Check if app is in connected mode
    pub async fn is_connected(&self) -> bool {
        matches!(*self.app_mode.read().await, AppMode::Connected { .. })
    }

    /// Get the server URL if in connected mode
    pub async fn get_server_url(&self) -> Option<String> {
        match &*self.app_mode.read().await {
            AppMode::Connected { server_url } => Some(server_url.clone()),
            AppMode::Standalone => None,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
