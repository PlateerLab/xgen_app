//! Inference Engine
//!
//! LLM inference using mistral.rs with MCP tool support.
//! mistral.rs handles:
//! - Automatic GPU detection and device mapping (CUDA/Metal/CPU)
//! - GGUF model loading with quantization
//! - Streaming token generation
//! - MCP tool integration via mistralrs_mcp
//!
//! ## Example
//! ```rust,ignore
//! use mistralrs::{GgufModelBuilder, TextMessages, TextMessageRole};
//!
//! let model = GgufModelBuilder::new("model-id", vec!["model.gguf"])
//!     .with_logging()
//!     .build()
//!     .await?;
//!
//! let messages = TextMessages::new()
//!     .add_message(TextMessageRole::User, "Hello!");
//!
//! let response = model.send_chat_request(messages).await?;
//! ```

use std::sync::Arc;
use tokio::sync::RwLock;

use mistralrs::{
    best_device, GgufModelBuilder, Model as MistralModel, TextMessageRole, TextMessages,
    PagedAttentionMetaBuilder, Response,
};
use mistralrs_mcp::{McpClient, McpClientConfig};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

/// Model configuration for loading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Path to GGUF model file
    pub model_path: String,

    /// Model ID for identification (HuggingFace repo or local identifier)
    pub model_id: String,

    /// Tokenizer model ID (HuggingFace repo for tokenizer)
    pub tokenizer_id: Option<String>,

    /// Context length (default: 4096)
    pub context_length: Option<usize>,

    /// Enable paged attention for long contexts
    pub paged_attention: bool,

    /// Chat template (optional, for custom formatting)
    pub chat_template: Option<String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            model_id: String::new(),
            tokenizer_id: None,
            context_length: Some(4096),
            paged_attention: false, // Disable by default for stability
            chat_template: None,
        }
    }
}

/// Generation request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// Input prompt or messages
    pub prompt: String,

    /// System prompt (optional)
    pub system_prompt: Option<String>,

    /// Maximum tokens to generate (note: may not be respected by all model configs)
    pub max_tokens: usize,

    /// Temperature (0.0 - 2.0) - currently uses default
    pub temperature: Option<f32>,

    /// Top-p sampling - currently uses default
    pub top_p: Option<f32>,

    /// Stop sequences
    pub stop_sequences: Option<Vec<String>>,

    /// Whether to stream tokens
    pub stream: bool,
}

impl Default for GenerateRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            system_prompt: None,
            max_tokens: 512,
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_sequences: None,
            stream: true,
        }
    }
}

/// Generation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    /// Generated text
    pub text: String,

    /// Number of prompt tokens
    pub prompt_tokens: usize,

    /// Number of completion tokens
    pub completion_tokens: usize,

    /// Generation time in milliseconds
    pub generation_time_ms: u64,

    /// Tokens per second
    pub tokens_per_second: f32,
}

/// Embedding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedRequest {
    /// Texts to embed
    pub texts: Vec<String>,
}

/// Embedding response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResponse {
    /// Embedding vectors
    pub embeddings: Vec<Vec<f32>>,

    /// Embedding dimension
    pub dimension: usize,
}

/// Model status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    /// Whether a model is loaded
    pub loaded: bool,

    /// Model ID if loaded
    pub model_id: Option<String>,

    /// Model path if loaded
    pub model_path: Option<String>,

    /// Device being used (auto-detected by mistral.rs)
    pub device: String,

    /// Memory usage in bytes (if available)
    pub memory_usage: Option<u64>,
}

/// Inference engine for LLM generation using mistral.rs
pub struct InferenceEngine {
    /// Loaded model instance
    model: Option<MistralModel>,

    /// Current model configuration
    config: Option<ModelConfig>,

    /// Model status
    status: ModelStatus,

    /// Cancellation flag for stopping generation
    cancel_flag: Arc<RwLock<bool>>,

    /// MCP client for tool integration (optional)
    mcp_client: Option<McpClient>,

    /// Number of MCP tools registered
    mcp_tool_count: usize,
}

impl InferenceEngine {
    /// Create a new InferenceEngine
    pub fn new() -> Self {
        Self {
            model: None,
            config: None,
            status: ModelStatus {
                loaded: false,
                model_id: None,
                model_path: None,
                device: "none".to_string(),
                memory_usage: None,
            },
            cancel_flag: Arc::new(RwLock::new(false)),
            mcp_client: None,
            mcp_tool_count: 0,
        }
    }

