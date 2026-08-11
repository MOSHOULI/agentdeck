/**
 * Tauri 命令调用桥接层。
 * 前端所有与 Rust 后端的通信都走这里，不从组件直接 invoke()。
 * 当不在 Tauri 环境中时（浏览器开发调试），返回 mock 数据。
 */

import { invoke } from '@tauri-apps/api/core';

// 浏览器调试模式（无 Tauri IPC）时返回 mock 数据
const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

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

const MOCK_AGENTS: AgentInfo[] = [
  {
    id: 'opencode',
    name: 'OpenCode',
    installed: true,
    version: '1.18.15',
    authVerified: true,
    source: 'open',
    uiType: 'all',
    stars: 194000,
    popular: true,
    description: '开源 AI 编程代理，支持 75+ 模型供应商、LSP 集成、Plan/Build 双模式。',
    icon: 'opencode',
    category: 'ai-coding',
  },
  {
    id: 'claude-code',
    name: 'Claude Code',
    installed: false,
    authVerified: false,
    source: 'closed',
    uiType: 'all',
    stars: 140000,
    popular: true,
    description: 'Anthropic 官方终端 AI 编程代理，MCP 原生支持、1M 上下文。',
    icon: 'claude-code',
    category: 'ai-coding',
  },
  {
    id: 'codex',
    name: 'OpenAI Codex',
    installed: true,
    version: '1.0.0',
    authVerified: true,
    source: 'open',
    uiType: 'cli+desktop',
    stars: 104000,
    popular: true,
    description: 'OpenAI 官方 AI 编程代理，沙箱安全执行、云端异步批处理。',
    icon: 'codex',
    category: 'ai-coding',
  },
];

const MOCK_MEMORIES: MemoryEntry[] = [
  {
    id: '1',
    agentId: 'codex',
    content: 'M0 收尾：cargo tauri dev 窗口跑通，环境工具链全部就绪（VS2022 Build Tools、Node 24、tauri-cli 2.11）。',
    collectedAt: Math.floor(Date.now() / 1000) - 3600,
    autoInject: true,
    tags: ['codex'],
  },
  {
    id: '2',
    agentId: 'opencode',
    content: 'OpenCode GO 套餐已接入，auth 中有 OpenCode Go api 凭据。',
    collectedAt: Math.floor(Date.now() / 1000) - 1800,
    autoInject: false,
    tags: ['opencode'],
  },
  {
    id: '3',
    agentId: 'codex',
    content: '注册表解析用 map key 回填 id，JSON 无需重复字段。',
    collectedAt: Math.floor(Date.now() / 1000) - 600,
    autoInject: false,
    tags: ['codex'],
  },
];

/** 获取所有注册的 agent 列表（含安装状态） */
export async function getAgents(): Promise<AgentInfo[]> {
  if (!isTauri) return MOCK_AGENTS;
  return invoke<AgentInfo[]>('get_agents');
}

/** 安装指定 agent */
export async function installAgent(id: string): Promise<void> {
  if (!isTauri) return;
  return invoke<void>('install_agent', { id });
}

/** 卸载指定 agent */
export async function uninstallAgent(id: string): Promise<void> {
  if (!isTauri) return;
  return invoke<void>('uninstall_agent', { id });
}

/** 验证 agent 认证状态 */
export async function verifyAgentAuth(id: string): Promise<boolean> {
  if (!isTauri) return true;
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
  if (!isTauri) return MOCK_MEMORIES.filter((m) => m.agentId === agentId);
  return invoke<MemoryEntry[]>('collect_memory', { agentId });
}

/** 搜索记忆 */
export async function searchMemory(query: string): Promise<MemoryEntry[]> {
  if (!isTauri) {
    const q = query.trim().toLowerCase();
    return MOCK_MEMORIES.filter(
      (m) => !q || m.content.toLowerCase().includes(q) || m.tags.some((t) => t.includes(q))
    );
  }
  return invoke<MemoryEntry[]>('search_memory', { query });
}

/** 注入记忆到指定 agent */
export async function injectMemory(agentId: string, memoryIds: string[]): Promise<void> {
  if (!isTauri) return;
  return invoke<void>('inject_memory', { agentId, memoryIds });
}

/** 切换记忆自动注入标签 */
export async function toggleAutoInject(memoryId: string, enabled: boolean): Promise<void> {
  if (!isTauri) return;
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
