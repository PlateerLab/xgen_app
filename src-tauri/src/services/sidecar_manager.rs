//! Sidecar Process Manager
//!
//! Manages Python sidecar processes (xgen-workflow, xgen-model, etc.)
//! for Service Mode operation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::async_runtime::spawn;
use tauri_plugin_shell::{process::CommandChild, ShellExt};

use crate::error::{AppError, Result};

/// Sidecar service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarConfig {
    /// Service name (e.g., "xgen-workflow")
    pub name: String,
    /// Binary name in binaries/ directory
    pub binary_name: String,
    /// Port the service listens on
    pub port: u16,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Whether to auto-start on app launch
    pub auto_start: bool,
}

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            name: "xgen-workflow".to_string(),
            binary_name: "xgen-workflow".to_string(),
            port: 8001,
            env: HashMap::new(),
            auto_start: false,
        }
    }
}

/// Status of a running sidecar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarStatus {
    pub name: String,
    pub running: bool,
    pub port: u16,
    pub url: String,
    pub pid: Option<u32>,
    pub health_ok: bool,
}

/// Manages sidecar processes
pub struct SidecarManager {
    /// Running processes keyed by service name
    processes: HashMap<String, CommandChild>,
    /// Service configurations
    configs: HashMap<String, SidecarConfig>,
}

impl SidecarManager {
    pub fn new() -> Self {
        let mut manager = Self {
            processes: HashMap::new(),
            configs: HashMap::new(),
        };

        // Register default xgen-workflow config
        manager.register_default_configs();
        manager
    }

    fn register_default_configs(&mut self) {
        // xgen-workflow service
        let mut workflow_env = HashMap::new();
        workflow_env.insert("APP_PORT".to_string(), "8001".to_string());
        workflow_env.insert("APP_HOST".to_string(), "127.0.0.1".to_string());

        self.configs.insert(
            "xgen-workflow".to_string(),
            SidecarConfig {
                name: "xgen-workflow".to_string(),
                binary_name: "xgen-workflow".to_string(),
                port: 8001,
                env: workflow_env,
                auto_start: true,
            },
        );

        // xgen-embedding service
        let mut embedding_env = HashMap::new();
        embedding_env.insert("APP_PORT".to_string(), "8002".to_string());
        embedding_env.insert("APP_HOST".to_string(), "127.0.0.1".to_string());

        self.configs.insert(
            "xgen-embedding".to_string(),
            SidecarConfig {
                name: "xgen-embedding".to_string(),
                binary_name: "xgen-embedding".to_string(),
                port: 8002,
                env: embedding_env,
                auto_start: true,
            },
        );
    }

