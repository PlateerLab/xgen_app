/**
 * Model Management IPC Wrapper
 *
 * Functions for managing local model files (list, download, delete).
 */

import { invoke } from '@tauri-apps/api/core';
import type { ModelInfo } from './types';

/**
 * List all available models
 *
 * @returns Array of model information
 *
 * @example
 * ```ts
 * const models = await listModels();
 * models.forEach(m => console.log(`${m.name}: ${m.sizeBytes / 1e9} GB`));
 * ```
 */
export async function listModels(): Promise<ModelInfo[]> {
  return invoke<ModelInfo[]>('list_models');
}

/**
 * Download a model from HuggingFace
 *
 * @param repoId - HuggingFace repository ID (e.g., "TheBloke/Llama-2-7B-GGUF")
 * @param filename - Model filename (e.g., "llama-2-7b.Q4_K_M.gguf")
 * @returns Downloaded model information
 *
 * @example
 * ```ts
 * const model = await downloadModel(
 *   'TheBloke/Llama-2-7B-GGUF',
 *   'llama-2-7b.Q4_K_M.gguf'
 * );
 * console.log(`Downloaded: ${model.path}`);
 * ```
 */
export async function downloadModel(
  repoId: string,
  filename: string
): Promise<ModelInfo> {
  return invoke<ModelInfo>('download_model', {
    repoId,
    filename,
  });
}

/**
 * Delete a model
 *
 * @param modelId - Model identifier
 *
 * @example
 * ```ts
 * await deleteModel('llama-2-7b.Q4_K_M');
 * ```
 */
export async function deleteModel(modelId: string): Promise<void> {
  return invoke<void>('delete_model', { modelId });
}

/**
 * Get model storage directory path
 *
 * @returns Path to the models directory
 *
 * @example
 * ```ts
 * const dir = await getModelsDir();
 * console.log(`Models stored in: ${dir}`);
 * ```
 */
export async function getModelsDir(): Promise<string> {
  return invoke<string>('get_models_dir');
}