    /// Get the number of MCP tools registered
    pub fn mcp_tool_count(&self) -> usize {
        self.mcp_tool_count
    }

    /// Check if MCP tools are available
    pub fn has_mcp_tools(&self) -> bool {
        self.mcp_tool_count > 0
    }

    /// Check if a model is loaded
    pub fn is_loaded(&self) -> bool {
        self.status.loaded
    }

    /// Get current model status
    pub fn status(&self) -> &ModelStatus {
        &self.status
    }

    /// Get the loaded model configuration
    pub fn config(&self) -> Option<&ModelConfig> {
        self.config.as_ref()
    }

    /// Get the loaded model (for advanced usage)
    pub fn model(&self) -> Option<&MistralModel> {
        self.model.as_ref()
    }

    /// Load a GGUF model
    ///
    /// Uses GgufModelBuilder for local GGUF files.
    /// mistral.rs automatically detects and uses the best available device.
    pub async fn load_model(&mut self, config: ModelConfig) -> Result<ModelStatus> {
        log::info!("Loading model: {} from {}", config.model_id, config.model_path);

        // Validate path exists
        let model_path = std::path::Path::new(&config.model_path);
        if !model_path.exists() {
            return Err(AppError::Model(format!(
                "Model file not found: {}",
                config.model_path
            )));
        }

        // Extract filename from path
        let filename = model_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::Model("Invalid model path".to_string()))?;

        // Detect best device
        let device = best_device(false).map_err(|e| AppError::Inference(e.to_string()))?;
        let device_name = format!("{:?}", device);
        log::info!("Using device: {}", device_name);

        // Build the model
        // Use model_id as HuggingFace repo, or use a local path indicator
        let mut builder = GgufModelBuilder::new(
            &config.model_id,
            vec![filename.to_string()],
        )
        .with_logging();

        // Set tokenizer if provided
        if let Some(ref tok_id) = config.tokenizer_id {
            builder = builder.with_tok_model_id(tok_id);
        }

        // Set chat template if provided
        if let Some(ref template) = config.chat_template {
            builder = builder.with_chat_template(template.clone());
        }

        // Enable paged attention if requested (may not work on all systems)
        if config.paged_attention {
            builder = builder.with_paged_attn(|| {
                PagedAttentionMetaBuilder::default().build()
            }).map_err(|e| AppError::Model(e.to_string()))?;
        }

        // Build the model
        let model = builder
            .build()
            .await
            .map_err(|e| AppError::Model(format!("Failed to load model: {}", e)))?;

        self.model = Some(model);
        self.config = Some(config.clone());
        self.status = ModelStatus {
            loaded: true,
            model_id: Some(config.model_id),
            model_path: Some(config.model_path),
            device: device_name,
            memory_usage: None,
        };

