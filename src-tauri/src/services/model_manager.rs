//! Model Manager
//!
//! Manages local model storage, downloading, and metadata.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{AppError, Result};

/// Model type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelType {
    /// Large Language Model for text generation
    Llm,

    /// Embedding model for vector generation
    Embedding,
}

impl std::fmt::Display for ModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelType::Llm => write!(f, "LLM"),
            ModelType::Embedding => write!(f, "Embedding"),
        }
    }
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique model identifier
    pub id: String,

    /// Display name
    pub name: String,

    /// Local file path
    pub path: PathBuf,

    /// File size in bytes
    pub size_bytes: u64,

    /// Model type
    pub model_type: ModelType,

    /// Quantization level (e.g., "Q4_K_M", "Q8_0")
    pub quantization: Option<String>,

    /// HuggingFace repository ID
    pub repo_id: Option<String>,

    /// Download date
    pub downloaded_at: Option<String>,
}

/// Model manager for handling local models
pub struct ModelManager {
    /// Directory for storing models
    models_dir: PathBuf,
}

impl ModelManager {
    /// Create a new ModelManager
    pub fn new() -> Self {
        let models_dir = Self::default_models_dir();

        // Ensure directory exists
        if let Err(e) = std::fs::create_dir_all(&models_dir) {
            log::warn!("Failed to create models directory: {}", e);
        }

        Self {
            models_dir,
        }
    }

    /// Get the default models directory
    fn default_models_dir() -> PathBuf {
        directories::ProjectDirs::from("com", "xgen", "app")
            .map(|d| d.data_dir().join("models"))
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".xgen")
                    .join("models")
            })
    }

    /// Get the models directory path
    pub fn models_dir(&self) -> &PathBuf {
        &self.models_dir
    }

    /// List all available models
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let mut models = Vec::new();

        if !self.models_dir.exists() {
            return Ok(models);
        }

        // Scan directory for GGUF files
        let entries = std::fs::read_dir(&self.models_dir)
            .map_err(|e| AppError::Io(e))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "gguf" {
                        let metadata = std::fs::metadata(&path)?;
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Unknown".to_string());

                        models.push(ModelInfo {
                            id: uuid::Uuid::new_v4().to_string(),
                            name: name.clone(),
                            path: path.clone(),
                            size_bytes: metadata.len(),
                            model_type: ModelType::Llm,
                            quantization: Self::detect_quantization(&name),
                            repo_id: None,
                            downloaded_at: None,
                        });
                    }
                }
            }
        }

        Ok(models)
    }

    /// Detect quantization from filename
    fn detect_quantization(name: &str) -> Option<String> {
        let quantization_patterns = [
            "Q2_K", "Q3_K_S", "Q3_K_M", "Q3_K_L",
            "Q4_0", "Q4_1", "Q4_K_S", "Q4_K_M",
            "Q5_0", "Q5_1", "Q5_K_S", "Q5_K_M",
            "Q6_K", "Q8_0", "F16", "F32",
        ];

        for pattern in quantization_patterns {
            if name.to_uppercase().contains(pattern) {
                return Some(pattern.to_string());
            }
        }

        None
    }

    /// Download a model from HuggingFace
    pub async fn download_model(&self, repo_id: &str, filename: &str) -> Result<ModelInfo> {
        log::info!("Downloading model: {}/{}", repo_id, filename);

        // TODO: Implement with hf-hub in Phase 2
        // For now, return an error
        Err(AppError::Model(
            "Model download not implemented yet (Phase 2)".to_string(),
        ))
    }

    /// Delete a model
    pub async fn delete_model(&self, model_id: &str) -> Result<()> {
        let models = self.list_models().await?;

        let model = models
            .iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| AppError::Model(format!("Model not found: {}", model_id)))?;

        std::fs::remove_file(&model.path)?;
        log::info!("Deleted model: {}", model.name);

        Ok(())
    }

    /// Get a model by ID
    pub async fn get_model(&self, model_id: &str) -> Result<ModelInfo> {
        let models = self.list_models().await?;

        models
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| AppError::Model(format!("Model not found: {}", model_id)))
    }
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}
