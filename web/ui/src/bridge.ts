import { exportTarget, exportIds, exportToken, validateExportSnapshot, validateExportPreview, validateExportReceipt } from './memoryExportContract';
import { requireDshNativePackage, dshNativeEvidence, validateDshNativeVerification, type DshNativePackage } from './dshNativeContract';
import { requireQoderRelease, type QoderRelease } from './qoderReleases';
import { validQoderDeploymentPath, validQoderDeploymentToken, validateQoderDeploymentPreview, validateQoderDeploymentOutcome } from "./qoderDeploymentContract";
import { validSessionRef, parseResidual } from './codexResidualContract';
import { validateAnimationsCapability, validateAnimationsObservation } from "./codexAnimationsContract";
import { validateAnimationsPreview, validateAnimationsRecovery, validateAnimationsMutation, validAnimationsToken } from "./codexAnimationsContract";
import { validateCodexConfigurationPlan, demoCodexConfigurationPlan } from './codexConfigurationContract';
import { validateDshRootPlan, validateDshRootVerification } from "./dshDistributionContract";
import { scopeIds, validateScopeReport, type ScopePaths } from './qoderScopeContract';
import { validateQoderWrite, type QoderWriteOperation } from './qoderContract';
import { requireQoderTarget, validateQoderPlan, validateQoderVerification, validateQoderCacheObservation } from './qoderContract';
import { qwenDemoSummary, validateQwenReport, validateQwenVimPreview, validateQwenMutation, validateQwenRecovery, validateQwenVimRestorePreview } from './qwenContract';
import type { QwenSummary } from './qwenContract';
/**
 * Tauri 命令调用桥接层。
 * 前端所有与 Rust 后端的通信都走这里，不从组件直接 invoke()。
 * 浏览器默认拒绝 IPC；仅 ?demo=1 显式展示只读示例。
 */

import { invoke } from '@tauri-apps/api/core';
import { dshDemoReport, dshLimits, validateDshReport } from './dshContract';
import type { DshInspectionResult } from './dshContract';
export type { DshInventoryReport, DshInspectionResult } from './dshContract';

export async function inspectQwenSettings(settingsPath: string, runtimeManifest: string): Promise<{ summary: QwenSummary; synthetic: boolean }> {
  const demo = useDemo();
  for (const path of [settingsPath, runtimeManifest]) {
    if (typeof path !== 'string' || !path.trim() || new TextEncoder().encode(path).length > 4096 || /[\x00-\x1f\x7f]/.test(path)) throw new Error('请选择有效且长度受限的明确路径。');
  }
  if (demo) return { summary: qwenDemoSummary(), synthetic: true };
  const summary = validateQwenReport(await invoke<unknown>('inspect_qwen_settings', { settingsPath, runtimeManifest }));
  return { summary, synthetic: false };
}

