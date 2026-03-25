//! XGEN Application State
//!
//! Manages global application state including hardware info, models, mode, and sidecars.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::gpu::SystemInfo;
use crate::services::{ModelManager, SidecarManager};
use crate::services::llm_client::ChatMessage;

/// Global application state shared across all Tauri commands
pub struct AppState {
    /// System hardware information
    pub system_info: Arc<RwLock<Option<SystemInfo>>>,

    /// Model manager for downloading and managing models
    pub model_manager: Arc<RwLock<ModelManager>>,

    /// Sidecar process manager (xgen-workflow, etc.)
    pub sidecar_manager: Arc<RwLock<SidecarManager>>,

    /// Current application mode (Standalone or Service)
    pub app_mode: Arc<RwLock<AppMode>>,

    /// Gateway URL for Connected mode
    pub gateway_url: Arc<RwLock<Option<String>>>,

    /// CLI session state
    pub cli_session: Arc<RwLock<CliSession>>,
}

/// AI CLI session state
pub struct CliSession {
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
    pub xgen_token: Option<String>,
}

impl CliSession {
    pub fn new() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            messages: Vec::new(),
            xgen_token: None,
        }
    }

    pub fn clear(&mut self) {
        self.session_id = uuid::Uuid::new_v4().to_string();
        self.messages.clear();
    }
}

/// Application operation mode
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum AppMode {
    /// Standalone mode
    #[default]
    Standalone,

    /// Service mode using Python sidecar (xgen-workflow)
    Service {
        /// URL of the running service (e.g., http://127.0.0.1:8001)
        service_url: String,
    },

    /// Connected to external xgen-backend-gateway
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
            sidecar_manager: Arc::new(RwLock::new(SidecarManager::new())),
            app_mode: Arc::new(RwLock::new(AppMode::default())),
            gateway_url: Arc::new(RwLock::new(None)),
            cli_session: Arc::new(RwLock::new(CliSession::new())),
        }
    }

    /// Check if app is in standalone mode
    pub async fn is_standalone(&self) -> bool {
        matches!(*self.app_mode.read().await, AppMode::Standalone)
    }

    /// Check if app is in service mode (using sidecar)
    pub async fn is_service_mode(&self) -> bool {
        matches!(*self.app_mode.read().await, AppMode::Service { .. })
    }

    /// Check if app is in connected mode
    pub async fn is_connected(&self) -> bool {
        matches!(*self.app_mode.read().await, AppMode::Connected { .. })
    }

    /// Get the server URL if in connected mode
    pub async fn get_server_url(&self) -> Option<String> {
        match &*self.app_mode.read().await {
            AppMode::Connected { server_url } => Some(server_url.clone()),
            AppMode::Service { service_url } => Some(service_url.clone()),
            AppMode::Standalone => None,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
