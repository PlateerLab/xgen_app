//! Tauri Commands
//!
//! All IPC commands exposed to the frontend.
//!
//! - gpu: System hardware info
//! - mode: Standalone/Connected mode switching
//! - model: Local model file management
//! - proxy: Local LLM proxy for Connected mode
//! - settings: Persistent app settings (config file)
//! - sidecar: Python sidecar process management

pub mod gpu;
pub mod mode;
pub mod model;
pub mod proxy;
pub mod settings;
pub mod sidecar;

// Re-export all commands
pub use gpu::*;
pub use mode::*;
pub use model::*;
pub use proxy::*;
pub use settings::*;
pub use sidecar::*;
