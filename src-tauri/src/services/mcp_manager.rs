//! MCP Configuration Manager
//!
//! Manages MCP server configurations for mistralrs_mcp integration.
//!
//! mistral.rs provides built-in MCP client support via mistralrs_mcp.
//! This module manages the configuration of external MCP servers
//! that the LLM can connect to.
//!
//! ## Usage
//! ```rust,ignore
//! use mistralrs_mcp::{McpClientConfig, McpClient};
//!
//! // Get mistralrs_mcp config from our manager
//! let mcp_config = manager.to_mcp_client_config();
//!
//! // Initialize MCP client
//! let mut client = McpClient::new(mcp_config);
//! client.initialize().await?;
//!
//! // Get tool callbacks for model
//! let tool_callbacks = client.get_tool_callbacks_with_tools();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use mistralrs_mcp::{
    McpClientConfig as MistralMcpClientConfig,
    McpServerConfig as MistralMcpServerConfig,
    McpServerSource,
};

use crate::error::{AppError, Result};

/// MCP server connection type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum McpConnectionType {
    /// stdio transport (local process)
    Stdio,
    /// HTTP/SSE transport (remote server)
    Http,
}

impl std::fmt::Display for McpConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpConnectionType::Stdio => write!(f, "stdio"),
            McpConnectionType::Http => write!(f, "http"),
        }
    }
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name/identifier
    pub name: String,

    /// Connection type
    pub connection_type: McpConnectionType,

    /// For stdio: command to run
    pub command: Option<String>,

    /// For stdio: command arguments
    pub args: Option<Vec<String>>,

    /// For http: server URL
    pub url: Option<String>,

    /// Whether this server is enabled
    pub enabled: bool,

    /// Optional description
    pub description: Option<String>,
}

impl McpServerConfig {
    /// Create a new stdio MCP server config
    pub fn stdio(name: &str, command: &str, args: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            connection_type: McpConnectionType::Stdio,
            command: Some(command.to_string()),
            args: Some(args),
            url: None,
            enabled: true,
            description: None,
        }
    }

    /// Create a new HTTP MCP server config
    pub fn http(name: &str, url: &str) -> Self {
        Self {
            name: name.to_string(),
            connection_type: McpConnectionType::Http,
            command: None,
            args: None,
            url: Some(url.to_string()),
            enabled: true,
            description: None,
        }
    }

    /// Set description
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
}

/// MCP server status (for UI display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerStatus {
    /// Server name
    pub name: String,

    /// Connection type
    pub connection_type: String,

    /// Whether enabled
    pub enabled: bool,

    /// Description
    pub description: Option<String>,
}

/// MCP Configuration Manager
///
/// Manages MCP server configurations. The actual MCP client functionality
/// is handled by mistralrs_mcp when loading models.
pub struct McpConfigManager {
    /// Configured MCP servers
    servers: HashMap<String, McpServerConfig>,
}

impl McpConfigManager {
    /// Create a new MCP config manager
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    /// Create with default servers
    pub fn with_defaults() -> Self {
        let mut manager = Self::new();

        // Add common MCP servers as disabled by default
        // Users can enable them in settings

        // Example: filesystem MCP (if installed)
        manager.servers.insert(
            "filesystem".to_string(),
            McpServerConfig::stdio("filesystem", "npx", vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                ".".to_string(),
            ])
            .with_description("File system access via MCP"),
        );

        // Disable by default
        if let Some(server) = manager.servers.get_mut("filesystem") {
            server.enabled = false;
        }

