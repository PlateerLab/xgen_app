/**
 * MCP (Model Context Protocol) IPC Wrapper
 *
 * Functions for managing MCP server configurations.
 * The actual MCP client is provided by mistralrs_mcp when loading models.
 */

import { invoke } from '@tauri-apps/api/core';
import type { McpServerStatus, McpConnectionType, AddMcpServerParams } from './types';

/**
 * List all configured MCP servers
 *
 * @returns Array of MCP server statuses
 *
 * @example
 * ```ts
 * const servers = await listMcpServers();
 * servers.forEach(s => console.log(`${s.name}: ${s.enabled ? 'on' : 'off'}`));
 * ```
 */
export async function listMcpServers(): Promise<McpServerStatus[]> {
  return invoke<McpServerStatus[]>('list_mcp_servers');
}

/**
 * Add a new MCP server configuration
 *
 * @param params - Server configuration parameters
 * @returns Created server status
 *
 * @example
 * ```ts
 * // Add stdio server
 * await addMcpServer({
 *   name: 'filesystem',
 *   connectionType: 'stdio',
 *   command: 'npx',
 *   args: ['-y', '@modelcontextprotocol/server-filesystem', '.'],
 *   description: 'File system access',
 * });
 *
 * // Add HTTP server
 * await addMcpServer({
 *   name: 'remote-tools',
 *   connectionType: 'http',
 *   url: 'http://localhost:3000',
 * });
 * ```
 */
export async function addMcpServer(params: AddMcpServerParams): Promise<McpServerStatus> {
  return invoke<McpServerStatus>('add_mcp_server', {
    name: params.name,
    connectionType: params.connectionType,
    command: params.command,
    args: params.args,
    url: params.url,
    description: params.description,
  });
}

/**
 * Remove an MCP server configuration
 *
 * @param name - Server name
 *
 * @example
 * ```ts
 * await removeMcpServer('filesystem');
 * ```
 */
export async function removeMcpServer(name: string): Promise<void> {
  return invoke<void>('remove_mcp_server', { name });
}

/**
 * Enable or disable an MCP server
 *
 * @param name - Server name
 * @param enabled - Whether to enable the server
 *
 * @example
 * ```ts
 * await setMcpServerEnabled('filesystem', true);
 * ```
 */
export async function setMcpServerEnabled(name: string, enabled: boolean): Promise<void> {
  return invoke<void>('set_mcp_server_enabled', { name, enabled });
}

/**
 * Get the count of enabled MCP servers
 *
 * @returns Number of enabled servers
 *
 * @example
 * ```ts
 * const count = await getEnabledMcpCount();
 * console.log(`${count} MCP servers active`);
 * ```
 */
export async function getEnabledMcpCount(): Promise<number> {
  return invoke<number>('get_enabled_mcp_count');
}

/**
 * Check if any MCP servers are enabled
 *
 * @returns Whether any servers are enabled
 *
 * @example
 * ```ts
 * if (await hasEnabledMcpServers()) {
 *   console.log('MCP tools available');
 * }
 * ```
 */
export async function hasEnabledMcpServers(): Promise<boolean> {
  return invoke<boolean>('has_enabled_mcp_servers');
}
