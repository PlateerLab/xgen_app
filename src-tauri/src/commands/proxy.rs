//! Local LLM Proxy Commands
//!
//! Tauri commands for proxying requests to local LLM servers (e.g., llama.cpp, vLLM).
//! This allows Connected mode users to use their local LLM endpoints.
//!
//! ## Use Case
//! When a user sets LLM endpoint to localhost:8080 in Connected mode,
//! the request should go to their local machine, not the gateway server.
//!
//! ## Tunnel
//! Uses bore tunnel to expose local LLM to the server.
//! No authtoken required - fully automatic.

use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};
use futures::StreamExt;
use tokio::sync::Mutex;

use crate::error::{AppError, Result};
use crate::proxy_server::ProxyServer;
use crate::tunnel::TunnelManager;

/// Response from local LLM proxy
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProxyResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// Proxy a request to a local LLM server (non-streaming)
#[tauri::command]
pub async fn proxy_local_llm(
    url: String,
    method: String,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> Result<ProxyResponse> {
    log::info!("Proxying request to local LLM: {} {}", method, url);

    // Validate URL is localhost
    if !is_localhost_url(&url) {
        return Err(AppError::Unknown("Only localhost URLs are allowed for proxy".to_string()));
    }

    let client = reqwest::Client::new();

    let mut request_builder = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => {
            return Err(AppError::Unknown(format!("Unsupported HTTP method: {}", method)));
        }
    };

    // Add headers
    for (key, value) in headers {
        request_builder = request_builder.header(&key, &value);
    }

    // Add body if present
    if let Some(body_str) = body {
        request_builder = request_builder.body(body_str);
    }

    let response = request_builder.send().await?;

    let status = response.status().as_u16();
    let response_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = response.text().await?;

    log::info!("Proxy response: status={}", status);

    Ok(ProxyResponse {
        status,
        headers: response_headers,
        body,
    })
}

/// Proxy a streaming request to a local LLM server (SSE)
///
/// Emits events:
/// - `proxy:chunk`: Each SSE chunk received
/// - `proxy:done`: Stream complete
/// - `proxy:error`: If an error occurs
#[tauri::command(rename_all = "camelCase")]
pub async fn proxy_local_llm_stream(
    app: AppHandle,
    request_id: String,
    url: String,
    method: String,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> Result<()> {
    log::info!("Proxying streaming request to local LLM: {} {} (id={})", method, url, request_id);

    // Validate URL is localhost
    if !is_localhost_url(&url) {
        let _ = app.emit(&format!("proxy:error:{}", request_id), "Only localhost URLs are allowed for proxy");
        return Err(AppError::Unknown("Only localhost URLs are allowed for proxy".to_string()));
    }

    let client = reqwest::Client::new();

    let mut request_builder = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => {
            let _ = app.emit(&format!("proxy:error:{}", request_id), format!("Unsupported HTTP method: {}", method));
            return Err(AppError::Unknown(format!("Unsupported HTTP method: {}", method)));
        }
    };

    // Add headers
    for (key, value) in headers {
        request_builder = request_builder.header(&key, &value);
    }

    // Add body if present
    if let Some(body_str) = body {
        request_builder = request_builder.body(body_str);
    }

    let response = match request_builder.send().await {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("Proxy request failed: {}", e);
            let _ = app.emit(&format!("proxy:error:{}", request_id), e.to_string());
            return Err(AppError::Network(e));
        }
    };

    let status = response.status().as_u16();

    // Emit initial status
    let _ = app.emit(&format!("proxy:start:{}", request_id), serde_json::json!({
        "status": status,
    }));

    // Stream the response body
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                let chunk_str = String::from_utf8_lossy(&chunk).to_string();
                if let Err(e) = app.emit(&format!("proxy:chunk:{}", request_id), &chunk_str) {
                    log::error!("Failed to emit chunk: {}", e);
                }
            }
            Err(e) => {
                log::error!("Stream error: {}", e);
                let _ = app.emit(&format!("proxy:error:{}", request_id), e.to_string());
                return Err(AppError::Network(e));
            }
        }
    }

    // Emit completion
    let _ = app.emit(&format!("proxy:done:{}", request_id), serde_json::json!({
        "status": status,
    }));

    log::info!("Proxy stream complete: {}", request_id);
    Ok(())
}

