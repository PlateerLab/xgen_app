/**
 * LLM Inference IPC Wrapper
 *
 * Functions for model loading, text generation, and embeddings.
 * Streaming is handled via Tauri events.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ModelStatus, GenerateRequest, GenerateStats } from './types';

// ============================================================================
// Model Loading
// ============================================================================

export interface LoadModelOptions {
  /** Path to GGUF model file */
  modelPath: string;
  /** Model ID for identification */
  modelId?: string;
  /** Tokenizer model ID (HuggingFace repo) */
  tokenizerId?: string;
  /** Context length (default: 4096) */
  contextLength?: number;
  /** Enable paged attention for long contexts */
  pagedAttention?: boolean;
  /** Chat template (optional) */
  chatTemplate?: string;
}

/**
 * Load a model for inference
 *
 * Uses mistral.rs GgufModelBuilder with automatic device mapping.
 *
 * @param options - Model loading options
 * @returns Model status after loading
 *
 * @example
 * ```ts
 * const status = await loadModel({
 *   modelPath: '/path/to/llama.gguf',
 *   modelId: 'llama-3-8b',
 *   contextLength: 8192,
 * });
 * console.log(`Loaded on ${status.device}`);
 * ```
 */
export async function loadModel(options: LoadModelOptions): Promise<ModelStatus> {
  return invoke<ModelStatus>('load_model', {
    modelPath: options.modelPath,
    modelId: options.modelId,
    tokenizerId: options.tokenizerId,
    contextLength: options.contextLength,
    pagedAttention: options.pagedAttention,
    chatTemplate: options.chatTemplate,
  });
}

/**
 * Get current model status
 *
 * @returns Current model status
 *
 * @example
 * ```ts
 * const status = await getModelStatus();
 * if (status.loaded) {
 *   console.log(`Model: ${status.modelId} on ${status.device}`);
 * }
 * ```
 */
export async function getModelStatus(): Promise<ModelStatus> {
  return invoke<ModelStatus>('get_model_status');
}

/**
 * Unload the current model
 *
 * @example
 * ```ts
 * await unloadModel();
 * ```
 */
export async function unloadModel(): Promise<void> {
  return invoke<void>('unload_model');
}

// ============================================================================
// Text Generation
// ============================================================================

export interface GenerateOptions extends Omit<GenerateRequest, 'prompt'> {
  /** Callback for each generated token */
  onToken?: (token: string) => void;
  /** Callback when generation is complete */
  onDone?: (stats: GenerateStats) => void;
  /** Callback for errors */
  onError?: (error: string) => void;
}

/**
 * Generate text using the loaded model (streaming)
 *
 * Tokens are streamed via the onToken callback.
 *
 * @param prompt - Input prompt
 * @param options - Generation options and callbacks
 * @returns Cleanup function to remove event listeners
 *
 * @example
 * ```ts
 * let fullText = '';
 * const cleanup = await generate('Hello, how are you?', {
 *   maxTokens: 256,
 *   temperature: 0.7,
 *   onToken: (token) => {
 *     fullText += token;
 *     process.stdout.write(token);
 *   },
 *   onDone: (stats) => {
 *     console.log(`\n${stats.tokensPerSecond} tok/s`);
 *   },
 * });
 *
 * // Later: cleanup listeners
 * cleanup();
 * ```
 */
export async function generate(
  prompt: string,
  options: GenerateOptions = {}
): Promise<() => void> {
  const unlisteners: UnlistenFn[] = [];

  // Set up event listeners
  if (options.onToken) {
    const unlisten = await listen<string>('llm:token', (event) => {
      options.onToken?.(event.payload);
    });
    unlisteners.push(unlisten);
  }

  if (options.onDone) {
    const unlisten = await listen<GenerateStats>('llm:done', (event) => {
      options.onDone?.(event.payload);
    });
    unlisteners.push(unlisten);
  }

  if (options.onError) {
    const unlisten = await listen<string>('llm:error', (event) => {
      options.onError?.(event.payload);
    });
    unlisteners.push(unlisten);
  }

  // Start generation
  await invoke<void>('generate', {
    prompt,
    systemPrompt: options.systemPrompt,
    maxTokens: options.maxTokens,
    temperature: options.temperature,
    topP: options.topP,
    stopSequences: options.stopSequences,
  });

  // Return cleanup function
  return () => {
    unlisteners.forEach((unlisten) => unlisten());
  };
}

/**
 * Generate text (non-streaming)
 *
 * Returns the complete generated text.
 *
 * @param prompt - Input prompt
 * @param options - Generation options
 * @returns Generated text
 *
 * @example
 * ```ts
 * const response = await generateSync('What is 2+2?', {
 *   maxTokens: 100,
 * });
 * console.log(response);
 * ```
 */
export async function generateSync(
  prompt: string,
  options: Omit<GenerateOptions, 'onToken' | 'onDone' | 'onError'> = {}
): Promise<string> {
  return invoke<string>('generate_sync', {
    prompt,
    systemPrompt: options.systemPrompt,
    maxTokens: options.maxTokens,
    temperature: options.temperature,
    topP: options.topP,
  });
}

/**
 * Stop ongoing generation
 *
 * @example
 * ```ts
 * await stopGeneration();
 * ```
 */
export async function stopGeneration(): Promise<void> {
  return invoke<void>('stop_generation');
}

// ============================================================================
// Embeddings
// ============================================================================

/**
 * Generate embeddings for text
 *
 * @param texts - Array of texts to embed
 * @returns Array of embedding vectors
 *
 * @example
 * ```ts
 * const embeddings = await embedText(['Hello', 'World']);
 * console.log(`Dimension: ${embeddings[0].length}`);
 * ```
 */
export async function embedText(texts: string[]): Promise<number[][]> {
  return invoke<number[][]>('embed_text', { texts });
}
