/**
 * Tauri IPC Wrapper (Legacy)
 *
 * @deprecated 이 모듈은 레거시입니다. 새로운 API 추상화 레이어를 사용하세요:
 *
 * ```ts
 * // 새로운 방식 (권장)
 * import { llm, auth, workflow } from '@/app/_common/api/domains';
 *
 * // 또는 직접 클라이언트 사용
 * import { createApiClient, createLLMClient } from '@/app/_common/api/core';
 * ```
 *
 * 새 API는 웹과 Tauri 환경을 자동으로 감지하여 적절한 클라이언트를 사용합니다.
 *
 * @example (Legacy - 기존 코드 호환용)
 * ```ts
 * import { llm, model, mcp, gpu, mode } from '@/lib/tauri';
 *
 * // Load a model
 * const status = await llm.loadModel({ modelPath: '/path/to/model.gguf' });
 *
 * // Generate text with streaming
 * await llm.generate('Hello!', {
 *   onToken: (token) => console.log(token),
 *   onDone: (stats) => console.log(`${stats.tokensPerSecond} tok/s`),
 * });
 * ```
 */

// Re-export all types
export * from './types';

// Re-export modules as namespaces
import * as gpu from './gpu';
import * as model from './model';
import * as llm from './llm';
import * as mcp from './mcp';
import * as mode from './mode';

export { gpu, model, llm, mcp, mode };

// Also export individual functions for convenience
export { getHardwareInfo } from './gpu';
export { listModels, downloadModel, deleteModel, getModelsDir } from './model';
export {
  loadModel,
  getModelStatus,
  unloadModel,
  generate,
  generateSync,
  stopGeneration,
  embedText,
} from './llm';
export {
  listMcpServers,
  addMcpServer,
  removeMcpServer,
  setMcpServerEnabled,
  getEnabledMcpCount,
  hasEnabledMcpServers,
} from './mcp';
export { setAppMode, getAppMode, checkGatewayConnection } from './mode';
