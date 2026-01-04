/**
 * GPU / Hardware IPC Wrapper
 *
 * Functions for system hardware information.
 * Note: GPU detection and device mapping is handled by mistral.rs automatically.
 */

import { invoke } from '@tauri-apps/api/core';
import type { HardwareStatus } from './types';

/**
 * Get system hardware information
 *
 * Returns system info including CPU, RAM, and backend hints.
 * Note: mistral.rs handles actual GPU detection and device selection.
 *
 * @returns Hardware status including system info and backend recommendations
 *
 * @example
 * ```ts
 * const hardware = await getHardwareInfo();
 * console.log(`CPU: ${hardware.system.cpuBrand}`);
 * console.log(`RAM: ${hardware.system.totalMemory / 1024 / 1024 / 1024} GB`);
 * console.log(`Recommended: ${hardware.recommendedBackend}`);
 * ```
 */
export async function getHardwareInfo(): Promise<HardwareStatus> {
  return invoke<HardwareStatus>('get_hardware_info');
}
