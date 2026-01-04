//! System Hardware Detection
//!
//! Provides basic system hardware information.
//! Note: GPU detection and device mapping is handled by mistral.rs automatically.

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// System hardware information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Operating system name
    pub os_name: String,

    /// Operating system version
    pub os_version: String,

    /// CPU brand/model
    pub cpu_brand: String,

    /// Number of CPU cores
    pub cpu_cores: usize,

    /// Total RAM in bytes
    pub total_memory: u64,

    /// Available RAM in bytes
    pub available_memory: u64,

    /// Detected platform (for mistral.rs backend selection)
    pub platform: Platform,
}

/// Platform type for inference backend selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Platform {
    /// macOS (Apple Silicon or Intel)
    MacOS,
    /// Windows
    Windows,
    /// Linux
    Linux,
    /// Unknown platform
    Unknown,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::MacOS => write!(f, "macOS"),
            Platform::Windows => write!(f, "Windows"),
            Platform::Linux => write!(f, "Linux"),
            Platform::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Inference backend hint (actual selection done by mistral.rs)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InferenceBackendHint {
    /// NVIDIA CUDA (requires CUDA toolkit)
    Cuda,
    /// Apple Metal (macOS only)
    Metal,
    /// CPU with optimizations
    Cpu,
}

impl std::fmt::Display for InferenceBackendHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InferenceBackendHint::Cuda => write!(f, "CUDA"),
            InferenceBackendHint::Metal => write!(f, "Metal"),
            InferenceBackendHint::Cpu => write!(f, "CPU"),
        }
    }
}

/// Get current platform
fn get_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::MacOS
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else {
        Platform::Unknown
    }
}

/// Check if CUDA might be available (hint only, mistral.rs does actual detection)
pub fn cuda_hint_available() -> bool {
    // Check for CUDA environment variable
    if std::env::var("CUDA_PATH").is_ok() {
        return true;
    }

    // Check common CUDA installation paths
    let cuda_paths = [
        "/usr/local/cuda",
        "/opt/cuda",
        "C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA",
    ];

    for path in cuda_paths {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }

    false
}

/// Check if Metal is available (macOS only)
pub fn metal_available() -> bool {
    cfg!(target_os = "macos")
}

/// Get recommended backend hint
/// Note: mistral.rs handles actual device selection automatically
pub fn get_backend_hint() -> InferenceBackendHint {
    if metal_available() {
        InferenceBackendHint::Metal
    } else if cuda_hint_available() {
        InferenceBackendHint::Cuda
    } else {
        InferenceBackendHint::Cpu
    }
}

/// Get system hardware information
pub fn get_system_info() -> Result<SystemInfo> {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_all();

    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());

    let cpu_brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let cpu_cores = sys.cpus().len();
    let total_memory = sys.total_memory();
    let available_memory = sys.available_memory();
    let platform = get_platform();

    Ok(SystemInfo {
        os_name,
        os_version,
        cpu_brand,
        cpu_cores,
        total_memory,
        available_memory,
        platform,
    })
}

/// Hardware status for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareStatus {
    /// System information
    pub system: SystemInfo,

    /// Whether CUDA might be available (hint)
    pub cuda_hint: bool,

    /// Whether Metal is available
    pub metal_available: bool,

    /// Recommended backend hint
    pub recommended_backend: String,

    /// Note about automatic device selection
    pub note: String,
}

/// Get complete hardware status
pub fn get_hardware_status() -> Result<HardwareStatus> {
    let system = get_system_info()?;
    let cuda_hint = cuda_hint_available();
    let metal_available = metal_available();
    let recommended_backend = get_backend_hint().to_string();

    Ok(HardwareStatus {
        system,
        cuda_hint,
        metal_available,
        recommended_backend,
        note: "mistral.rs handles GPU detection and device mapping automatically".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", Platform::MacOS), "macOS");
        assert_eq!(format!("{}", Platform::Windows), "Windows");
        assert_eq!(format!("{}", Platform::Linux), "Linux");
    }

    #[test]
    fn test_backend_hint_display() {
        assert_eq!(format!("{}", InferenceBackendHint::Cuda), "CUDA");
        assert_eq!(format!("{}", InferenceBackendHint::Metal), "Metal");
        assert_eq!(format!("{}", InferenceBackendHint::Cpu), "CPU");
    }

    #[test]
    fn test_get_system_info() {
        let info = get_system_info().unwrap();
        assert!(info.cpu_cores > 0);
        assert!(info.total_memory > 0);
    }
}