/** These operations are never simulated in browser/demo mode. */
function qwenOwnedPaths(settingsPath: string, runtimeManifest: string): void {
  useDemo(true);
  for (const path of [settingsPath, runtimeManifest]) {
    if (typeof path !== 'string' || !path.trim() || new TextEncoder().encode(path).length > 4096 || /[\x00-\x1f\x7f]/.test(path)) throw new Error('请选择有效且长度受限的明确路径。');
  }
}
export async function previewQwenVim(settingsPath: string, runtimeManifest: string, enabled: boolean) {
  qwenOwnedPaths(settingsPath, runtimeManifest);
  if (typeof enabled !== 'boolean') throw new Error('设置值不受支持。');
  return validateQwenVimPreview(await invoke<unknown>('preview_qwen_vim', { settingsPath, runtimeManifest, enabled }), enabled);
}
export async function applyQwenVim(settingsPath: string, runtimeManifest: string, enabled: boolean, token: string) {
  qwenOwnedPaths(settingsPath, runtimeManifest);
  if (typeof enabled !== 'boolean' || typeof token !== 'string' || !/^[a-f0-9]{64}$/.test(token)) throw new Error('请重新预览。');
  return validateQwenMutation(await invoke<unknown>('apply_qwen_vim', { settingsPath, runtimeManifest, enabled, token }));
}
export async function previewQwenVimUpdate(settingsPath: string, runtimeManifest: string, enabled: boolean) {
  qwenOwnedPaths(settingsPath, runtimeManifest);
  if (typeof enabled !== 'boolean') throw new Error('设置值不受支持。');
  return validateQwenVimPreview(await invoke<unknown>('preview_qwen_vim_update', { settingsPath, runtimeManifest, enabled }), enabled);
}
export async function updateQwenVim(settingsPath: string, runtimeManifest: string, enabled: boolean, token: string) {
  qwenOwnedPaths(settingsPath, runtimeManifest);
  if (typeof enabled !== 'boolean' || typeof token !== 'string' || !/^[a-f0-9]{64}$/.test(token)) throw new Error('请重新预览。');
  return validateQwenMutation(await invoke<unknown>('update_qwen_vim', { settingsPath, runtimeManifest, enabled, token }));
}
export async function inspectQwenVimRecovery(settingsPath: string, runtimeManifest: string) {
  qwenOwnedPaths(settingsPath, runtimeManifest);
  return validateQwenRecovery(await invoke<unknown>('inspect_qwen_vim_recovery', { settingsPath, runtimeManifest }));
}
export async function previewQwenVimRestore(settingsPath: string, runtimeManifest: string) {
  qwenOwnedPaths(settingsPath, runtimeManifest);
  return validateQwenVimRestorePreview(await invoke<unknown>('preview_qwen_vim_restore', { settingsPath, runtimeManifest }));
}
export async function restoreQwenVim(settingsPath: string, runtimeManifest: string, token: string) {
  qwenOwnedPaths(settingsPath, runtimeManifest);
  if (typeof token !== 'string' || !/^[a-f0-9]{64}$/.test(token)) throw new Error('请重新预览。');
  return validateQwenMutation(await invoke<unknown>('restore_qwen_vim', { settingsPath, runtimeManifest, token }));
}

/** Native, read-only Qoder distribution checks. Never stage/unpack/install. */
export async function qoderDistributionPlan(target: string, release: QoderRelease = '1.1.85') {
  requireQoderRelease(release);
  useDemo(true); requireQoderTarget(target);
  return validateQoderPlan(await invoke<unknown>('qoder_distribution_plan', { target, release }), target, release);
}
export async function verifyQoderPackage(target: string, archivePath: string, release: QoderRelease = '1.1.85') {
  requireQoderRelease(release);
  useDemo(true); requireQoderTarget(target);
  if (typeof archivePath !== 'string' || !archivePath.trim() || new TextEncoder().encode(archivePath).length > 4096 || /[\x00-\x1f\x7f]/.test(archivePath)) throw new Error('请选择长度受限的有效包路径。');
  return validateQoderVerification(await invoke<unknown>('verify_qoder_package', { target, archivePath, release }), target, release);
}

export async function inspectQoderCache(cachePath: string, release: QoderRelease = '1.1.85') {
  requireQoderRelease(release);
  useDemo(true);
  if (typeof cachePath !== 'string' || !cachePath.trim() || new TextEncoder().encode(cachePath).length > 4096 || /[\x00-\x1f\x7f]/.test(cachePath)) throw new Error('请选择长度受限的有效缓存路径。');
  return validateQoderCacheObservation(await invoke<unknown>('inspect_qoder_cache', { cachePath, release }), release);
}

/** Independent of deployment: no registry read, discovery, or lifecycle command. */
export async function inspectDshProfile(profileDir: string, runtimeManifest: string | null): Promise<DshInspectionResult> {
  const demo = useDemo();
  const validInput = (value: unknown): value is string => typeof value === 'string'
    && value.trim().length > 0 && new TextEncoder().encode(value).length <= dshLimits.max_path_bytes
    && !/[\x00-\x1f\x7f]/.test(value);
  if (!validInput(profileDir) || !(runtimeManifest === null || validInput(runtimeManifest))) {
    throw new Error('请提供有效且长度受限的明确路径。');
  }
  if (demo) return { report: dshDemoReport(runtimeManifest !== null), synthetic: true };
  const report = validateDshReport(await invoke<unknown>('inspect_dsh_profile', { profileDir, runtimeManifest }));
  if ((report.runtime_manifest === null) !== (runtimeManifest === null)) {
    throw new Error('DSH 盘点响应与所选文件不一致，请重新盘点。');
  }
  return { report, synthetic: false };
}

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

