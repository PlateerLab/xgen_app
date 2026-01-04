/**
 * Tauri IPC Type Definitions
 *
 * TypeScript interfaces matching Rust backend types.
 * Auto-generated from src-tauri/src structs.
 */

// ============================================================================
// Hardware / GPU Types
// ============================================================================

/** Platform type for inference backend selection */
export type Platform = 'MacOS' | 'Windows' | 'Linux' | 'Unknown';

/** System hardware information */
export interface SystemInfo {
  /** Operating system name */
  osName: string;
  /** Operating system version */
  osVersion: string;
  /** CPU brand/model */
  cpuBrand: string;
  /** Number of CPU cores */
  cpuCores: number;
  /** Total RAM in bytes */
  totalMemory: number;
  /** Available RAM in bytes */
  availableMemory: number;
  /** Detected platform */
  platform: Platform;
}

/** Hardware status information */
export interface HardwareStatus {
  /** System information */
  system: SystemInfo;
  /** Whether CUDA might be available (hint) */
  cudaHint: boolean;
  /** Whether Metal is available */
  metalAvailable: boolean;
  /** Recommended backend hint */
  recommendedBackend: string;
}

// ============================================================================
// Model Management Types
// ============================================================================

/** Model type */
export type ModelType = 'Llm' | 'Embedding';

/** Model information */
export interface ModelInfo {
  /** Unique model identifier */
  id: string;
  /** Display name */
  name: string;
  /** Local file path */
  path: string;
  /** File size in bytes */
  sizeBytes: number;
  /** Model type */
  modelType: ModelType;
  /** Quantization level (e.g., "Q4_K_M", "Q8_0") */
  quantization: string | null;
  /** HuggingFace repository ID */
  repoId: string | null;
  /** Download date */
  downloadedAt: string | null;
}

// ============================================================================
// LLM Inference Types
// ============================================================================

/** Model configuration for loading */
export interface ModelConfig {
  /** Path to GGUF model file */
  modelPath: string;
  /** Model ID for identification */
  modelId: string;
  /** Tokenizer model ID (HuggingFace repo) */
  tokenizerId?: string;
  /** Context length (default: 4096) */
  contextLength?: number;
  /** Enable paged attention for long contexts */
  pagedAttention?: boolean;
  /** Chat template (optional) */
  chatTemplate?: string;
}

/** Model status information */
export interface ModelStatus {
  /** Whether a model is loaded */
  loaded: boolean;
  /** Model ID if loaded */
  modelId: string | null;
  /** Model path if loaded */
  modelPath: string | null;
  /** Device being used (auto-detected by mistral.rs) */
  device: string;
  /** Memory usage in bytes (if available) */
  memoryUsage: number | null;
}

/** Generation request parameters */
export interface GenerateRequest {
  /** Input prompt */
  prompt: string;
  /** System prompt (optional) */
  systemPrompt?: string;
  /** Maximum tokens to generate */
  maxTokens?: number;
  /** Temperature (0.0 - 2.0) */
  temperature?: number;
  /** Top-p sampling */
  topP?: number;
  /** Stop sequences */
  stopSequences?: string[];
}

/** Generation completion stats (emitted with llm:done event) */
export interface GenerateStats {
  /** Number of prompt tokens */
  promptTokens: number;
  /** Number of completion tokens */
  completionTokens: number;
  /** Generation time in milliseconds */
  generationTimeMs: number;
  /** Tokens per second */
  tokensPerSecond: number;
}

// ============================================================================
// MCP (Model Context Protocol) Types
// ============================================================================

/** MCP connection type */
export type McpConnectionType = 'stdio' | 'http';

/** MCP server status */
export interface McpServerStatus {
  /** Server name */
  name: string;
  /** Connection type */
  connectionType: string;
  /** Whether enabled */
  enabled: boolean;
  /** Description */
  description: string | null;
}

/** MCP tool info */
export interface ToolInfo {
  /** Tool name */
  name: string;
  /** Tool description */
  description: string | null;
  /** Source server */
  source: string;
  /** JSON schema */
  schema: unknown;
}

/** Parameters for adding an MCP server */
export interface AddMcpServerParams {
  /** Server name */
  name: string;
  /** Connection type: 'stdio' or 'http' */
  connectionType: McpConnectionType;
  /** Command for stdio connection */
  command?: string;
  /** Command arguments for stdio connection */
  args?: string[];
  /** URL for http connection */
  url?: string;
  /** Description */
  description?: string;
}

// ============================================================================
// App Mode Types
// ============================================================================

/** Application mode */
export type AppMode = 'standalone' | 'connected';

/** Application mode information */
export interface AppModeInfo {
  /** Current mode */
  mode: AppMode;
  /** Server URL (for connected mode) */
  serverUrl: string | null;
  /** Whether connected to server */
  connected: boolean;
}

// ============================================================================
// Event Types
// ============================================================================

/** LLM streaming events */
export interface LlmEvents {
  /** Token generated */
  'llm:token': string;
  /** Generation complete */
  'llm:done': GenerateStats;
  /** Generation error */
  'llm:error': string;
}

// ============================================================================
// Error Types
// ============================================================================

/** Tauri IPC error */
export interface TauriError {
  /** Error message */
  message: string;
  /** Error type */
  type?: string;
}
