/**
 * Tauri 命令调用桥接层。
 * 前端所有与 Rust 后端的通信都走这里，不从组件直接 invoke()。
 * 当不在 Tauri 环境中时（浏览器开发调试），返回 mock 数据。
 */

import { invoke } from '@tauri-apps/api/core';

// ─── Agent 管理 ────────────────────────────────────

export interface AgentInfo {
  id: string;
  name: string;
  installed: boolean;
  version?: string;
  authVerified: boolean;
  source: 'open' | 'closed';
  uiType: string;
  stars: number;
  popular?: boolean;
  description: string;
  icon: string;
  category: string;
}

/** 获取所有注册的 agent 列表（含安装状态） */
export async function getAgents(): Promise<AgentInfo[]> {
  return invoke<AgentInfo[]>('get_agents');
}

/** 安装指定 agent */
export async function installAgent(id: string): Promise<void> {
  return invoke<void>('install_agent', { id });
}

/** 卸载指定 agent */
export async function uninstallAgent(id: string): Promise<void> {
  return invoke<void>('uninstall_agent', { id });
}

/** 验证 agent 认证状态 */
export async function verifyAgentAuth(id: string): Promise<boolean> {
  return invoke<boolean>('verify_agent_auth', { id });
}

// ─── 记忆管理 ────────────────────────────────────

export interface MemoryEntry {
  id: string;
  agentId: string;
  content: string;
  collectedAt: number;
  autoInject: boolean;
  tags: string[];
}

/** 采集指定 agent 的记忆 */
export async function collectMemory(agentId: string): Promise<MemoryEntry[]> {
  return invoke<MemoryEntry[]>('collect_memory', { agentId });
}

/** 搜索记忆 */
export async function searchMemory(query: string): Promise<MemoryEntry[]> {
  return invoke<MemoryEntry[]>('search_memory', { query });
}

/** 注入记忆到指定 agent */
export async function injectMemory(agentId: string, memoryIds: string[]): Promise<void> {
  return invoke<void>('inject_memory', { agentId, memoryIds });
}

/** 切换记忆自动注入标签 */
export async function toggleAutoInject(memoryId: string, enabled: boolean): Promise<void> {
  return invoke<void>('toggle_auto_inject', { memoryId, enabled });
}

// ─── MCP 管理 ────────────────────────────────────

export interface McpServer {
  name: string;
  description: string;
  enabled: boolean;
  enabledAgents: string[];
}

/** 获取 MCP 服务器列表 */
export async function getMcpServers(): Promise<McpServer[]> {
  return invoke<McpServer[]>('get_mcp_servers');
}

/** 切换 agent 的 MCP 服务器开关 */
export async function toggleMcpForAgent(
  serverName: string,
  agentId: string,
  enabled: boolean
): Promise<void> {
  return invoke<void>('toggle_mcp_for_agent', { serverName, agentId, enabled });
}

// ─── 密钥管理 ────────────────────────────────────

/** 保存 API Key（加密存入 vault） */
export async function saveApiKey(agentId: string, provider: string, key: string): Promise<void> {
  return invoke<void>('save_api_key', { agentId, provider, key });
}

/** 列出已存储的 API Key（脱敏显示） */
export async function listApiKeys(): Promise<Array<{ agentId: string; provider: string; maskedKey: string }>> {
  return invoke<Array<{ agentId: string; provider: string; maskedKey: string }>>('list_api_keys');
}