export type InstallationStatus = 'installed' | 'not_detected' | 'error' | 'unsupported';

/** Evidence about the current files, never an installation/ownership history. */
export type InstallationSource =
  | { kind: 'unknown'; reason: string }
  | { kind: 'npm'; package: string; packageRoot: string; globalRoot: string; prefix: string; managerPath: string };

export interface InstallationInstance {
  path: string;
  resolvedPath: string | null;
  entrypointPath: string | null;
  version: string | null;
  source: InstallationSource;
  active: boolean;
}

export interface InstallationDetails {
  probe: { status: InstallationStatus; message: string | null };
  instances: InstallationInstance[];
  ambiguous: boolean;
  message: string | null;
}

export interface AgentInfo {
  id: string;
  name: string;
  installed: boolean;
  // Optional for legacy backends; missing values must never authorize a mutation.
  installationStatus?: InstallationStatus;
  installationMessage?: string | null;
  installSource?: 'unknown' | 'npm';
  installationDetails?: InstallationDetails;
  uninstallHelp?: string | null;
  canConfigure?: boolean;
  canInstall?: boolean;
  canVerifyInstall?: boolean;
  canUninstall?: boolean;
  installHelp?: string | null;
  homepage?: string | null;
  version?: string | null;
  authVerified: boolean;
  source: 'open' | 'closed';
  uiType: string;
  stars: number;
  popular?: boolean;
  description: string;
  icon: string;
  category: string;
}

/** Shared UI/store write gate. Legacy DTOs have no details; present evidence must agree. */
export function installationEvidenceProblem(agent: AgentInfo): 'invalid' | 'ambiguous' | null {
  const details: unknown = agent.installationDetails;
  if (details === undefined) return null;
  const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
  if (!isRecord(details) || !isRecord(details.probe) || typeof details.probe.status !== 'string'
    || !['installed', 'not_detected', 'error', 'unsupported'].includes(details.probe.status)
    || details.probe.status !== agent.installationStatus
    || agent.installed !== (details.probe.status === 'installed')
    || typeof details.ambiguous !== 'boolean' || !Array.isArray(details.instances)
    || details.instances.some((instance) => !isRecord(instance) || typeof instance.path !== 'string'
      || !instance.path.trim() || typeof instance.active !== 'boolean')) return 'invalid';
  const activeCount = details.instances.filter((instance) => instance.active).length;
  return details.ambiguous || (details.instances.length > 0 && activeCount !== 1) ? 'ambiguous' : null;
}

