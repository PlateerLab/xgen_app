//! Background Services
//!
//! Long-running services for model management and sidecar processes.

pub mod model_manager;
pub mod sidecar_manager;

pub use model_manager::{ModelInfo, ModelManager, ModelType};
pub use sidecar_manager::{SidecarConfig, SidecarManager, SidecarStatus};
