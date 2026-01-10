//! Mode Commands
//!
//! Tauri commands for app mode management (Standalone/Connected).

use std::sync::Arc;
use tauri::State;

use crate::error::{AppError, Result};
use crate::state::{AppMode, AppState};

/// Set the application mode
#[tauri::command(rename_all = "camelCase")]
pub async fn set_app_mode(
    state: State<'_, Arc<AppState>>,
    mode: String,
    server_url: Option<String>,
) -> Result<()> {
    // 현재 모드 확인 (중복 설정 방지)
    {
        let current_mode = state.app_mode.read().await;
        let is_same = match (&*current_mode, mode.as_str(), &server_url) {
            (AppMode::Standalone, "standalone", _) => true,
            (AppMode::Connected { server_url: current_url }, "connected", Some(new_url)) => {
                current_url == new_url
            }
            _ => false,
        };

        if is_same {
            log::debug!("Mode already set to {}, skipping", mode);
            return Ok(());
        }
    }

    log::info!("Setting app mode: {}, server_url: {:?}", mode, server_url);

    let mut app_mode = state.app_mode.write().await;

    *app_mode = match mode.as_str() {
        "standalone" => {
            log::info!("Switching to Standalone mode");
            AppMode::Standalone
        }
        "connected" => {
            let url = server_url.ok_or_else(|| {
                AppError::Unknown("server_url required for connected mode".to_string())
            })?;
            log::info!("Switching to Connected mode: {}", url);

            // Store gateway URL
            let mut gateway = state.gateway_url.write().await;
            *gateway = Some(url.clone());

            AppMode::Connected { server_url: url }
        }
        _ => {
            return Err(AppError::Unknown(format!("Invalid mode: {}", mode)));
        }
    };

    Ok(())
}

/// Get the current application mode
#[tauri::command]
pub async fn get_app_mode(state: State<'_, Arc<AppState>>) -> Result<AppModeInfo> {
    let mode = state.app_mode.read().await;

    let info = match &*mode {
        AppMode::Standalone => AppModeInfo {
            mode: "standalone".to_string(),
            server_url: None,
            connected: false,
        },
        AppMode::Service { service_url } => AppModeInfo {
            mode: "service".to_string(),
            server_url: Some(service_url.clone()),
            connected: true,
        },
        AppMode::Connected { server_url } => AppModeInfo {
            mode: "connected".to_string(),
            server_url: Some(server_url.clone()),
            connected: true, // TODO: Actually check connection
        },
    };

    Ok(info)
}

/// Application mode information
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppModeInfo {
    pub mode: String,
    pub server_url: Option<String>,
    pub connected: bool,
}

/// Check connection to gateway server
#[tauri::command]
pub async fn check_gateway_connection(state: State<'_, Arc<AppState>>) -> Result<bool> {
    let mode = state.app_mode.read().await;

    match &*mode {
        AppMode::Standalone => Ok(false),
        AppMode::Service { service_url } => {
            log::info!("Checking connection to service: {}", service_url);

            let client = reqwest::Client::new();
            let url = format!("{}/health", service_url);

            match client.get(&url).send().await {
                Ok(resp) => {
                    let connected = resp.status().is_success();
                    log::info!("Service connection: {}", if connected { "OK" } else { "Failed" });
                    Ok(connected)
                }
                Err(e) => {
                    log::warn!("Service connection failed: {}", e);
                    Ok(false)
                }
            }
        }
        AppMode::Connected { server_url } => {
            log::info!("Checking connection to: {}", server_url);

            // Try to reach the health endpoint
            let client = reqwest::Client::new();
            let url = format!("{}/health", server_url);

            match client.get(&url).send().await {
                Ok(resp) => {
                    let connected = resp.status().is_success();
                    log::info!("Gateway connection: {}", if connected { "OK" } else { "Failed" });
                    Ok(connected)
                }
                Err(e) => {
                    log::warn!("Gateway connection failed: {}", e);
                    Ok(false)
                }
            }
        }
    }
}
