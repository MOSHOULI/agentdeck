/**
 * Tauri 命令调用桥接层。
 * 前端所有与 Rust 后端的通信都走这里，不从组件直接 invoke()。
 * 浏览器默认拒绝 IPC；仅 ?demo=1 显式展示只读示例。
 */

import { invoke } from '@tauri-apps/api/core';

export function runtimeMode(): 'native' | 'demo' | 'unavailable' {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) return 'native';
  return typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('demo') === '1'
    ? 'demo' : 'unavailable';
}

function useDemo(write = false): boolean {
  const mode = runtimeMode();
  if (mode === 'native') return false;
  if (mode === 'unavailable') throw new Error('未连接桌面后端。请通过 AgentDeck 桌面应用运行；浏览器只读演示请使用 ?demo=1。');
  if (write) throw new Error('只读演示模式：不会安装软件、读取密钥或修改本地配置。请使用桌面应用。');
  return true;
}

// ─── Agent 管理 ────────────────────────────────────

export interface AgentInfo {
  id: string;
  name: string;
  installed: boolean;
  canInstall?: boolean;
  canVerifyInstall?: boolean;
  canUninstall?: boolean;
  installHelp?: string | null;
  homepage?: string | null;
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
    content: '示例记忆：项目使用 Rust 和 React，修改后运行测试。',
    collectedAt: Math.floor(Date.now() / 1000) - 3600,
    autoInject: true,
    tags: ['codex'],
  },
  {
    id: '2',
    agentId: 'opencode',
    content: '示例记忆：界面偏好暗色主题，操作需要明确反馈。',
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
  if (useDemo()) return MOCK_AGENTS;
  return invoke<AgentInfo[]>('get_agents');
}

export interface InstallOutcome { warnings: string[]; }

/** 安装指定 agent，保留配置跳过警告 */
export async function installAgent(id: string): Promise<InstallOutcome> {
  useDemo(true);
  return invoke<InstallOutcome>('install_agent', { id });
}

/** 卸载指定 agent */
export async function uninstallAgent(id: string): Promise<void> {
  if (useDemo(true)) return;
  return invoke<void>('uninstall_agent', { id });
}

/** 验证 agent 认证状态 */
export async function verifyAgentAuth(id: string): Promise<boolean> {
  if (useDemo(true)) return true;
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
  contentHash?: string;
  sourceSessionId?: string | null;
}

/** 采集指定 agent 的记忆 */
export async function collectMemory(agentId: string): Promise<MemoryEntry[]> {
  if (useDemo(true)) return MOCK_MEMORIES.filter((m) => m.agentId === agentId);
  return invoke<MemoryEntry[]>('collect_memory', { agentId });
}

/** 搜索记忆 */
export async function searchMemory(query: string): Promise<MemoryEntry[]> {
  if (useDemo()) {
    const q = query.trim().toLowerCase();
    return MOCK_MEMORIES.filter(
      (m) => !q || m.content.toLowerCase().includes(q) || m.tags.some((t) => t.includes(q))
    );
  }
  return invoke<MemoryEntry[]>('search_memory', { query });
}

/** 注入记忆到指定 agent */
export async function injectMemory(agentId: string, memoryIds: string[]): Promise<void> {
  if (useDemo(true)) return;
  return invoke<void>('inject_memory', { agentId, memoryIds });
}

/** 切换记忆自动注入标签 */
export async function toggleAutoInject(memoryId: string, enabled: boolean): Promise<void> {
  if (useDemo(true)) return;
  return invoke<void>('toggle_auto_inject', { memoryId, enabled });
}

// ─── MCP 管理 ────────────────────────────────────

export interface McpServer {
  name: string;
  description: string;
  kind: 'http' | 'stdio';
  url?: string;
  command?: string;
  args: string[];
  supportedAgents: string[];
  enabledAgents: string[];
}

// MOCK: 与 agents/registry-v3.json 的 config_inject.mcp 目标保持一致
const MOCK_MCP_SERVERS: McpServer[] = [
  {
    name: 'ragflow',
    description: 'RAGFlow 本地知识库 MCP（只读检索：ragflow_retrieval / ragflow_list_datasets）',
    kind: 'http',
    url: 'http://127.0.0.1:9382/mcp',
    args: [],
    supportedAgents: ['codex', 'claude-code', 'opencode', 'gemini-cli', 'hermes', 'cline'],
    enabledAgents: ['codex', 'opencode'],
  },
];

