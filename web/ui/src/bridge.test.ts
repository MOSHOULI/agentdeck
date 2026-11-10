import { afterEach, describe, expect, it, vi } from 'vitest';
import * as bridge from './bridge';
import nativeAgentContract from '../../../tests/fixtures/agent-info-installed.json';
import nativeNpmContract from '../../../tests/fixtures/agent-info-npm.json';
import { npmMatchedAgent, ambiguousAgent, installedAgent } from './test-fixtures/agents';
import { invoke } from '@tauri-apps/api/core';
import { dshDemoReport } from './dshContract';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => {
  window.history.replaceState({}, '', '/');
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe('DSH passive inspection boundary', () => {
  it('rejects browser inspection without issuing IPC', async () => {
    await expect(bridge.inspectDshProfile('/explicit/profile', null)).rejects.toThrow('未连接桌面后端');
    expect(invoke).not.toHaveBeenCalled();
  });
  it('returns visibly synthetic declarations in demo without IPC or path reflection', async () => {
    window.history.replaceState({}, '', '/?demo=1');
    const result = await bridge.inspectDshProfile('/private/DO_NOT_ECHO', null);
    expect(result.synthetic).toBe(true);
    expect(result.report.runtime_manifest).toBeNull();
    expect(JSON.stringify(result)).not.toContain('DO_NOT_ECHO');
    expect(invoke).not.toHaveBeenCalled();
  });
  it('uses only the explicit read-only command and preserves unknown/false evidence', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    const report = dshDemoReport(true);
    vi.mocked(invoke).mockResolvedValue(report);
    expect(await bridge.inspectDshProfile('/selected/profile', '/selected/runtime/package.json')).toEqual({ report, synthetic: false });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_dsh_profile', { profileDir: '/selected/profile', runtimeManifest: '/selected/runtime/package.json' });
  });
  it.each(['', '\u0000bad', 'x'.repeat(4097)])('rejects invalid or unbounded input before IPC', async (input) => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    await expect(bridge.inspectDshProfile(input, null)).rejects.toThrow('有效且长度受限');
    await expect(bridge.inspectDshProfile('/selected/profile', input)).rejects.toThrow('有效且长度受限');
    expect(invoke).not.toHaveBeenCalled();
  });
  it.each([
    (r: Record<string, unknown>) => { r.runtime_status = 'running'; },
    (r: Record<string, unknown>) => { r.authentication = 'authenticated'; },
    (r: Record<string, unknown>) => { r.version_recognition = 'reviewed_manifest_version'; },
    (r: Record<string, unknown>) => { (r.mutation_capabilities as Record<string, unknown>).manage_plugins = true; },
    (r: Record<string, unknown>) => { (r.mutation_capabilities as Record<string, unknown>).future_write = true; },
    (r: Record<string, unknown>) => { r.profile_manifest = { status: 'read', issue: null, metadata: null }; },
    (r: Record<string, unknown>) => { r.profile_manifest = { status: 'missing', issue: 'missing', metadata: { name: 'partial' } }; },
    (r: Record<string, unknown>) => { r.limitations = ['secret\u001b[0m']; },
    (r: Record<string, unknown>) => { (r.limits as Record<string, unknown>).max_files = 100; },
    (r: Record<string, unknown>) => { r.profile_manifest = { status: 'corrupt', issue: 'missing', metadata: null }; },
  ])('rejects incompatible responses without presenting partial success', async (mutate) => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    const report = JSON.parse(JSON.stringify(dshDemoReport(false))) as Record<string, unknown>;
    mutate(report);
    vi.mocked(invoke).mockResolvedValue(report);
    await expect(bridge.inspectDshProfile('/selected/profile', null)).rejects.toThrow('响应格式不兼容');
  });
  it('rejects an unsolicited runtime manifest and malformed dependency states', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockResolvedValue(dshDemoReport(true));
    await expect(bridge.inspectDshProfile('/selected/profile', null)).rejects.toThrow('所选文件不一致');
    const report = dshDemoReport(false);
    Object.assign(report.profile_manifest.metadata!.declared_dependencies[0], { installed: 'installed' });
    vi.mocked(invoke).mockResolvedValue(report);
    await expect(bridge.inspectDshProfile('/selected/profile', null)).rejects.toThrow('响应格式不兼容');
  });
  it('rejects unsafe runtime metadata rather than reflecting arbitrary strings', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    for (const field of ['reported_version', 'declared_node_engine'] as const) {
      const report = dshDemoReport(true);
      report.runtime_manifest!.metadata![field] = 'https://private.invalid/TOKEN';
      vi.mocked(invoke).mockResolvedValue(report);
      await expect(bridge.inspectDshProfile('/selected/profile', '/runtime/package.json')).rejects.toThrow('响应格式不兼容');
    }
  });
});
describe('runtime boundary', () => {
  it('rejects browser reads and writes without an explicit demo', async () => {
    expect(bridge.runtimeMode()).toBe('unavailable');
    await expect(bridge.getAgents()).rejects.toThrow('未连接桌面后端');
    await expect(bridge.installAgent('codex', 'a'.repeat(64))).rejects.toThrow('未连接桌面后端');
    await expect(bridge.configureAgent('codex')).rejects.toThrow('未连接桌面后端');
    expect(invoke).not.toHaveBeenCalled();
  });
  it('allows only sample reads in explicit demo mode', async () => {
    window.history.replaceState({}, '', '/?demo=1');
    expect(bridge.runtimeMode()).toBe('demo');
    const agents = await bridge.getAgents();
    expect(agents).not.toHaveLength(0);
    expect(agents.every((agent) => agent.popular === false && agent.stars === 0)).toBe(true);
    expect(agents.every((agent) => agent.description.startsWith('合成示例：'))).toBe(true);
    expect(agents.every((agent) => !agent.version || agent.version === '0.0.0-demo')).toBe(true);
    await expect(bridge.installAgent('codex', 'a'.repeat(64))).rejects.toThrow('只读演示');
    await expect(bridge.configureAgent('codex')).rejects.toThrow('只读演示');
    await expect(bridge.saveApiKey('codex', 'test', 'not-a-secret')).rejects.toThrow('只读演示');
    await expect(bridge.getApiKey('codex')).rejects.toThrow('只读演示');
    await expect(bridge.collectMemory('codex')).rejects.toThrow('只读演示');
    expect(invoke).not.toHaveBeenCalled();
  });
  it('preserves the shared native DTO contract without inferring source, version or uninstall permission', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockResolvedValue([nativeAgentContract]);
    const agents = await bridge.getAgents();
    expect(agents).toEqual([nativeAgentContract]);
    expect(agents[0]).toMatchObject({ installationStatus: 'installed', installSource: 'unknown', canUninstall: false, canConfigure: true });
    expect(agents[0].version).toBeNull();
    expect(agents[0].installationDetails).toMatchObject({ probe: { status: 'installed' }, instances: [], ambiguous: false });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('get_agents');
  });
  it.each([nativeNpmContract, npmMatchedAgent, ambiguousAgent])('preserves instance/source observations without elevating them to management permission (%s)', async (agent) => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockResolvedValue([agent]);
    expect(await bridge.getAgents()).toEqual([agent]);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('get_agents');
  });
  it('does not invent details or provenance for an older DTO', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    const legacy = { ...installedAgent, installationDetails: undefined, installSource: undefined, version: undefined };
    vi.mocked(invoke).mockResolvedValue([legacy]);
    const [agent] = await bridge.getAgents();
    expect(agent.installationDetails).toBeUndefined();
    expect(agent.installSource).toBeUndefined();
    expect(agent.version).toBeUndefined();
  });
  it.each(['native', 'demo', 'unavailable'] as const)('blocks every direct automatic uninstall request in %s mode', async (mode) => {
    if (mode === 'native') Object.assign(window, { __TAURI_INTERNALS__: {} });
    if (mode === 'demo') window.history.replaceState({}, '', '/?demo=1');
    await expect(bridge.uninstallAgent('synthetic-agent')).rejects.toThrow(mode === 'native' ? '自动卸载功能尚未提供' : mode === 'demo' ? '只读演示' : '未连接桌面后端');
    expect(invoke).not.toHaveBeenCalled();
  });
  it('uses distinct native IPC commands for software installation and explicit configuration', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockResolvedValue({ warnings: [] });
    await bridge.installAgent('codex', 'a'.repeat(64));
    expect(invoke).toHaveBeenCalledExactlyOnceWith('install_agent', { id: 'codex', revision: 'a'.repeat(64) });
    vi.mocked(invoke).mockClear();
    await bridge.configureAgent('opencode');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('configure_agent', { id: 'opencode' });
  });
  it('native IPC always takes precedence over demo and propagates failures', async () => {
    window.history.replaceState({}, '', '/?demo=1');
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockRejectedValue(new Error('backend unavailable'));
    expect(bridge.runtimeMode()).toBe('native');
    await expect(bridge.installAgent('codex', 'a'.repeat(64))).rejects.toThrow('backend unavailable');
    expect(invoke).toHaveBeenCalledWith('install_agent', { id: 'codex', revision: 'a'.repeat(64) });
  });
});

