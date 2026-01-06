//! XGEN Desktop App
//!
//! A personalized AI workstation with local LLM inference and MCP support.
//!
//! ## Architecture (mistral.rs centric)
//! - LLM inference via mistral.rs (automatic GPU detection and device mapping)
//! - MCP client via mistralrs_mcp (connects to external MCP servers)
//! - Model management with local storage

pub mod commands;
pub mod error;
pub mod gpu;
pub mod services;
pub mod state;

use state::AppState;
use std::sync::Arc;
use tauri::Manager;

/// Main Tauri application entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize application state
    let app_state = Arc::new(AppState::new());

    tauri::Builder::default()
        // Manage shared state
        .manage(app_state)
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
            log::info!("Architecture: mistral.rs centric (GPU auto-detection, MCP client)");

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

            Ok(())
        })
        // Register all commands
        .invoke_handler(tauri::generate_handler![
            // Hardware Commands (system info, backend hints)
            commands::get_hardware_info,
            // Model Management Commands
            commands::list_models,
            commands::download_model,
            commands::delete_model,
            commands::get_models_dir,
            // LLM Commands (mistral.rs)
            commands::load_model,
            commands::get_model_status,
            commands::generate,
            commands::generate_sync,
            commands::stop_generation,
            commands::embed_text,
            commands::unload_model,
            // MCP Configuration Commands
            commands::list_mcp_servers,
            commands::add_mcp_server,
            commands::remove_mcp_server,
            commands::set_mcp_server_enabled,
            commands::get_enabled_mcp_count,
            commands::has_enabled_mcp_servers,
            // Mode Commands (legacy)
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
