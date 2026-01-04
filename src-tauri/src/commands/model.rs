//! Model Commands
//!
//! Tauri commands for model management (download, list, delete).

use std::sync::Arc;
use tauri::State;

use crate::error::Result;
use crate::services::ModelInfo;
use crate::state::AppState;

/// List all available models
#[tauri::command]
pub async fn list_models(state: State<'_, Arc<AppState>>) -> Result<Vec<ModelInfo>> {
    log::info!("Listing models...");

    let manager = state.model_manager.read().await;
    let models = manager.list_models().await?;

    log::info!("Found {} models", models.len());
    Ok(models)
}

/// Download a model from HuggingFace
#[tauri::command]
pub async fn download_model(
    state: State<'_, Arc<AppState>>,
    repo_id: String,
    filename: String,
) -> Result<ModelInfo> {
    log::info!("Downloading model: {}/{}", repo_id, filename);

    let manager = state.model_manager.read().await;
    let model = manager.download_model(&repo_id, &filename).await?;

    log::info!("Model downloaded successfully: {}", model.name);
    Ok(model)
}

/// Delete a model
#[tauri::command]
pub async fn delete_model(state: State<'_, Arc<AppState>>, model_id: String) -> Result<()> {
    log::info!("Deleting model: {}", model_id);

    let manager = state.model_manager.read().await;
    manager.delete_model(&model_id).await?;

    log::info!("Model deleted: {}", model_id);
    Ok(())
}

/// Get model storage directory
#[tauri::command]
pub async fn get_models_dir(state: State<'_, Arc<AppState>>) -> Result<String> {
    let manager = state.model_manager.read().await;
    Ok(manager.models_dir().to_string_lossy().to_string())
}
