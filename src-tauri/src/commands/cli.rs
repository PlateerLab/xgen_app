//! AI CLI Commands
//!
//! IPC commands for the AI CLI panel.
//! Uses XGEN backend's LLM provider — no separate API key needed.

use std::sync::Arc;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri::webview::WebviewWindowBuilder;

use crate::error::{AppError, Result};
use crate::services::{LlmClient, XgenApiClient};
use crate::services::llm_client::ChatMessage;
use crate::state::AppState;

/// Open AI CLI in a separate window with auth token
#[tauri::command]
pub async fn open_cli_window(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    xgen_token: Option<String>,
) -> Result<()> {
    // Store token in CLI session for later use
    if let Some(token) = &xgen_token {
        let mut session = state.cli_session.write().await;
        session.xgen_token = Some(token.clone());
    }

    // If window already exists, focus it
    if let Some(window) = app.get_webview_window("cli") {
        let _ = window.set_focus();
        return Ok(());
    }

    // Pass token as query param so cli.html can use it
    let url = if let Some(token) = &xgen_token {
        format!("cli.html?token={}", token)
    } else {
        "cli.html".to_string()
    };

    let _window = WebviewWindowBuilder::new(
        &app,
        "cli",
        tauri::WebviewUrl::App(url.into()),
    )
    .title("XGEN AI CLI")
    .inner_size(700.0, 500.0)
    .min_inner_size(400.0, 300.0)
    .resizable(true)
    .decorations(true)
    .build()
    .map_err(|e| AppError::Cli(format!("Failed to create CLI window: {}", e)))?;

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResponse {
    pub session_id: String,
    pub text: String,
    pub tool_calls_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliHistoryMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<Value>,
}

/// Send a message to the AI CLI
/// LLM config is fetched from XGEN backend automatically.
/// Optional provider/model selection (defaults to anthropic).
#[tauri::command]
pub async fn cli_send_message(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    message: String,
    xgen_token: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<CliResponse> {
    // Get XGEN API base URL
    let base_url = state.get_server_url().await
        .or_else(|| std::env::var("XGEN_SERVER_URL").ok())
        .unwrap_or_else(|| "https://xgen.x2bee.com".to_string());

    // Use token: prefer passed token, fallback to session stored token
    let token = xgen_token.or_else(|| {
        let session = state.cli_session.try_read().ok();
        session.and_then(|s| s.xgen_token.clone())
    });
    let xgen_api = XgenApiClient::new(base_url, token);

    // Create LLM client from XGEN backend config with optional provider/model
    let llm = LlmClient::from_xgen(
        &xgen_api,
        provider.as_deref(),
        model.as_deref(),
    ).await?;

    let mut session = state.cli_session.write().await;

    // Add user message
    session.messages.push(ChatMessage {
        role: "user".into(),
        content: Value::String(message),
    });

    let session_id = session.session_id.clone();
    let mut messages = session.messages.clone();

    // Release lock during API call
    drop(session);

    // Run tool use loop
    let final_text = llm.send_with_tools(&mut messages, &xgen_api, &session_id, &app).await?;

    // Count tool calls
    let tool_calls_count = messages.iter()
        .filter(|m| m.role == "assistant")
        .filter_map(|m| m.content.as_array())
        .flatten()
        .filter(|b| b["type"].as_str() == Some("tool_use"))
        .count();

    // Save updated messages back
    let mut session = state.cli_session.write().await;
    session.messages = messages;

    Ok(CliResponse {
        session_id,
        text: final_text,
        tool_calls_count,
    })
}

/// Get chat history
#[tauri::command]
pub async fn cli_get_history(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<CliHistoryMessage>> {
    let session = state.cli_session.read().await;

    let history: Vec<CliHistoryMessage> = session.messages.iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| {
            let (text, tools) = extract_display_content(&m.content);
            CliHistoryMessage {
                role: m.role.clone(),
                content: text,
                tool_calls: tools,
            }
        })
        .collect();

    Ok(history)
}

/// Clear the current session
#[tauri::command]
pub async fn cli_clear_session(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<String> {
    let mut session = state.cli_session.write().await;
    session.clear();
    Ok(session.session_id.clone())
}

/// List available LLM providers from XGEN backend
#[tauri::command]
pub async fn cli_list_providers(
    state: tauri::State<'_, Arc<AppState>>,
    xgen_token: Option<String>,
) -> Result<Value> {
    let base_url = state.get_server_url().await
        .or_else(|| std::env::var("XGEN_SERVER_URL").ok())
        .unwrap_or_else(|| "https://xgen.x2bee.com".to_string());
    let token = xgen_token.or_else(|| {
        let session = state.cli_session.try_read().ok();
        session.and_then(|s| s.xgen_token.clone())
    });
    let xgen_api = XgenApiClient::new(base_url, token);
    let providers = xgen_api.list_available_providers().await?;
    Ok(serde_json::to_value(providers).unwrap_or_default())
}

/// Store auth token in CLI session (called by frontend after login)
#[tauri::command]
pub async fn cli_set_token(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
) -> Result<()> {
    let mut session = state.cli_session.write().await;
    session.xgen_token = Some(token);
    Ok(())
}

/// Get stored auth token from CLI session (fallback for when URL param is missing)
#[tauri::command]
pub async fn cli_get_token(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<String>> {
    let session = state.cli_session.read().await;
    Ok(session.xgen_token.clone())
}

/// Get CLI session info
#[tauri::command]
pub async fn cli_get_session_info(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Value> {
    let session = state.cli_session.read().await;
    Ok(serde_json::json!({
        "sessionId": session.session_id,
        "messageCount": session.messages.len(),
    }))
}

/// Extract displayable text and tool calls from message content
fn extract_display_content(content: &Value) -> (String, Vec<Value>) {
    match content {
        Value::String(s) => (s.clone(), vec![]),
        Value::Array(blocks) => {
            let mut text = String::new();
            let mut tools = Vec::new();
            for block in blocks {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(t) = block["text"].as_str() {
                            text.push_str(t);
                        }
                    }
                    Some("tool_use") => {
                        tools.push(serde_json::json!({
                            "name": block["name"],
                            "input": block["input"],
                        }));
                    }
                    _ => {}
                }
            }
            (text, tools)
        }
        _ => (String::new(), vec![]),
    }
}
