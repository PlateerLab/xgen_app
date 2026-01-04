//! MCP Commands
//!
//! Tauri commands for MCP server configuration management.
//!
//! Note: The actual MCP client functionality is provided by mistralrs_mcp
//! when loading models. This module manages the configuration of external
//! MCP servers that the LLM can connect to.

use std::sync::Arc;
use tauri::State;

use crate::error::Result;
use crate::services::{McpServerConfig, McpServerStatus};
use crate::state::AppState;

/// List all configured MCP servers
#[tauri::command]
pub async fn list_mcp_servers(state: State<'_, Arc<AppState>>) -> Result<Vec<McpServerStatus>> {
    log::info!("Listing MCP server configurations...");

    let config = state.mcp_config.read().await;
    let servers = config.list_servers();

    log::info!("Found {} MCP server configurations", servers.len());
    Ok(servers)
}

/// Add a new MCP server configuration
#[tauri::command]
pub async fn add_mcp_server(
    state: State<'_, Arc<AppState>>,
    name: String,
    connection_type: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    url: Option<String>,
    description: Option<String>,
) -> Result<McpServerStatus> {
    log::info!("Adding MCP server: {} ({})", name, connection_type);

    let desc_clone = description.clone();
    let config = match connection_type.as_str() {
        "stdio" => {
            let cmd = command.ok_or_else(|| {
                crate::error::AppError::Mcp("command required for stdio".to_string())
            })?;
            let mut server = McpServerConfig::stdio(&name, &cmd, args.unwrap_or_default());
            if let Some(ref desc) = description {
                server = server.with_description(desc);
            }
            server
        }
        "http" => {
            let server_url = url.ok_or_else(|| {
                crate::error::AppError::Mcp("url required for http".to_string())
            })?;
            let mut server = McpServerConfig::http(&name, &server_url);
            if let Some(ref desc) = description {
                server = server.with_description(desc);
            }
            server
        }
        _ => {
            return Err(crate::error::AppError::Mcp(format!(
                "Invalid connection type: {}",
                connection_type
            )));
        }
    };

    let mut manager = state.mcp_config.write().await;
    manager.add_server(config)?;

    let status = McpServerStatus {
        name: name.clone(),
        connection_type,
        enabled: true,
        description: desc_clone,
    };

    log::info!("Added MCP server: {}", name);
    Ok(status)
}

/// Remove an MCP server configuration
#[tauri::command]
pub async fn remove_mcp_server(state: State<'_, Arc<AppState>>, name: String) -> Result<()> {
    log::info!("Removing MCP server: {}", name);

    let mut manager = state.mcp_config.write().await;
    manager.remove_server(&name)?;

    log::info!("Removed MCP server: {}", name);
    Ok(())
}

/// Enable or disable an MCP server
#[tauri::command]
pub async fn set_mcp_server_enabled(
    state: State<'_, Arc<AppState>>,
    name: String,
    enabled: bool,
) -> Result<()> {
    log::info!("Setting MCP server {} enabled: {}", name, enabled);

    let mut manager = state.mcp_config.write().await;
    manager.set_enabled(&name, enabled)?;

    Ok(())
}

/// Get enabled MCP servers count (for UI display)
#[tauri::command]
pub async fn get_enabled_mcp_count(state: State<'_, Arc<AppState>>) -> Result<usize> {
    let manager = state.mcp_config.read().await;
    Ok(manager.get_enabled_configs().len())
}

/// Check if any MCP servers are enabled
#[tauri::command]
pub async fn has_enabled_mcp_servers(state: State<'_, Arc<AppState>>) -> Result<bool> {
    let manager = state.mcp_config.read().await;
    Ok(manager.has_enabled_servers())
}

// Re-export types for frontend compatibility
pub use crate::services::McpServerStatus as McpServerInfo;

/// Legacy tool info structure (for compatibility)
/// Note: With mistralrs_mcp, tool discovery happens at model load time
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub schema: serde_json::Value,
}
