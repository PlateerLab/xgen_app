//! Sidecar Management Commands
//!
//! Commands for managing Python sidecar processes (xgen-workflow, etc.)

use std::collections::HashMap;
use tauri::{AppHandle, State};

use crate::error::Result;
use crate::services::SidecarStatus;
use crate::state::{AppMode, AppState};

/// Start a sidecar service
#[tauri::command]
pub async fn start_sidecar(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    name: String,
    env: Option<HashMap<String, String>>,
) -> Result<SidecarStatus> {
    tracing::info!("Starting sidecar: {}", name);

    let mut manager = state.sidecar_manager.write().await;
    let status = manager.start_sidecar(&app_handle, &name, env).await?;

    tracing::info!("Sidecar {} started on port {}", name, status.port);
    Ok(status)
}

/// Stop a sidecar service
#[tauri::command]
pub async fn stop_sidecar(state: State<'_, AppState>, name: String) -> Result<()> {
    tracing::info!("Stopping sidecar: {}", name);

    let mut manager = state.sidecar_manager.write().await;
    manager.stop_sidecar(&name).await?;

    // If we're in Service mode and this was the active service, switch back to Standalone
    let mut mode = state.app_mode.write().await;
    if matches!(&*mode, AppMode::Service { .. }) {
        *mode = AppMode::Standalone;
        tracing::info!("Switched back to Standalone mode");
    }

    Ok(())
}

/// Stop all running sidecars
#[tauri::command]
pub async fn stop_all_sidecars(state: State<'_, AppState>) -> Result<()> {
    tracing::info!("Stopping all sidecars");

    let mut manager = state.sidecar_manager.write().await;
    manager.stop_all().await?;

    // Switch back to Standalone mode
    let mut mode = state.app_mode.write().await;
    *mode = AppMode::Standalone;

    Ok(())
}

/// Get status of a specific sidecar
#[tauri::command]
pub async fn get_sidecar_status(state: State<'_, AppState>, name: String) -> Result<SidecarStatus> {
    let manager = state.sidecar_manager.read().await;
    manager.get_status(&name).await
}

/// Get status of all sidecars
#[tauri::command]
pub async fn get_all_sidecar_status(state: State<'_, AppState>) -> Result<Vec<SidecarStatus>> {
    let manager = state.sidecar_manager.read().await;
    Ok(manager.get_all_status().await)
}

/// List available sidecars (registered but not necessarily running)
#[tauri::command]
pub async fn list_sidecars(state: State<'_, AppState>) -> Result<Vec<String>> {
    let manager = state.sidecar_manager.read().await;
    Ok(manager.list_sidecars())
}

/// Switch to Service mode (start sidecar and use it)
#[tauri::command]
pub async fn enable_service_mode(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    service_name: String,
    env: Option<HashMap<String, String>>,
) -> Result<SidecarStatus> {
    tracing::info!("Enabling Service mode with: {}", service_name);

    // Start the sidecar
    let mut manager = state.sidecar_manager.write().await;
    let status = manager.start_sidecar(&app_handle, &service_name, env).await?;

    // Wait for service to be healthy
    if !status.health_ok {
        // Give it a few more seconds
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        let updated_status = manager.get_status(&service_name).await?;
        if !updated_status.health_ok {
            tracing::warn!("Service started but health check failed");
        }
    }

    // Update app mode
    let mut mode = state.app_mode.write().await;
    *mode = AppMode::Service {
        service_url: status.url.clone(),
    };

    tracing::info!("Service mode enabled: {}", status.url);
    Ok(status)
}

/// Switch to Standalone mode (stop sidecar if running)
#[tauri::command]
pub async fn enable_standalone_mode(state: State<'_, AppState>) -> Result<()> {
    tracing::info!("Enabling Standalone mode");

    // Stop any running sidecars
    let mut manager = state.sidecar_manager.write().await;
    manager.stop_all().await?;

    // Update app mode
    let mut mode = state.app_mode.write().await;
    *mode = AppMode::Standalone;

    tracing::info!("Standalone mode enabled");
    Ok(())
}

/// Get current app mode info
#[tauri::command]
pub async fn get_current_mode(state: State<'_, AppState>) -> Result<SidecarAppModeInfo> {
    let mode = state.app_mode.read().await;
    let manager = state.sidecar_manager.read().await;

    let (mode_type, service_url, service_name) = match &*mode {
        AppMode::Standalone => ("standalone".to_string(), None, None),
        AppMode::Service { service_url } => {
            // Find which service is running
            let running: Vec<String> = manager
                .get_all_status()
                .await
                .into_iter()
                .filter(|s| s.running)
                .map(|s| s.name)
                .collect();
            (
                "service".to_string(),
                Some(service_url.clone()),
                running.first().cloned(),
            )
        }
        AppMode::Connected { server_url } => {
            ("connected".to_string(), Some(server_url.clone()), None)
        }
    };

    Ok(SidecarAppModeInfo {
        mode: mode_type,
        service_url,
        service_name,
    })
}

/// Information about the current app mode (sidecar context)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SidecarAppModeInfo {
    /// Mode type: "standalone", "service", or "connected"
    pub mode: String,
    /// Service URL if in service or connected mode
    pub service_url: Option<String>,
    /// Running service name (for service mode)
    pub service_name: Option<String>,
}
