//! Hardware Detection Module
//!
//! Provides system hardware information.
//! Note: GPU detection and device mapping is handled by mistral.rs automatically.

pub mod detection;

pub use detection::{
    cuda_hint_available, get_backend_hint, get_hardware_status, get_system_info, metal_available,
    HardwareStatus, InferenceBackendHint, Platform, SystemInfo,
};