const MOCK_AGENTS: AgentInfo[] = [
  {
    id: 'opencode',
    name: 'OpenCode',
    installed: true,
    installationStatus: 'installed',
    installationMessage: null,
    installSource: 'unknown',
    canInstall: true,
    canVerifyInstall: true,
    canUninstall: false,
    canConfigure: true,
    uninstallHelp: '示例数据：安装来源未验证，请使用原安装方式卸载。',
    version: '0.0.0-demo',
    authVerified: true,
    source: 'open',
    uiType: 'all',
    stars: 0,
    popular: false,
    description: '合成示例：展示 Agent 卡片与配置入口，不代表本机安装或当前产品能力。',
    icon: 'opencode',
    category: 'ai-coding',
  },
  {
    id: 'claude-code',
    name: 'Claude Code',
    installed: false,
    installationStatus: 'not_detected',
    installationMessage: '示例数据：PATH 中未检测到命令。',
    installSource: 'unknown',
    canInstall: true,
    canVerifyInstall: true,
    canUninstall: false,
    canConfigure: true,
    uninstallHelp: '示例数据：安装来源未验证，请使用原安装方式卸载。',
    authVerified: false,
    source: 'closed',
    uiType: 'all',
    stars: 0,
    popular: false,
    description: '合成示例：展示 PATH 未检测到的界面，不代表本机安装或当前产品能力。',
    icon: 'claude-code',
    category: 'ai-coding',
  },
  {
    id: 'codex',
    name: 'OpenAI Codex',
    installed: true,
    installationStatus: 'installed',
    installationMessage: null,
    installSource: 'unknown',
    canInstall: true,
    canVerifyInstall: true,
    canUninstall: false,
    canConfigure: true,
    uninstallHelp: '示例数据：安装来源未验证，请使用原安装方式卸载。',
    version: '0.0.0-demo',
    authVerified: true,
    source: 'open',
    uiType: 'cli+desktop',
    stars: 0,
    popular: false,
    description: '合成示例：展示 CLI 与桌面类型，不代表本机安装或当前产品能力。',
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

export interface AgentInstallPreview {
  sessionRef?: string;
  schemaVersion: 1;
  agentId: string;
  package: string;
  executable: string;
  managerPath: string;
  nodePath: string;
  installerNodeVersion: string;
  installerNodeMinimum: string;
  prefix: string;
  globalRoot: string;
  packageRoot: string;
  launcherDirectory: string;
  scriptPolicy: 'disabled' | 'npm_configured';
  versionPolicy: 'npm_configured' | 'exact_root';
  packageVersion: string | null;
  revision: string;
}

const installPackages: Record<string, readonly [string, string, string]> = {
  codex: ['@openai/codex', 'codex', '18.0.0'],
  opencode: ['opencode-ai', 'opencode', '18.0.0'],
  'gemini-cli': ['@google/gemini-cli', 'gemini', '20.0.0'],
  pi: ['@earendil-works/pi-coding-agent', 'pi', '22.19.0'],
};
/** Only the frozen installer's stable Node version, not package engine compatibility. */
function validInstallerNodeVersion(value: unknown, minimum: string): value is string {
  if (typeof value !== 'string' || value.length > 33 || value.trim() !== value || !/^v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$/.test(value)) return false;
  const measured = value.slice(1).split('.').map(Number);
  if (measured.some(part => !Number.isInteger(part) || part > 0xffffffff)) return false;
  const required = minimum.split('.').map(Number);
  for (let i = 0; i < required.length; i++) {
    if (measured[i] !== required[i]) return measured[i] > required[i];
  }
  return true;
}
function validExactPackageVersion(value: unknown): value is string {
  return typeof value === 'string' && value.length <= 32
    && /^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$/.test(value)
    && value.split('.').every(part => Number(part) <= 0xffffffff);
}
export function validInstallRevision(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
}
function safeInstallPath(value: unknown): value is string {
  return typeof value === 'string' && new TextEncoder().encode(value).length <= 32768 && value.length > 0 && value.trim() === value
    && /^(?:\/|(?:\\\\\?\\)?[A-Za-z]:[\\/]|\\\\[^\\]+\\[^\\]+)/.test(value)
    && !/[\u0000-\u001f\u007f-\u009f\u200b-\u200f\u2028-\u202e\u2060-\u206f\ufeff]/.test(value)
    && !value.split(/[\\/]/).some(part => part === '..' || part === '.');
}
/** Compare the promised layout lexically; never rewrite the displayed evidence. */
function consistentInstallLayout(raw: Record<string, unknown>, packageName: string): boolean {
  const prefix = raw.prefix as string;
  const windows = /^(?:[A-Za-z]:[\\/]|\\\\)/.test(prefix);
  // Rust Path::join can retain forward slashes inside a scoped npm package.
  // Only Windows separators are equivalent here; POSIX backslashes are literal.
  const comparable = (value: string) => (windows ? value.replace(/\\/g, '/') : value).replace(/\/+$/, '');
  const base = comparable(prefix);
  const root = `${base}/${windows ? 'node_modules' : 'lib/node_modules'}`;
  return comparable(raw.globalRoot as string) === root
    && comparable(raw.packageRoot as string) === `${root}/${packageName}`
    && comparable(raw.launcherDirectory as string) === (windows ? base : `${base}/bin`);
}

/** Project an untrusted response, never forwarding backend prose or extra fields. */
export function parseInstallPreview(value: unknown, id: string): AgentInstallPreview {
  const fail = () => { throw new Error('安装预览无效，请刷新后重新检查。'); };
  const expected = Object.prototype.hasOwnProperty.call(installPackages, id) ? installPackages[id] : undefined;
  if (!expected || !value || typeof value !== 'object' || Array.isArray(value)) return fail();
  const raw = value as Record<string, unknown>;
  const scriptPolicy = (id === 'pi' || id === 'codex') ? 'disabled' : 'npm_configured';
  if (raw.schemaVersion !== 1 || raw.agentId !== id || raw.package !== expected[0]
    || raw.executable !== expected[1] || raw.scriptPolicy !== scriptPolicy
    || raw.versionPolicy !== (id === 'codex' ? 'exact_root' : 'npm_configured')
    || (id === 'codex' ? !validExactPackageVersion(raw.packageVersion) : raw.packageVersion !== null)
    || !validInstallRevision(raw.revision)
    || raw.installerNodeMinimum !== expected[2] || !validInstallerNodeVersion(raw.installerNodeVersion, expected[2])) return fail();
  const paths = ['managerPath', 'nodePath', 'prefix', 'globalRoot', 'packageRoot', 'launcherDirectory'] as const;
  for (const key of paths) if (!safeInstallPath(raw[key])) return fail();
  if (!consistentInstallLayout(raw, expected[0])) return fail();
  return {
    schemaVersion: 1, agentId: id, package: expected[0], executable: expected[1], scriptPolicy,
    ...(id === 'codex' && validSessionRef(raw.sessionRef) ? { sessionRef: raw.sessionRef } : {}),
    versionPolicy: id === 'codex' ? 'exact_root' : 'npm_configured',
    packageVersion: id === 'codex' ? raw.packageVersion as string : null, revision: raw.revision,
    managerPath: raw.managerPath as string, nodePath: raw.nodePath as string,
    installerNodeVersion: raw.installerNodeVersion, installerNodeMinimum: expected[2],
    prefix: raw.prefix as string, globalRoot: raw.globalRoot as string,
    packageRoot: raw.packageRoot as string, launcherDirectory: raw.launcherDirectory as string,
  };
}

export async function previewAgentInstall(id: string): Promise<AgentInstallPreview> {
  useDemo(true);
  if (!Object.prototype.hasOwnProperty.call(installPackages, id)) throw new Error('此 Agent 不支持安装预览。');
  try {
    return parseInstallPreview(await invoke<unknown>('preview_agent_install', { id }), id);
  } catch {
    throw new Error('无法取得有效安装预览，请刷新后重新检查。预览可能读取 npm 配置或更新缓存，但不会安装软件或应用配置。');
  }
}

/** Only dispatch an explicitly reviewed revision. No unconfirmed fallback. */
export async function installAgent(id: string, revision: string, sessionRef?: string): Promise<InstallOutcome> {
  useDemo(true);
  if (!Object.prototype.hasOwnProperty.call(installPackages, id) || !validInstallRevision(revision)) throw new Error('请先检查有效安装预览。');
  return invoke<InstallOutcome>('install_agent', { id, revision, ...(validSessionRef(sessionRef) ? { sessionRef } : {}) });
}

/** 单独应用默认 profile 支持的配置（调用方必须先明确确认） */
export async function previewCodexConfiguration() {
  if (useDemo()) return { plan: demoCodexConfigurationPlan(), synthetic: true };
  return { plan: validateCodexConfigurationPlan(await invoke<unknown>('preview_codex_configuration')), synthetic: false };
}
export async function configureAgent(id: string): Promise<InstallOutcome> {
  useDemo(true);
  if (id === 'codex') throw new Error('Codex 当前仅提供只读配置计划，不支持应用。');
  return invoke<InstallOutcome>('configure_agent', { id });
}

/** Automatic uninstall is unavailable, including for currently matched npm files. */
export async function uninstallAgent(_id: string): Promise<void> {
  useDemo(true);
  throw new Error('自动卸载功能尚未提供；来源目录匹配不代表安装历史或管理权限。请使用原安装方式或官方卸载说明。');
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

/** A content-free preview. revision binds confirmation to the prepared change. */
export interface McpTogglePreview {
  agentId: string;
  serverName: string;
  enabled: boolean;
  configPath: string;
  format: string;
  fileExists: boolean;
  changed: boolean;
  revision: string;
}

export async function previewMcpToggle(serverName: string, agentId: string, enabled: boolean): Promise<McpTogglePreview> {
  useDemo(true); // Never fabricate a writable preview in a browser demo.
  return invoke<McpTogglePreview>('preview_mcp_toggle', { serverName, agentId, enabled });
}

export async function applyMcpTogglePreview(serverName: string, agentId: string, enabled: boolean, revision: string): Promise<void> {
  useDemo(true);
  return invoke<void>('apply_mcp_toggle_preview', { serverName, agentId, enabled, revision });
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

export async function writeQoderCache(operation: QoderWriteOperation, source: string, destination: string, confirmed: boolean, release: QoderRelease = '1.1.85') {
  requireQoderRelease(release);
  useDemo(true);
  if (!confirmed || !['stage','unpack'].includes(operation) || [source,destination].some(p => typeof p !== 'string' || !p.startsWith('/') || !p.trim() || new TextEncoder().encode(p).length > 4096 || /[\x00-\x1f\x7f]/.test(p))) throw new Error('QODER_WRITE_INVALID_REQUEST');
  return validateQoderWrite(await invoke<unknown>('write_qoder_cache', { operation, target: 'linux-amd64-baseline', source, destination, confirmed: true, release }), operation, release);
}

export async function inspectQoderScopes(paths: ScopePaths) {
  useDemo(true);
  const captured = { ...paths };
  if (!scopeIds.some(id=>!!captured[id]) || scopeIds.some(id=>typeof captured[id]!=='string' || new TextEncoder().encode(captured[id]).length>4096 || /[\x00-\x1f\x7f]/.test(captured[id]))) throw new Error('QODER_SCOPE_INVALID_SELECTION');
  return validateScopeReport(await invoke<unknown>('inspect_qoder_scopes', {userSettings:captured.user_settings||null,projectSettings:captured.project_settings||null,localSettings:captured.local_settings||null,projectMcp:captured.project_mcp||null}),captured);
}

export async function dshDistributionPlan() {
  useDemo(true);
  return validateDshRootPlan(await invoke<unknown>('dsh_distribution_plan'));
}
export async function verifyDshPackage(archivePath: string) {
  useDemo(true);
  if (typeof archivePath!=='string'||!archivePath.trim()||new TextEncoder().encode(archivePath).length>4096||/[\x00-\x1f\x7f]/.test(archivePath)) throw new Error('DSH_PACKAGE_INVALID_PATH');
  if (!(/^(?:\/(?!\/)|[a-zA-Z]:[\\/])/.test(archivePath)) || archivePath.split(/[\\/]/).slice(-1)[0]!=='dsh-0.2.2-rc.1.tgz' || archivePath.split(/[\\/]/).includes('..')) throw new Error('DSH_PACKAGE_INVALID_PATH');
  return validateDshRootVerification(await invoke<unknown>('verify_dsh_package',{archivePath}));
}

// Single reviewed display preference, never simulated or routed to bulk configuration.
export async function previewCodexAnimations(enabled: boolean) {
  useDemo(true);
  if (typeof enabled !== 'boolean') throw new Error('设置值不受支持。');
  return validateAnimationsPreview(await invoke<unknown>('preview_codex_animations', { enabled }), enabled);
}
export async function applyCodexAnimations(enabled: boolean, token: string) {
  useDemo(true);
  if (typeof enabled !== 'boolean' || !validAnimationsToken(token)) throw new Error('请重新预览。');
  return validateAnimationsMutation(await invoke<unknown>('apply_codex_animations', { enabled, token }));
}
export async function inspectCodexAnimationsRecovery() {
  useDemo(true);
  return validateAnimationsRecovery(await invoke<unknown>('inspect_codex_animations_recovery'));
}
export async function restoreCodexAnimations(token: string) {
  useDemo(true);
  if (!validAnimationsToken(token)) throw new Error('请重新检查恢复状态。');
  return validateAnimationsMutation(await invoke<unknown>('restore_codex_animations', { token }));
}
export async function getCodexAnimationsCapability() {
  useDemo(true);
  return validateAnimationsCapability(await invoke<unknown>('get_codex_animations_capability'));
}
export async function observeCodexAnimations() {
  useDemo(true);
  return validateAnimationsObservation(await invoke<unknown>('observe_codex_animations'));
}

export async function inspectCodexInstallResidual(sessionRef: string) {
  useDemo(true);
  if (!validSessionRef(sessionRef)) throw new Error('本会话只读检查不可用。');
  try { return parseResidual(await invoke<unknown>('inspect_codex_install_residual', { sessionRef })); }
  catch { throw new Error('本会话只读检查不可用或路径发生变化。未清理或执行程序。'); }
}

/** Explicit private executable deployment; no demo, download, activation or execution. */
export async function previewQoderDeployment(source: string, destination: string, release: QoderRelease = '1.1.85') {
  requireQoderRelease(release);
  useDemo(true);
  if (!validQoderDeploymentPath(source) || !validQoderDeploymentPath(destination)) throw new Error('请选择有效绝对路径。');
  return validateQoderDeploymentPreview(await invoke<unknown>('preview_qoder_deployment', { source, destination, release }), release);
}
export async function deployQoderPrivate(source: string, destination: string, token: string, confirmed: boolean, release: QoderRelease = '1.1.85') {
  requireQoderRelease(release);
  useDemo(true);
  if (!confirmed || !validQoderDeploymentPath(source) || !validQoderDeploymentPath(destination) || !validQoderDeploymentToken(token)) throw new Error('请重新预览并确认。');
  return validateQoderDeploymentOutcome(await invoke<unknown>('deploy_qoder_private', { source, destination, token, confirmed: true, release }), release);
}

// Static verification only. No demo success, package resolution or execution.
export async function verifyDshNativePackage(packageId: DshNativePackage, archivePath: string) {
  useDemo(true);
  requireDshNativePackage(packageId);
  if (typeof archivePath !== 'string' || !archivePath.trim() ||
      new TextEncoder().encode(archivePath).length > 4096 || /[\x00-\x1f\x7f-\x9f]/.test(archivePath) ||
      !/^(?:\/(?!\/)|[a-zA-Z]:[\\/])/.test(archivePath) ||
      archivePath.split(/[\\/]/).includes('..') ||
      archivePath.split(/[\\/]/).slice(-1)[0] !== dshNativeEvidence[packageId].archive_name) {
    throw new Error('DSH_NATIVE_INVALID_PATH');
  }
  return validateDshNativeVerification(await invoke<unknown>('verify_dsh_native_package', {packageId, archivePath}), packageId);
}

/** Export editor never simulates reads or writes outside the native desktop. */
function requireMemoryExportNative(agentId: string) {
  exportTarget(agentId);
  if (runtimeMode() !== 'native') throw new Error('记忆导出编辑需要桌面后端；演示模式不读写文件。');
}
export async function readMemoryExport(agentId: string) {
  requireMemoryExportNative(agentId); exportTarget(agentId);
  return validateExportSnapshot(await invoke<unknown>('read_memory_export', { agentId }), agentId);
}
export async function previewMemoryExport(agentId: string, expectedStateToken: string, memoryIds: string[]) {
  requireMemoryExportNative(agentId); exportTarget(agentId); exportToken(expectedStateToken); exportIds(memoryIds);
  return validateExportPreview(await invoke<unknown>('preview_memory_export', { agentId, expectedStateToken, memoryIds }), agentId, expectedStateToken, memoryIds);
}
export async function replaceMemoryExport(agentId: string, previewToken: string, memoryIds: string[]) {
  requireMemoryExportNative(agentId); exportTarget(agentId); exportToken(previewToken); exportIds(memoryIds);
  return validateExportReceipt(await invoke<unknown>('replace_memory_export', { agentId, previewToken, memoryIds }), agentId, memoryIds);
}
