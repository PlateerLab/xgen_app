//! Settings Commands
//!
//! Tauri commands for persistent app settings (stored in app config directory).

use std::fs;
use tauri::Manager;

use crate::error::{AppError, Result};

/// Persistent application settings
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct AppSettings {
    /// Last used server URL (for connected mode)
    pub server_url: Option<String>,
    /// Last used mode: "standalone" or "connected"
    pub last_mode: String,
}

impl AppSettings {
    /// Create default settings
    pub fn new() -> Self {
        Self {
            server_url: None,
            last_mode: "standalone".to_string(),
        }
    }
}

/// Save application settings to config file
#[tauri::command]
pub async fn save_app_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<()> {
    log::info!("Saving app settings: mode={}, server_url={:?}",
        settings.last_mode, settings.server_url);

    // Get config directory
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Unknown(format!("Failed to get config dir: {}", e)))?;

    // Create directory if not exists
    fs::create_dir_all(&config_dir)
        .map_err(|e| AppError::Unknown(format!("Failed to create config dir: {}", e)))?;

    // Write settings file
    let config_path = config_dir.join("settings.json");
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| AppError::Unknown(format!("Failed to serialize settings: {}", e)))?;

    fs::write(&config_path, json)
        .map_err(|e| AppError::Unknown(format!("Failed to write settings: {}", e)))?;

    log::info!("Settings saved to: {:?}", config_path);
    Ok(())
}

/// Load application settings from config file
#[tauri::command]
pub async fn load_app_settings(app: tauri::AppHandle) -> Result<AppSettings> {
    // Get config directory
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Unknown(format!("Failed to get config dir: {}", e)))?;

    let config_path = config_dir.join("settings.json");

    // Check if file exists
    if !config_path.exists() {
        log::info!("No settings file found, returning defaults");
        return Ok(AppSettings::new());
    }

    // Read and parse settings
    let content = fs::read_to_string(&config_path)
        .map_err(|e| AppError::Unknown(format!("Failed to read settings: {}", e)))?;

    let settings: AppSettings = serde_json::from_str(&content)
        .map_err(|e| AppError::Unknown(format!("Failed to parse settings: {}", e)))?;

    log::info!("Settings loaded: mode={}, server_url={:?}",
        settings.last_mode, settings.server_url);

    Ok(settings)
}

/// Check connection to a gateway server (for testing before saving)
#[tauri::command]
pub async fn test_gateway_connection(url: String) -> Result<ConnectionTestResult> {
    log::info!("Testing connection to: {}", url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Unknown(format!("Failed to create HTTP client: {}", e)))?;

    let health_url = format!("{}/health", url.trim_end_matches('/'));
    let start = std::time::Instant::now();

    match client.get(&health_url).send().await {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let success = resp.status().is_success();

            log::info!(
                "Connection test: {} ({}ms)",
                if success { "OK" } else { "Failed" },
                elapsed
            );

            Ok(ConnectionTestResult {
                success,
                response_time_ms: Some(elapsed),
                error: if success {
                    None
                } else {
                    Some(format!("HTTP {}", resp.status()))
                },
            })
        }
        Err(e) => {
            log::warn!("Connection test failed: {}", e);

            Ok(ConnectionTestResult {
                success: false,
                response_time_ms: None,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Connection test result
#[derive(serde::Serialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub response_time_ms: Option<u64>,
    pub error: Option<String>,
}