/// Check if a URL is a localhost URL
fn is_localhost_url(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    url_lower.starts_with("http://localhost")
        || url_lower.starts_with("https://localhost")
        || url_lower.starts_with("http://127.0.0.1")
        || url_lower.starts_with("https://127.0.0.1")
        || url_lower.starts_with("http://0.0.0.0")
        || url_lower.starts_with("https://0.0.0.0")
        || url_lower.starts_with("http://[::1]")
        || url_lower.starts_with("https://[::1]")
}

// ============================================================================
// Proxy Server Commands
// ============================================================================

/// Proxy server state stored in Tauri (includes bore tunnel management)
pub struct ProxyServerManager {
    server: Mutex<ProxyServer>,
    tunnel: TunnelManager,
}

impl ProxyServerManager {
    pub fn new() -> Self {
        Self {
            server: Mutex::new(ProxyServer::new()),
            tunnel: TunnelManager::new(),
        }
    }

    /// Auto-start proxy server with given endpoint
    pub async fn auto_start(&self, endpoint: String, port: u16) -> std::result::Result<u16, String> {
        let mut server = self.server.lock().await;
        server.set_local_llm_endpoint(Some(endpoint)).await;
        server.start(port).await
    }

    /// Auto-start proxy server AND bore tunnel
    pub async fn auto_start_with_tunnel(&self, endpoint: String, port: u16) -> std::result::Result<String, String> {
        // Start proxy server first
        let actual_port = self.auto_start(endpoint, port).await?;

        // Then start bore tunnel
        let public_url = self.tunnel.start(actual_port).await?;

        Ok(public_url)
    }

    /// Get tunnel manager reference
    pub fn tunnel(&self) -> &TunnelManager {
        &self.tunnel
    }
}

impl Default for ProxyServerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Proxy server status
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyServerStatus {
    pub running: bool,
    pub port: Option<u16>,
    pub local_url: Option<String>,
}

/// Start the local LLM proxy server
#[tauri::command]
pub async fn start_proxy_server(
    app: AppHandle,
    port: Option<u16>,
    local_llm_endpoint: String,
) -> Result<ProxyServerStatus> {
    log::info!("Starting proxy server for local LLM: {}", local_llm_endpoint);

    let manager = app.state::<ProxyServerManager>();
    let mut server = manager.server.lock().await;

    // 이미 실행 중이면 중지
    if server.is_running() {
        server.stop();
    }

    // 로컬 LLM 엔드포인트 설정
    server.set_local_llm_endpoint(Some(local_llm_endpoint)).await;

    // 서버 시작 (기본 포트: 19820)
    let actual_port = server.start(port.unwrap_or(19820)).await
        .map_err(|e| AppError::Unknown(e))?;

    Ok(ProxyServerStatus {
        running: true,
        port: Some(actual_port),
        local_url: Some(format!("http://127.0.0.1:{}", actual_port)),
    })
}

/// Stop the local LLM proxy server
#[tauri::command]
pub async fn stop_proxy_server(app: AppHandle) -> Result<ProxyServerStatus> {
    log::info!("Stopping proxy server");

    let manager = app.state::<ProxyServerManager>();
    let mut server = manager.server.lock().await;

    server.stop();

    Ok(ProxyServerStatus {
        running: false,
        port: None,
        local_url: None,
    })
}

