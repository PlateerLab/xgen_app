//! LLM Commands
//!
//! Tauri commands for LLM inference and embedding using mistral.rs.
//!
//! ## Architecture
//! - Model loading via InferenceEngine (wraps mistral.rs GgufModelBuilder)
//! - Streaming generation via Tauri events
//! - MCP integration via mistralrs_mcp (configured via McpConfigManager)

use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, Result};
use crate::services::{EmbedRequest, GenerateRequest, ModelConfig, ModelStatus};
use crate::state::AppState;

/// Load a model for inference
///
/// Uses mistral.rs GgufModelBuilder with automatic device mapping
#[tauri::command]
pub async fn load_model(
    state: State<'_, Arc<AppState>>,
    model_path: String,
    model_id: Option<String>,
    tokenizer_id: Option<String>,
    context_length: Option<usize>,
    paged_attention: Option<bool>,
    chat_template: Option<String>,
) -> Result<ModelStatus> {
    log::info!("Loading model: {}", model_path);

    let config = ModelConfig {
        model_path: model_path.clone(),
        model_id: model_id.unwrap_or_else(|| {
            std::path::Path::new(&model_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        }),
        tokenizer_id,
        context_length,
        paged_attention: paged_attention.unwrap_or(false),
        chat_template,
    };

    let mut engine = state.inference_engine.write().await;
    let status = engine.load_model(config).await?;

    log::info!("Model loaded: {} on {}", status.model_id.as_deref().unwrap_or("unknown"), status.device);
    Ok(status)
}

/// Get current model status
#[tauri::command]
pub async fn get_model_status(state: State<'_, Arc<AppState>>) -> Result<ModelStatus> {
    let engine = state.inference_engine.read().await;
    Ok(engine.status().clone())
}

/// Generate text using the loaded model (streaming via events)
///
/// Emits events:
/// - `llm:token`: Each generated token
/// - `llm:done`: Generation complete with stats
/// - `llm:error`: If an error occurs
#[tauri::command]
pub async fn generate(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    prompt: String,
    system_prompt: Option<String>,
    max_tokens: Option<usize>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    stop_sequences: Option<Vec<String>>,
) -> Result<()> {
    log::info!(
        "Generating text: {} chars, max_tokens={}",
        prompt.len(),
        max_tokens.unwrap_or(512)
    );

    let engine = state.inference_engine.read().await;

    if !engine.is_loaded() {
        return Err(AppError::Inference("No model loaded".to_string()));
    }

    let request = GenerateRequest {
        prompt,
        system_prompt,
        max_tokens: max_tokens.unwrap_or(512),
        temperature,
        top_p,
        stop_sequences,
        stream: true,
    };

    // Clone app handle for callback
    let app_handle = app.clone();

    // Use real streaming with callback
    let response = engine
        .generate_stream_with_callback(request, move |token| {
            // Emit each token to frontend
            if let Err(e) = app_handle.emit("llm:token", &token) {
                log::error!("Failed to emit token: {}", e);
            }
        })
        .await?;

    app.emit("llm:done", serde_json::json!({
        "prompt_tokens": response.prompt_tokens,
        "completion_tokens": response.completion_tokens,
        "generation_time_ms": response.generation_time_ms,
        "tokens_per_second": response.tokens_per_second,
    }))?;

    log::info!("Generation complete: {} tokens", response.completion_tokens);
    Ok(())
}

/// Generate text (non-streaming)
#[tauri::command]
pub async fn generate_sync(
    state: State<'_, Arc<AppState>>,
    prompt: String,
    system_prompt: Option<String>,
    max_tokens: Option<usize>,
    temperature: Option<f32>,
    top_p: Option<f32>,
) -> Result<String> {
    let engine = state.inference_engine.read().await;

    if !engine.is_loaded() {
        return Err(AppError::Inference("No model loaded".to_string()));
    }

    let request = GenerateRequest {
        prompt,
        system_prompt,
        max_tokens: max_tokens.unwrap_or(512),
        temperature,
        top_p,
        stop_sequences: None,
        stream: false,
    };

    let response = engine.generate(request).await?;
    Ok(response.text)
}

/// Stop ongoing generation
#[tauri::command]
pub async fn stop_generation(state: State<'_, Arc<AppState>>) -> Result<()> {
    log::info!("Stopping generation...");

    let engine = state.inference_engine.read().await;
    engine.stop().await?;

    Ok(())
}

/// Generate embeddings for text
#[tauri::command]
pub async fn embed_text(
    state: State<'_, Arc<AppState>>,
    texts: Vec<String>,
) -> Result<Vec<Vec<f32>>> {
    log::info!("Embedding {} texts", texts.len());

    let engine = state.inference_engine.read().await;

    if !engine.is_loaded() {
        return Err(AppError::Inference("No model loaded".to_string()));
    }

    let request = EmbedRequest { texts };
    let response = engine.embed(request).await?;

    log::info!("Embedding complete: {} vectors of dim {}", response.embeddings.len(), response.dimension);
    Ok(response.embeddings)
}

/// Unload the current model
#[tauri::command]
pub async fn unload_model(state: State<'_, Arc<AppState>>) -> Result<()> {
    log::info!("Unloading model...");

    let mut engine = state.inference_engine.write().await;
    engine.unload()?;

    log::info!("Model unloaded");
    Ok(())
}