/** 获取 MCP 服务器列表 */
export async function getMcpServers(): Promise<McpServer[]> {
  if (useDemo()) return MOCK_MCP_SERVERS;
  return invoke<McpServer[]>('get_mcp_servers');
}

/** 切换 agent 的 MCP 服务器开关 */
export async function toggleMcpForAgent(
  serverName: string,
  agentId: string,
  enabled: boolean
): Promise<void> {
  if (useDemo(true)) return;
  return invoke<void>('toggle_mcp_for_agent', { serverName, agentId, enabled });
}

// ─── 密钥管理 ────────────────────────────────────

export interface ApiKeyInfo {
  agentId: string;
  provider: string;
  maskedKey: string;
}

export interface BackupInfo {
  hash: string;
  timestamp: string;
  message: string;
}

const MOCK_KEYS: ApiKeyInfo[] = [
  { agentId: 'codex', provider: 'manual', maskedKey: 'sk-a***5678' },
  { agentId: 'opencode', provider: 'env', maskedKey: 'oc-***9999' },
];

const MOCK_BACKUPS: BackupInfo[] = [
  {
    hash: 'abc1234',
    timestamp: String(Date.now() * 1_000_000),
    message: 'inject codex',
  },
];

/** 保存 API Key（加密存入 vault） */
export async function saveApiKey(agentId: string, provider: string, key: string): Promise<void> {
  if (useDemo(true)) return;
  return invoke<void>('save_api_key', { agentId, provider, key });
}

/** 列出已存储的 API Key（脱敏显示） */
export async function listApiKeys(): Promise<ApiKeyInfo[]> {
  if (useDemo()) return MOCK_KEYS;
  return invoke<Array<{ agentId: string; provider: string; maskedKey: string }>>('list_api_keys');
}

/** 取明文 API Key（复制用） */
export async function getApiKey(agentId: string): Promise<string | null> {
  if (useDemo(true)) {
    const mock = MOCK_KEYS.find((k) => k.agentId === agentId);
    return mock ? `mock-${agentId}-key` : null;
  }
  return invoke<string | null>('get_api_key', { agentId });
}

/** 删除 API Key */
export async function deleteApiKey(agentId: string): Promise<boolean> {
  if (useDemo(true)) return true;
  return invoke<boolean>('delete_api_key', { agentId });
}

/** 从 V2 vault（~/.aisync-vault）导入密钥，返回导入条数 */
export async function importV2Keys(): Promise<number> {
  if (useDemo(true)) return 0;
  return invoke<number>('import_v2_keys');
}

// ─── 备份回滚 ────────────────────────────────────

/** 列出备份历史 */
export async function listBackups(): Promise<BackupInfo[]> {
  if (useDemo()) return MOCK_BACKUPS;
  return invoke<BackupInfo[]>('list_backups');
}

/** 按时间戳回滚：full=false 按操作回滚，full=true 整体恢复 */
export async function restoreBackup(timestamp: string, full: boolean): Promise<string[]> {
  if (useDemo(true)) return [];
  return invoke<string[]>('restore_backup', { timestamp, full });
}

// ─── 在线更新 ────────────────────────────────────

export interface UpdateSummary {
  version: string;
  released: string;
  files: string[];
  pubkeyFp: string;
}

/** 检查注册表在线更新（无更新返回 null） */
export async function checkUpdates(): Promise<UpdateSummary | null> {
  if (useDemo()) throw new Error('只读演示不检查真实更新，请使用桌面应用。');
  return invoke<UpdateSummary | null>('check_updates');
}

/** 应用更新（下载 → 校验 → 备份 → 原子写入） */
export async function applyUpdates(): Promise<string[]> {
  if (useDemo(true)) return [];
  return invoke<string[]>('apply_updates');
}

/** 应用版本（来自后端健康检查） */
export async function getAppVersion(): Promise<string> {
  if (useDemo()) return '演示数据（非本机版本）';
  const health = await invoke<string>('health_check');
  return health.replace(/^AgentDeck /, '').replace(/ OK$/, '');
}


export interface CollectionStatus {
  enabled: boolean;
  lastRunAt: number | null;
  lastError: string | null;
  nextRunAt: number | null;
}

export async function getMemoryCollectionStatus(): Promise<CollectionStatus> {
  if (useDemo()) return { enabled: false, lastRunAt: null, lastError: null, nextRunAt: null };
  return invoke<CollectionStatus>('memory_collection_status');
}

export async function setMemoryCollectionEnabled(enabled: boolean): Promise<void> {
  useDemo(true);
  return invoke<void>('set_memory_collection_enabled', { enabled });
}