    /// Start a sidecar process
    pub async fn start_sidecar(
        &mut self,
        app_handle: &tauri::AppHandle,
        name: &str,
        extra_env: Option<HashMap<String, String>>,
    ) -> Result<SidecarStatus> {
        // Check if already running
        if self.processes.contains_key(name) {
            return Err(AppError::Workflow(format!(
                "Sidecar '{}' is already running",
                name
            )));
        }

        // Get config
        let config = self
            .configs
            .get(name)
            .ok_or_else(|| AppError::Workflow(format!("Unknown sidecar: {}", name)))?
            .clone();

        // Build environment
        let mut env = config.env.clone();
        if let Some(extra) = extra_env {
            env.extend(extra);
        }

        // Start the sidecar using Tauri shell plugin
        let sidecar_name = format!("binaries/{}", config.binary_name);
        let shell = app_handle.shell();

        let mut command = shell
            .sidecar(&sidecar_name)
            .map_err(|e| AppError::Workflow(format!("Failed to create sidecar command: {}", e)))?;

        // Set environment variables
        for (key, value) in &env {
            command = command.env(key, value);
        }

        // Spawn the process
        let (mut rx, child) = command
            .spawn()
            .map_err(|e| AppError::Workflow(format!("Failed to spawn sidecar: {}", e)))?;

        let pid = child.pid();

        // Store the process
        self.processes.insert(name.to_string(), child);

        // Spawn a task to handle stdout/stderr
        let service_name = name.to_string();
        spawn(async move {
            use tauri_plugin_shell::process::CommandEvent;
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(data) => {
                        if let Ok(line) = String::from_utf8(data) {
                            tracing::info!("[{}] {}", service_name, line.trim());
                        }
                    }
                    CommandEvent::Stderr(data) => {
                        if let Ok(line) = String::from_utf8(data) {
                            tracing::warn!("[{}] {}", service_name, line.trim());
                        }
                    }
                    CommandEvent::Terminated(payload) => {
                        tracing::info!(
                            "[{}] Process terminated with code: {:?}",
                            service_name,
                            payload.code
                        );
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Wait a bit for the service to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Check health
        let url = format!("http://127.0.0.1:{}", config.port);
        let health_ok = self.check_health(&url).await;

        Ok(SidecarStatus {
            name: name.to_string(),
            running: true,
            port: config.port,
            url,
            pid: Some(pid),
            health_ok,
        })
    }

    /// Stop a sidecar process
    pub async fn stop_sidecar(&mut self, name: &str) -> Result<()> {
        if let Some(child) = self.processes.remove(name) {
            child
                .kill()
                .map_err(|e| AppError::Workflow(format!("Failed to kill sidecar: {}", e)))?;
            tracing::info!("Stopped sidecar: {}", name);
            Ok(())
        } else {
            Err(AppError::Workflow(format!(
                "Sidecar '{}' is not running",
                name
            )))
        }
    }

    /// Stop all running sidecars
    pub async fn stop_all(&mut self) -> Result<()> {
        let names: Vec<String> = self.processes.keys().cloned().collect();
        for name in names {
            if let Err(e) = self.stop_sidecar(&name).await {
                tracing::warn!("Failed to stop {}: {}", name, e);
            }
        }
        Ok(())
    }

    /// Get status of a sidecar
    pub async fn get_status(&self, name: &str) -> Result<SidecarStatus> {
        let config = self
            .configs
            .get(name)
            .ok_or_else(|| AppError::Workflow(format!("Unknown sidecar: {}", name)))?;

        let running = self.processes.contains_key(name);
        let url = format!("http://127.0.0.1:{}", config.port);
        let health_ok = if running {
            self.check_health(&url).await
        } else {
            false
        };

        let pid = self.processes.get(name).map(|p| p.pid());

        Ok(SidecarStatus {
            name: name.to_string(),
            running,
            port: config.port,
            url,
            pid,
            health_ok,
        })
    }

    /// Get status of all sidecars
    pub async fn get_all_status(&self) -> Vec<SidecarStatus> {
        let mut statuses = Vec::new();
        for name in self.configs.keys() {
            if let Ok(status) = self.get_status(name).await {
                statuses.push(status);
            }
        }
        statuses
    }

    /// Check if a service is healthy
    async fn check_health(&self, url: &str) -> bool {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .ok();

        if let Some(client) = client {
            // Try health endpoint first, then root
            for endpoint in &["/health", "/docs", "/"] {
                let full_url = format!("{}{}", url, endpoint);
                if let Ok(resp) = client.get(&full_url).send().await {
                    if resp.status().is_success() || resp.status().as_u16() == 307 {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Update sidecar configuration
    pub fn update_config(&mut self, name: &str, config: SidecarConfig) {
        self.configs.insert(name.to_string(), config);
    }

    /// Get sidecar configuration
    pub fn get_config(&self, name: &str) -> Option<&SidecarConfig> {
        self.configs.get(name)
    }

    /// List all registered sidecars
    pub fn list_sidecars(&self) -> Vec<String> {
        self.configs.keys().cloned().collect()
    }

    /// Start all sidecars marked with auto_start = true
    pub async fn start_auto_start_sidecars(
        &mut self,
        app_handle: &tauri::AppHandle,
    ) -> Vec<Result<SidecarStatus>> {
        let auto_start_names: Vec<String> = self
            .configs
            .iter()
            .filter(|(_, config)| config.auto_start)
            .map(|(name, _)| name.clone())
            .collect();

        let mut results = Vec::new();
        for name in auto_start_names {
            tracing::info!("Auto-starting sidecar: {}", name);
            let result = self.start_sidecar(app_handle, &name, None).await;
            match &result {
                Ok(status) => {
                    tracing::info!(
                        "Auto-started {} on port {} (health: {})",
                        name,
                        status.port,
                        status.health_ok
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to auto-start {}: {}", name, e);
                }
            }
            results.push(result);
        }
        results
    }
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}
