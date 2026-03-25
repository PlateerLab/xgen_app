//! Background Services
//!
//! Long-running services for model management and sidecar processes.

pub mod model_manager;
pub mod sidecar_manager;
pub mod xgen_api;
pub mod llm_client;
pub mod tool_search;

pub use model_manager::{ModelInfo, ModelManager, ModelType};
pub use sidecar_manager::{SidecarConfig, SidecarManager, SidecarStatus};
pub use xgen_api::XgenApiClient;
pub use llm_client::LlmClient;
