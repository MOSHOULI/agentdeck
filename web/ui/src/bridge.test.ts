import { afterEach, describe, expect, it, vi } from 'vitest';
import * as bridge from './bridge';
import nativeAgentContract from '../../../tests/fixtures/agent-info-installed.json';
import nativeNpmContract from '../../../tests/fixtures/agent-info-npm.json';
import { npmMatchedAgent, ambiguousAgent, installedAgent } from './test-fixtures/agents';
import { invoke } from '@tauri-apps/api/core';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => {
  window.history.replaceState({}, '', '/');
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.clearAllMocks();
});
describe('runtime boundary', () => {
  it('rejects browser reads and writes without an explicit demo', async () => {
    expect(bridge.runtimeMode()).toBe('unavailable');
    await expect(bridge.getAgents()).rejects.toThrow('未连接桌面后端');
    await expect(bridge.installAgent('codex')).rejects.toThrow('未连接桌面后端');
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
    await expect(bridge.installAgent('codex')).rejects.toThrow('只读演示');
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
    await bridge.installAgent('codex');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('install_agent', { id: 'codex' });
    vi.mocked(invoke).mockClear();
    await bridge.configureAgent('codex');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('configure_agent', { id: 'codex' });
  });
  it('native IPC always takes precedence over demo and propagates failures', async () => {
    window.history.replaceState({}, '', '/?demo=1');
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockRejectedValue(new Error('backend unavailable'));
    expect(bridge.runtimeMode()).toBe('native');
    await expect(bridge.installAgent('codex')).rejects.toThrow('backend unavailable');
    expect(invoke).toHaveBeenCalledWith('install_agent', { id: 'codex' });
  });
});
