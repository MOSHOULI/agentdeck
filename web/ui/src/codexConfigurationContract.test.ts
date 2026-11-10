import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { demoCodexConfigurationPlan, validateCodexConfigurationPlan } from './codexConfigurationContract';
import { previewCodexConfiguration, configureAgent } from './bridge';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { Object.assign(window, { __TAURI_INTERNALS__: {} }); });
afterEach(() => { delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', '/'); vi.resetAllMocks(); });
describe('Codex plan contract', () => {
  it('projects fixed metadata and strips values, limits and arbitrary keys', async () => {
    const raw = { ...demoCodexConfigurationPlan(), limitations: 'SECRET', token: 'SECRET', targets: demoCodexConfigurationPlan().targets.map(t => ({ ...t, value: 'SECRET' })) };
    vi.mocked(invoke).mockResolvedValue(raw);
    expect(await previewCodexConfiguration()).toEqual({ plan: demoCodexConfigurationPlan(), synthetic: false });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('preview_codex_configuration');
    expect(JSON.stringify(validateCodexConfigurationPlan(raw))).not.toContain('SECRET');
  });
  it.each([null, {}, { ...demoCodexConfigurationPlan(), applySupported: true }, { ...demoCodexConfigurationPlan(), agentId: 'other' }, { ...demoCodexConfigurationPlan(), conflictCount: 0 }, { ...demoCodexConfigurationPlan(), revision: 'bad' }, { ...demoCodexConfigurationPlan(), targets: [] }, { ...demoCodexConfigurationPlan(), targets: demoCodexConfigurationPlan().targets.map(t => ({ ...t, conflictCodes: ['SECRET'] })) }, { ...demoCodexConfigurationPlan(), targets: demoCodexConfigurationPlan().targets.map(t => ({ ...t, path: '\u0000bad' })) }])('fails closed for incompatible result', value => { expect(() => validateCodexConfigurationPlan(value)).toThrow(); });
  it('accepts unmarked instructions merge and absent unchanged settings', () => {
    const plan = demoCodexConfigurationPlan();
    plan.targets = [{ ...plan.targets[0], state: 'present', action: 'merge' }, { ...plan.targets[1], state: 'absent', action: 'unchanged', conflictCodes: [], conflictCount: 0 }];
    plan.conflictCount = 0; expect(validateCodexConfigurationPlan(plan)).toEqual(plan);
  });
  it('shows labelled synthetic data without IPC in demo', async () => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', '/?demo=1');
    expect((await previewCodexConfiguration()).synthetic).toBe(true); expect(invoke).not.toHaveBeenCalled();
  });
  it('unavailable browser rejects, and Codex apply never invokes', async () => {
    await expect(configureAgent('codex')).rejects.toThrow('只读配置计划');
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    await expect(previewCodexConfiguration()).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
  });
});
