import { afterEach, describe, expect, it, vi } from 'vitest';
import * as bridge from './bridge';
import nativeAgentContract from '../../../tests/fixtures/agent-info-installed.json';
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
    expect(await bridge.getAgents()).not.toHaveLength(0);
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
    expect(agents[0].version).toBeUndefined();
    expect(invoke).toHaveBeenCalledExactlyOnceWith('get_agents');
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
