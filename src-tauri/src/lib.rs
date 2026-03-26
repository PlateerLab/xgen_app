//! XGEN Desktop App
//!
//! A personalized AI workstation with proxy, tunnel, and sidecar support.

pub mod commands;
pub mod error;
pub mod gpu;
pub mod tunnel;
pub mod proxy_server;
pub mod services;
pub mod state;

use commands::ProxyServerManager;
use state::AppState;
use std::sync::Arc;
use tauri::Manager;

/// Auto-initialize app mode from saved settings
async fn auto_init_app_mode(app: &tauri::AppHandle) -> Result<(), String> {
    use std::fs;
    use tauri::Manager;

    // Get config directory
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;

    let config_path = config_dir.join("settings.json");

    // Check if settings file exists
    if !config_path.exists() {
        log::info!("No settings file found, using default Standalone mode");
        return Ok(());
    }

    // Read and parse settings
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;

    let settings: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings: {}", e))?;

    // Get last mode and server URL
    let last_mode = settings.get("lastMode")
        .and_then(|m| m.as_str())
        .unwrap_or("standalone");

    let server_url = settings.get("serverUrl")
        .and_then(|u| u.as_str())
        .map(|s| s.to_string());

    // Set app mode based on saved settings
    if last_mode == "connected" {
        if let Some(url) = server_url {
            log::info!("Auto-initializing app mode: Connected to {}", url);

            let state = app.state::<Arc<AppState>>();
            let mut mode = state.app_mode.write().await;
            *mode = state::AppMode::Connected { server_url: url.clone() };

            let mut gateway = state.gateway_url.write().await;
            *gateway = Some(url);
        } else {
            log::warn!("Connected mode saved but no server URL found, using Standalone");
        }
    } else {
        log::info!("Auto-initializing app mode: Standalone");
    }

    Ok(())
}

/// Auto-start proxy server and bore tunnel if local LLM is configured
async fn auto_start_tunnel(app: &tauri::AppHandle) -> Result<(), String> {
    use std::fs;
    use tauri::Manager;

    // Get config directory
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;

    let config_path = config_dir.join("settings.json");

    // Check if settings file exists
    if !config_path.exists() {
        log::info!("No settings file found, skipping tunnel auto-start");
        return Ok(());
    }

    // Read and parse settings
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;

    let settings: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings: {}", e))?;

    // Check if local LLM is configured
    let local_llm = settings.get("localLlm");
    let endpoint = local_llm
        .and_then(|llm| llm.get("endpoint"))
        .and_then(|e| e.as_str())
        .filter(|e| !e.is_empty());

    // Default to enabled if endpoint is set (enabled field may not exist)
    let enabled = local_llm
        .and_then(|llm| llm.get("enabled"))
        .and_then(|e| e.as_bool())
        .unwrap_or(true); // Default to true if endpoint exists

    if let Some(endpoint) = endpoint {
        if enabled {
            log::info!("Auto-starting tunnel for local LLM: {}", endpoint);

            let manager = app.state::<ProxyServerManager>();

            // Start proxy server and tunnel
            match manager.auto_start_with_tunnel(endpoint.to_string(), 19820).await {
                Ok(public_url) => {
                    log::info!("Tunnel auto-started: {}", public_url);
                }
                Err(e) => {
                    log::warn!("Failed to auto-start tunnel: {}", e);
                }
            }
        } else {
            log::info!("Local LLM configured but not enabled, skipping tunnel auto-start");
        }
    } else {
        log::info!("No local LLM endpoint configured, skipping tunnel auto-start");
    }

    Ok(())
}

/// Main Tauri application entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize application state
    let app_state = Arc::new(AppState::new());

    // Initialize proxy server manager
    let proxy_manager = ProxyServerManager::new();

    tauri::Builder::default()
        // Manage shared state
        .manage(app_state)
        .manage(proxy_manager)
        // Register plugins
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_http::init())
        // Setup hook
        .setup(|app| {
            // Initialize logging
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            } else {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            log::info!("XGEN Desktop App starting...");
            log::info!("Version: {}", env!("CARGO_PKG_VERSION"));
            log::info!("Architecture: proxy + tunnel + sidecar");

            // Auto-start sidecars in background
            let app_handle = app.handle().clone();
            let state = app.state::<Arc<AppState>>().inner().clone();

            tauri::async_runtime::spawn(async move {
                log::info!("Starting auto-start sidecars...");

                let mut manager = state.sidecar_manager.write().await;
                let results = manager.start_auto_start_sidecars(&app_handle).await;

                let success_count = results.iter().filter(|r| r.is_ok()).count();
                let total = results.len();

                log::info!(
                    "Auto-start complete: {}/{} sidecars started successfully",
                    success_count,
                    total
                );
            });

            // Auto-initialize app mode from saved settings (must be first)
            let app_handle_mode = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = auto_init_app_mode(&app_handle_mode).await {
                    log::warn!("Failed to auto-init app mode: {}", e);
                }
            });

            // Auto-start tunnel if local LLM is configured
            let app_handle_tunnel = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Small delay to ensure mode is initialized first
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if let Err(e) = auto_start_tunnel(&app_handle_tunnel).await {
                    log::warn!("Failed to auto-start tunnel: {}", e);
                }
            });

            Ok(())
        })
        // Register all commands
        .invoke_handler(tauri::generate_handler![
            // Hardware Commands
            commands::get_hardware_info,
            // Model Management Commands
            commands::list_models,
            commands::download_model,
            commands::delete_model,
            commands::get_models_dir,
            // Mode Commands
            commands::set_app_mode,
            commands::get_app_mode,
            commands::check_gateway_connection,
            // Sidecar Commands (xgen-workflow, etc.)
            commands::start_sidecar,
            commands::stop_sidecar,
            commands::stop_all_sidecars,
            commands::get_sidecar_status,
            commands::get_all_sidecar_status,
            commands::list_sidecars,
            commands::enable_service_mode,
            commands::enable_standalone_mode,
            commands::get_current_mode,
            // Settings Commands (persistent)
            commands::save_app_settings,
            commands::load_app_settings,
            commands::test_gateway_connection,
            commands::test_local_llm_connection,
            // Local LLM Proxy Commands
            commands::proxy_local_llm,
            commands::proxy_local_llm_stream,
            // Proxy Server Commands
            commands::start_proxy_server,
            commands::stop_proxy_server,
            commands::get_proxy_server_status,
            commands::update_proxy_endpoint,
            // Bore Tunnel Commands
            commands::start_tunnel,
            commands::stop_tunnel,
            commands::get_tunnel_status,
            // AI CLI Commands
            commands::open_cli_window,
            commands::cli_send_message,
            commands::cli_get_history,
            commands::cli_clear_session,
            commands::cli_get_token,
            commands::cli_get_session_info,
            commands::cli_list_providers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