/// Get proxy server status
#[tauri::command]
pub async fn get_proxy_server_status(app: AppHandle) -> Result<ProxyServerStatus> {
    let manager = app.state::<ProxyServerManager>();
    let server = manager.server.lock().await;

    let port = server.get_port();

    Ok(ProxyServerStatus {
        running: server.is_running(),
        port,
        local_url: port.map(|p| format!("http://127.0.0.1:{}", p)),
    })
}

/// Update local LLM endpoint for proxy server
#[tauri::command]
pub async fn update_proxy_endpoint(
    app: AppHandle,
    local_llm_endpoint: Option<String>,
) -> Result<()> {
    log::info!("Updating proxy endpoint: {:?}", local_llm_endpoint);

    let manager = app.state::<ProxyServerManager>();
    let server = manager.server.lock().await;

    server.set_local_llm_endpoint(local_llm_endpoint).await;

    Ok(())
}

// ============================================================================
// Bore Tunnel Commands
// ============================================================================

/// Full tunnel status (proxy + bore)
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub proxy_running: bool,
    pub proxy_port: Option<u16>,
    pub tunnel_connected: bool,
    pub public_url: Option<String>,
}

/// Start proxy server and bore tunnel together
#[tauri::command]
pub async fn start_tunnel(
    app: AppHandle,
    local_llm_endpoint: String,
) -> Result<TunnelStatus> {
    log::info!("Starting tunnel for local LLM: {}", local_llm_endpoint);

    let manager = app.state::<ProxyServerManager>();

    // Start or reuse proxy server
    let proxy_port: u16;
    {
        let mut server = manager.server.lock().await;

        // Update endpoint
        server.set_local_llm_endpoint(Some(local_llm_endpoint)).await;

        // If already running, just reuse it
        if server.is_running() {
            proxy_port = server.get_port().unwrap_or(19820);
            log::info!("Proxy server already running on port {}", proxy_port);
        } else {
            // Start new server
            proxy_port = server.start(19820).await.map_err(|e| AppError::Unknown(e))?;
            log::info!("Proxy server started on port {}", proxy_port);
        }
    }

    // Start bore tunnel
    let tunnel_status = manager.tunnel().get_status().await;
    let mut public_url = tunnel_status.public_url.clone();
    let mut tunnel_connected = tunnel_status.connected;

    if !tunnel_connected {
        match manager.tunnel().start(proxy_port).await {
            Ok(url) => {
                log::info!("Bore tunnel started: {}", url);
                public_url = Some(url);
                tunnel_connected = true;
            }
            Err(e) => {
                log::error!("Failed to start bore tunnel: {}", e);
                return Err(AppError::Unknown(format!("Failed to start tunnel: {}", e)));
            }
        }
    } else {
        log::info!("Bore tunnel already running: {:?}", public_url);
    }

    Ok(TunnelStatus {
        proxy_running: true,
        proxy_port: Some(proxy_port),
        tunnel_connected,
        public_url,
    })
}

/// Stop both proxy server and bore tunnel
#[tauri::command]
pub async fn stop_tunnel(app: AppHandle) -> Result<TunnelStatus> {
    log::info!("Stopping tunnel");

    let manager = app.state::<ProxyServerManager>();

    // Stop bore tunnel first
    manager.tunnel().stop().await;

    // Then stop proxy server
    {
        let mut server = manager.server.lock().await;
        server.stop();
    }

    Ok(TunnelStatus {
        proxy_running: false,
        proxy_port: None,
        tunnel_connected: false,
        public_url: None,
    })
}

/// Get full tunnel status
#[tauri::command]
pub async fn get_tunnel_status(app: AppHandle) -> Result<TunnelStatus> {
    let manager = app.state::<ProxyServerManager>();

    let server = manager.server.lock().await;
    let tunnel_status = manager.tunnel().get_status().await;

    Ok(TunnelStatus {
        proxy_running: server.is_running(),
        proxy_port: server.get_port(),
        tunnel_connected: tunnel_status.connected,
        public_url: tunnel_status.public_url,
    })
}
