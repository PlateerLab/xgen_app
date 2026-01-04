/**
 * App Mode IPC Wrapper
 *
 * Functions for managing app mode (Standalone/Connected).
 */

import { invoke } from '@tauri-apps/api/core';
import type { AppMode, AppModeInfo } from './types';

/**
 * Set the application mode
 *
 * @param mode - 'standalone' or 'connected'
 * @param serverUrl - Server URL (required for connected mode)
 *
 * @example
 * ```ts
 * // Switch to standalone mode
 * await setAppMode('standalone');
 *
 * // Switch to connected mode
 * await setAppMode('connected', 'http://gateway.example.com');
 * ```
 */
export async function setAppMode(mode: AppMode, serverUrl?: string): Promise<void> {
  return invoke<void>('set_app_mode', { mode, serverUrl });
}

/**
 * Get the current application mode
 *
 * @returns Current mode information
 *
 * @example
 * ```ts
 * const info = await getAppMode();
 * if (info.mode === 'connected') {
 *   console.log(`Connected to ${info.serverUrl}`);
 * }
 * ```
 */
export async function getAppMode(): Promise<AppModeInfo> {
  return invoke<AppModeInfo>('get_app_mode');
}

/**
 * Check connection to gateway server
 *
 * Only works in connected mode.
 *
 * @returns Whether connected to the gateway
 *
 * @example
 * ```ts
 * const isConnected = await checkGatewayConnection();
 * console.log(isConnected ? 'Online' : 'Offline');
 * ```
 */
export async function checkGatewayConnection(): Promise<boolean> {
  return invoke<boolean>('check_gateway_connection');
}
