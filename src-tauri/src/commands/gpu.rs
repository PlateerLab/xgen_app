//! Hardware Commands
//!
//! Tauri commands for system hardware information.
//! Note: GPU detection and device mapping is handled by mistral.rs automatically.

use std::sync::Arc;
use tauri::State;

use crate::error::Result;
use crate::gpu::{get_hardware_status, HardwareStatus};
use crate::state::AppState;

/// Get system hardware information
///
/// Returns system info including CPU, RAM, and backend hints.
/// Note: mistral.rs handles actual GPU detection and device selection.
#[tauri::command]
pub async fn get_hardware_info(state: State<'_, Arc<AppState>>) -> Result<HardwareStatus> {
    log::info!("Getting hardware information...");

    let status = get_hardware_status()?;

    // Cache system info in state
    {
        let mut sys_info = state.system_info.write().await;
        *sys_info = Some(status.system.clone());
    }

    log::info!(
        "Hardware detection complete: {} cores, {} GB RAM, recommended backend: {}",
        status.system.cpu_cores,
        status.system.total_memory / 1024 / 1024 / 1024,
        status.recommended_backend
    );

    Ok(status)
}