describe('MCP preview boundary', () => {
  it.each(['/', '/?demo=1'])('does not fabricate previews or apply in browser mode %s', async (url) => {
    window.history.replaceState({}, '', url);
    await expect(bridge.previewMcpToggle('ragflow', 'codex', true)).rejects.toThrow();
    await expect(bridge.applyMcpTogglePreview('ragflow', 'codex', true, 'revision')).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
  it('uses separate preview and revision-bound apply commands', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    const preview = { agentId: 'codex', serverName: 'ragflow', enabled: true, configPath: '/synthetic/config.toml', format: 'toml_section', fileExists: true, changed: true, revision: 'revision-1' };
    vi.mocked(invoke).mockResolvedValueOnce(preview);
    expect(await bridge.previewMcpToggle('ragflow', 'codex', true)).toEqual(preview);
    expect(invoke).toHaveBeenNthCalledWith(1, 'preview_mcp_toggle', { serverName: 'ragflow', agentId: 'codex', enabled: true });
    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await bridge.applyMcpTogglePreview('ragflow', 'codex', true, preview.revision);
    expect(invoke).toHaveBeenNthCalledWith(2, 'apply_mcp_toggle_preview', { serverName: 'ragflow', agentId: 'codex', enabled: true, revision: 'revision-1' });
  });
});

describe('Qwen read-only bridge', () => {
  it('rejects browser calls and uses labeled demo without IPC', async () => {
    await expect(bridge.inspectQwenSettings('/settings.json', '/package.json')).rejects.toThrow('未连接桌面后端');
    window.history.replaceState({}, '', '/?demo=1');
    const result = await bridge.inspectQwenSettings('/PRIVATE/settings.json', '/package.json');
    expect(result.synthetic).toBe(true);
    expect(JSON.stringify(result)).not.toContain('PRIVATE');
    expect(invoke).not.toHaveBeenCalled();
  });
  it('dispatches only the explicit read command and projects fixed metadata', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockResolvedValue({
      mode: 'explicit_settings_summary', authentication: 'unknown', effective_configuration: 'unknown',
      version_recognition: 'unknown', runtime_manifest: { status: 'unsupported' },
      settings: { status: 'not_read', sections: null },
      mutation_capabilities: { configure: false, authenticate: false, install: false, restore: false, execute: false },
    });
    const result = await bridge.inspectQwenSettings('/selected/settings.json', '/runtime/package.json');
    expect(result.synthetic).toBe(false);
    expect(result.summary.reviewed).toBe(false);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_qwen_settings', { settingsPath: '/selected/settings.json', runtimeManifest: '/runtime/package.json' });
  });
  it.each(['', '\u0000x', 'x'.repeat(4097)])('rejects invalid paths before dispatch', async value => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    await expect(bridge.inspectQwenSettings(value, '/package.json')).rejects.toThrow();
    await expect(bridge.inspectQwenSettings('/settings.json', value)).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
});
