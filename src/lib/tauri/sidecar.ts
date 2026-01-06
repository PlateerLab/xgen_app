/**
 * Sidecar Management API
 *
 * Manages Python sidecar processes (xgen-workflow, etc.) for Service Mode.
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

/** Sidecar service status */
export interface SidecarStatus {
  /** Service name (e.g., "xgen-workflow") */
  name: string;
  /** Whether the service is running */
  running: boolean;
  /** Port the service is listening on */
  port: number;
  /** Full URL to the service */
  url: string;
  /** Process ID if running */
  pid: number | null;
  /** Whether health check passed */
  healthOk: boolean;
}

/** Current app mode information */
export interface AppModeInfo {
  /** Mode type: "standalone", "service", or "connected" */
  mode: 'standalone' | 'service' | 'connected';
  /** Service URL if in service or connected mode */
  serviceUrl: string | null;
  /** Running service name (for service mode) */
  serviceName: string | null;
}

/** Environment variables for sidecar */
export type SidecarEnv = Record<string, string>;

// ============================================================================
// Sidecar Process Management
// ============================================================================

/**
 * Start a sidecar service
 * @param name Service name (e.g., "xgen-workflow")
 * @param env Optional environment variables
 */
export async function startSidecar(
  name: string,
  env?: SidecarEnv
): Promise<SidecarStatus> {
  return await invoke<SidecarStatus>('start_sidecar', { name, env });
}

/**
 * Stop a running sidecar service
 * @param name Service name
 */
export async function stopSidecar(name: string): Promise<void> {
  return await invoke('stop_sidecar', { name });
}

/**
 * Stop all running sidecars
 */
export async function stopAllSidecars(): Promise<void> {
  return await invoke('stop_all_sidecars');
}

/**
 * Get status of a specific sidecar
 * @param name Service name
 */
export async function getSidecarStatus(name: string): Promise<SidecarStatus> {
  return await invoke<SidecarStatus>('get_sidecar_status', { name });
}

/**
 * Get status of all sidecars
 */
export async function getAllSidecarStatus(): Promise<SidecarStatus[]> {
  return await invoke<SidecarStatus[]>('get_all_sidecar_status');
}

/**
 * List all registered sidecar names
 */
export async function listSidecars(): Promise<string[]> {
  return await invoke<string[]>('list_sidecars');
}

// ============================================================================
// Mode Switching
// ============================================================================

/**
 * Enable Service Mode
 * Starts the specified sidecar and switches to using it for API calls.
 *
 * @param serviceName Service to start (e.g., "xgen-workflow")
 * @param env Optional environment variables
 */
export async function enableServiceMode(
  serviceName: string = 'xgen-workflow',
  env?: SidecarEnv
): Promise<SidecarStatus> {
  return await invoke<SidecarStatus>('enable_service_mode', {
    serviceName,
    env,
  });
}

/**
 * Enable Standalone Mode
 * Stops any running sidecars and switches to local mistral.rs inference.
 */
export async function enableStandaloneMode(): Promise<void> {
  return await invoke('enable_standalone_mode');
}

/**
 * Get current app mode information
 */
export async function getCurrentMode(): Promise<AppModeInfo> {
  return await invoke<AppModeInfo>('get_current_mode');
}

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Check if xgen-workflow service is running
 */
export async function isWorkflowServiceRunning(): Promise<boolean> {
  try {
    const status = await getSidecarStatus('xgen-workflow');
    return status.running && status.healthOk;
  } catch {
    return false;
  }
}

/**
 * Get the workflow service URL if running
 */
export async function getWorkflowServiceUrl(): Promise<string | null> {
  try {
    const status = await getSidecarStatus('xgen-workflow');
    return status.running ? status.url : null;
  } catch {
    return null;
  }
}

/**
 * Start xgen-workflow with common environment variables
 */
export async function startWorkflowService(options?: {
  postgresHost?: string;
  redisHost?: string;
  port?: number;
  promptsCsvPath?: string;
}): Promise<SidecarStatus> {
  const env: SidecarEnv = {};

  if (options?.postgresHost) {
    env['POSTGRES_HOST'] = options.postgresHost;
  }
  if (options?.redisHost) {
    env['REDIS_HOST'] = options.redisHost;
  }
  if (options?.port) {
    env['APP_PORT'] = String(options.port);
  }
  if (options?.promptsCsvPath) {
    env['PROMPTS_CSV_PATH'] = options.promptsCsvPath;
  }

  return await startSidecar('xgen-workflow', env);
}