        log::info!("Model loaded successfully on device: {}", self.status.device);
        Ok(self.status.clone())
    }

    /// Load a GGUF model with MCP tool support
    ///
    /// Initializes MCP client and registers tool callbacks with the model.
    /// MCP tools become available for the model to call during generation.
    pub async fn load_model_with_mcp(
        &mut self,
        config: ModelConfig,
        mcp_config: McpClientConfig,
    ) -> Result<ModelStatus> {
        log::info!(
            "Loading model with MCP: {} from {} ({} MCP servers)",
            config.model_id,
            config.model_path,
            mcp_config.servers.len()
        );

        // Validate path exists
        let model_path = std::path::Path::new(&config.model_path);
        if !model_path.exists() {
            return Err(AppError::Model(format!(
                "Model file not found: {}",
                config.model_path
            )));
        }

        // Extract filename from path
        let filename = model_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::Model("Invalid model path".to_string()))?;

        // Detect best device
        let device = best_device(false).map_err(|e| AppError::Inference(e.to_string()))?;
        let device_name = format!("{:?}", device);
        log::info!("Using device: {}", device_name);

        // Initialize MCP client
        let mut mcp_client = McpClient::new(mcp_config);
        mcp_client
            .initialize()
            .await
            .map_err(|e| AppError::Mcp(format!("Failed to initialize MCP client: {}", e)))?;

        // Get tool callbacks from MCP client
        let tool_callbacks = mcp_client.get_tool_callbacks_with_tools();
        let tool_count = tool_callbacks.len();
        log::info!("Registered {} MCP tools", tool_count);

        // Build the model with MCP tools
        let mut builder = GgufModelBuilder::new(&config.model_id, vec![filename.to_string()])
            .with_logging();

        // Set tokenizer if provided
        if let Some(ref tok_id) = config.tokenizer_id {
            builder = builder.with_tok_model_id(tok_id);
        }

        // Set chat template if provided
        if let Some(ref template) = config.chat_template {
            builder = builder.with_chat_template(template.clone());
        }

        // Enable paged attention if requested
        if config.paged_attention {
            builder = builder
                .with_paged_attn(|| PagedAttentionMetaBuilder::default().build())
                .map_err(|e| AppError::Model(e.to_string()))?;
        }

        // Register MCP tool callbacks with the model
        for (name, callback_with_tool) in tool_callbacks {
            log::debug!("Registering MCP tool: {}", name);
            builder = builder.with_tool_callback_and_tool(
                name.clone(),
                callback_with_tool.callback.clone(),
                callback_with_tool.tool.clone(),
            );
        }

        // Build the model
        let model = builder
            .build()
            .await
            .map_err(|e| AppError::Model(format!("Failed to load model: {}", e)))?;

        self.model = Some(model);
        self.config = Some(config.clone());
        self.mcp_client = Some(mcp_client);
        self.mcp_tool_count = tool_count;
        self.status = ModelStatus {
            loaded: true,
            model_id: Some(config.model_id),
            model_path: Some(config.model_path),
            device: device_name,
            memory_usage: None,
        };

        log::info!(
            "Model loaded successfully with {} MCP tools on device: {}",
            tool_count,
            self.status.device
        );
        Ok(self.status.clone())
    }

    /// Unload the current model
    pub fn unload(&mut self) -> Result<()> {
        if !self.status.loaded {
            return Err(AppError::Inference("No model loaded".to_string()));
        }

        self.model = None;
        self.config = None;
        self.mcp_client = None;
        self.mcp_tool_count = 0;
        self.status = ModelStatus {
            loaded: false,
            model_id: None,
            model_path: None,
            device: "none".to_string(),
            memory_usage: None,
        };

        log::info!("Model unloaded");
        Ok(())
    }

    /// Generate text (non-streaming)
    ///
    /// Uses TextMessages for simple chat completion.
    /// Note: Sampling parameters (temperature, top_p) use default values.
    pub async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse> {
        let model = self.model.as_ref()
            .ok_or_else(|| AppError::Inference("No model loaded".to_string()))?;

        log::info!(
            "Generating: max {} tokens from {} chars",
            request.max_tokens,
            request.prompt.len()
        );

        let start = std::time::Instant::now();

        // Build messages
        let mut messages = TextMessages::new();

        if let Some(ref system) = request.system_prompt {
            messages = messages.add_message(TextMessageRole::System, system);
        }

        messages = messages.add_message(TextMessageRole::User, &request.prompt);

        // Send request (uses deterministic sampling by default)
        let response = model
            .send_chat_request(messages)
            .await
            .map_err(|e| AppError::Inference(format!("Generation failed: {}", e)))?;

        let generation_time_ms = start.elapsed().as_millis() as u64;

        // Extract response text
        let text = response.choices.first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        let prompt_tokens = response.usage.prompt_tokens;
        let completion_tokens = response.usage.completion_tokens;

        let tokens_per_second = if generation_time_ms > 0 {
            (completion_tokens as f32 / generation_time_ms as f32) * 1000.0
        } else {
            0.0
        };

        log::info!(
            "Generation complete: {} tokens in {}ms ({:.1} tok/s)",
            completion_tokens,
            generation_time_ms,
            tokens_per_second
        );

        Ok(GenerateResponse {
            text,
            prompt_tokens,
            completion_tokens,
            generation_time_ms,
            tokens_per_second,
        })
    }

    /// Generate text with streaming using a callback
    ///
    /// Calls the provided callback for each token chunk.
    /// This is more ergonomic than returning a Stream since mistral.rs
    /// uses its own Stream type that doesn't implement futures::Stream.
    pub async fn generate_stream_with_callback<F>(
        &self,
        request: GenerateRequest,
        mut callback: F,
    ) -> Result<GenerateResponse>
    where
        F: FnMut(String) + Send,
    {
        let model = self.model.as_ref()
            .ok_or_else(|| AppError::Inference("No model loaded".to_string()))?;

        log::info!(
            "Streaming generation: max {} tokens",
            request.max_tokens
        );

        let start = std::time::Instant::now();

        // Build messages
        let mut messages = TextMessages::new();

        if let Some(ref system) = request.system_prompt {
            messages = messages.add_message(TextMessageRole::System, system);
        }

        messages = messages.add_message(TextMessageRole::User, &request.prompt);

        // Get streaming response
        let mut stream = model
            .stream_chat_request(messages)
            .await
            .map_err(|e| AppError::Inference(format!("Stream failed: {}", e)))?;

        let mut full_text = String::new();
        let mut prompt_tokens = 0usize;
        let mut completion_tokens = 0usize;

        // Iterate using mistral.rs Stream's native next() method
        while let Some(response) = stream.next().await {
            match response {
                Response::Chunk(chunk) => {
                    if let Some(choice) = chunk.choices.first() {
                        if let Some(ref content) = choice.delta.content {
                            if !content.is_empty() {
                                full_text.push_str(content);
                                callback(content.clone());
                            }
                        }
                    }
                }
                Response::Done(done) => {
                    prompt_tokens = done.usage.prompt_tokens;
                    completion_tokens = done.usage.completion_tokens;
                    break;
                }
                Response::InternalError(e) => {
                    return Err(AppError::Inference(e.to_string()));
                }
                Response::ValidationError(e) => {
                    return Err(AppError::Inference(e.to_string()));
                }
                Response::ModelError(msg, _) => {
                    return Err(AppError::Inference(msg));
                }
                _ => {}
            }
        }

        let generation_time_ms = start.elapsed().as_millis() as u64;
        let tokens_per_second = if generation_time_ms > 0 {
            (completion_tokens as f32 / generation_time_ms as f32) * 1000.0
        } else {
            0.0
        };

        log::info!(
            "Streaming generation complete: {} tokens in {}ms ({:.1} tok/s)",
            completion_tokens,
            generation_time_ms,
            tokens_per_second
        );

        Ok(GenerateResponse {
            text: full_text,
            prompt_tokens,
            completion_tokens,
            generation_time_ms,
            tokens_per_second,
        })
    }

    /// Generate embeddings
    ///
    /// Note: Requires an embedding model to be loaded.
    /// Standard LLM models don't support embeddings directly.
    pub async fn embed(&self, request: EmbedRequest) -> Result<EmbedResponse> {
        if !self.status.loaded {
            return Err(AppError::Inference("No model loaded".to_string()));
        }

        log::info!("Embedding {} texts", request.texts.len());

        // Note: mistral.rs embedding requires EmbeddingModelBuilder
        // For now, return a placeholder - implement when embedding model support is added
        log::warn!("Embedding not yet implemented - returning placeholder");

        let dimension = 384;
        let embeddings = request
            .texts
            .iter()
            .map(|_| vec![0.0f32; dimension])
            .collect();

        Ok(EmbedResponse {
            embeddings,
            dimension,
        })
    }

    /// Stop ongoing generation
    pub async fn stop(&self) -> Result<()> {
        log::info!("Stop generation requested");
        let mut cancel = self.cancel_flag.write().await;
        *cancel = true;
        Ok(())
    }

    /// Reset cancellation flag
    pub async fn reset_cancel(&self) {
        let mut cancel = self.cancel_flag.write().await;
        *cancel = false;
    }
}

impl Default for InferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inference_engine_status() {
        let engine = InferenceEngine::new();
        assert!(!engine.is_loaded());
        assert_eq!(engine.status().device, "none");
    }

    #[tokio::test]
    async fn test_generate_without_model() {
        let engine = InferenceEngine::new();
        let request = GenerateRequest {
            prompt: "Hello".to_string(),
            ..Default::default()
        };
        let result = engine.generate(request).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert!(!config.paged_attention);
        assert_eq!(config.context_length, Some(4096));
    }

    #[test]
    fn test_generate_request_default() {
        let request = GenerateRequest::default();
        assert_eq!(request.max_tokens, 512);
        assert_eq!(request.temperature, Some(0.7));
    }
}