        manager
    }

    /// Add a server configuration
    pub fn add_server(&mut self, config: McpServerConfig) -> Result<()> {
        if self.servers.contains_key(&config.name) {
            return Err(AppError::Mcp(format!(
                "Server already exists: {}",
                config.name
            )));
        }

        log::info!("Adding MCP server config: {}", config.name);
        self.servers.insert(config.name.clone(), config);
        Ok(())
    }

    /// Remove a server configuration
    pub fn remove_server(&mut self, name: &str) -> Result<()> {
        if self.servers.remove(name).is_some() {
            log::info!("Removed MCP server config: {}", name);
            Ok(())
        } else {
            Err(AppError::Mcp(format!("Server not found: {}", name)))
        }
    }

    /// Enable/disable a server
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        if let Some(server) = self.servers.get_mut(name) {
            server.enabled = enabled;
            log::info!("MCP server {} enabled: {}", name, enabled);
            Ok(())
        } else {
            Err(AppError::Mcp(format!("Server not found: {}", name)))
        }
    }

    /// Get a server configuration
    pub fn get_server(&self, name: &str) -> Option<&McpServerConfig> {
        self.servers.get(name)
    }

    /// List all server configurations
    pub fn list_servers(&self) -> Vec<McpServerStatus> {
        self.servers
            .values()
            .map(|s| McpServerStatus {
                name: s.name.clone(),
                connection_type: s.connection_type.to_string(),
                enabled: s.enabled,
                description: s.description.clone(),
            })
            .collect()
    }

    /// Get enabled server configurations (for mistralrs_mcp)
    pub fn get_enabled_configs(&self) -> Vec<&McpServerConfig> {
        self.servers.values().filter(|s| s.enabled).collect()
    }

    /// Check if any servers are enabled
    pub fn has_enabled_servers(&self) -> bool {
        self.servers.values().any(|s| s.enabled)
    }

    /// Convert our config to mistralrs_mcp McpServerConfig
    fn to_mistral_server_config(config: &McpServerConfig) -> MistralMcpServerConfig {
        let source = match config.connection_type {
            McpConnectionType::Stdio => {
                McpServerSource::Process {
                    command: config.command.clone().unwrap_or_default(),
                    args: config.args.clone().unwrap_or_default(),
                    work_dir: None,
                    env: None,
                }
            }
            McpConnectionType::Http => {
                McpServerSource::Http {
                    url: config.url.clone().unwrap_or_default(),
                    timeout_secs: Some(30),
                    headers: None,
                }
            }
        };

        MistralMcpServerConfig {
            id: config.name.clone(),
            name: config.description.clone().unwrap_or_else(|| config.name.clone()),
            source,
            enabled: config.enabled,
            tool_prefix: Some(config.name.clone()),
            resources: None,
            bearer_token: None,
        }
    }

    /// Convert enabled servers to mistralrs_mcp McpClientConfig
    ///
    /// Returns None if no servers are enabled.
    pub fn to_mcp_client_config(&self) -> Option<MistralMcpClientConfig> {
        let enabled_servers: Vec<MistralMcpServerConfig> = self
            .servers
            .values()
            .filter(|s| s.enabled)
            .map(Self::to_mistral_server_config)
            .collect();

        if enabled_servers.is_empty() {
            return None;
        }

        log::info!(
            "Creating MCP client config with {} enabled servers",
            enabled_servers.len()
        );

        Some(MistralMcpClientConfig {
            servers: enabled_servers,
            auto_register_tools: true,
            tool_timeout_secs: Some(30),
            max_concurrent_calls: Some(3),
        })
    }
}

impl Default for McpConfigManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_config() {
        let config = McpServerConfig::stdio("test", "node", vec!["server.js".to_string()]);
        assert_eq!(config.connection_type, McpConnectionType::Stdio);
        assert_eq!(config.command, Some("node".to_string()));
    }

    #[test]
    fn test_http_config() {
        let config = McpServerConfig::http("remote", "http://localhost:3000");
        assert_eq!(config.connection_type, McpConnectionType::Http);
        assert_eq!(config.url, Some("http://localhost:3000".to_string()));
    }

    #[test]
    fn test_manager_add_remove() {
        let mut manager = McpConfigManager::new();

        let config = McpServerConfig::http("test", "http://localhost:3000");
        manager.add_server(config).unwrap();

        assert!(manager.get_server("test").is_some());
        manager.remove_server("test").unwrap();
        assert!(manager.get_server("test").is_none());
    }
}
