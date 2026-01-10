//! Tauri Commands
//!
//! All IPC commands exposed to the frontend.
//!
//! ## Architecture (mistral.rs centric)
//! - gpu: System hardware info (actual GPU detection by mistral.rs)
//! - llm: Model loading and inference via mistral.rs
//! - mcp: MCP server configuration (actual client via mistralrs_mcp)
//! - mode: Standalone/Connected mode switching
//! - model: Local model file management
//! - proxy: Local LLM proxy for Connected mode
//! - settings: Persistent app settings (config file)
//! - sidecar: Python sidecar process management

pub mod gpu;
pub mod llm;
pub mod mcp;
pub mod mode;
pub mod model;
pub mod proxy;
pub mod settings;
pub mod sidecar;

// Re-export all commands
pub use gpu::*;
pub use llm::*;
pub use mcp::*;
pub use mode::*;
pub use model::*;
pub use proxy::*;
pub use settings::*;
pub use sidecar::*;
