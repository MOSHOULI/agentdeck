import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import * as bridge from './bridge';
import { qoderTargets, validateQoderPlan, validateQoderVerification } from './qoderContract';
import { qoderRawPlan } from './test-fixtures/qoder';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { Object.assign(window, { __TAURI_INTERNALS__: {} }); });
afterEach(() => { delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', '/'); vi.clearAllMocks(); });
it.each(qoderTargets)('strictly projects reviewed target %s', async target => {
  const raw = qoderRawPlan(target); vi.mocked(invoke).mockResolvedValue(raw);
  const plan = await bridge.qoderDistributionPlan(target);
  expect(plan.target).toBe(target); expect(JSON.stringify(plan)).not.toContain('SECRET');
  expect(invoke).toHaveBeenCalledExactlyOnceWith('qoder_distribution_plan', { target });
  vi.mocked(invoke).mockResolvedValue({ state: 'matches_pinned_archive', bytes: 123, plan: raw, path: 'SECRET' });
  expect(await bridge.verifyQoderPackage(target, '/explicit/archive')).toEqual({ plan, bytes: 123 });
  expect(invoke).toHaveBeenLastCalledWith('verify_qoder_package', { target, archivePath: '/explicit/archive' });
});
it.each(['version', 'manifest_sha256', 'target', 'url', 'sha256', 'archive_name', 'runtime', 'variant', 'min_windows_build', 'host_eligibility', 'install_supported', 'execution_verified', 'signature_verified', 'max_archive_bytes'])('rejects incompatible field %s', key => {
  expect(() => validateQoderPlan({ ...qoderRawPlan(), [key]: 'SECRET' }, 'linux-amd64-baseline')).toThrow();
});
it.each([null, {}, [], 'SECRET'])('rejects non-plan values', value => { expect(() => validateQoderPlan(value, 'linux-amd64-baseline')).toThrow(); });
it.each([0, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER, 268435457, '123', null])('rejects invalid byte counts', bytes => {
  expect(() => validateQoderVerification({ state: 'matches_pinned_archive', bytes, plan: qoderRawPlan() }, 'linux-amd64-baseline')).toThrow();
});
it('rejects a success response for a different selected target or state', () => {
  expect(() => validateQoderVerification({ state: 'installed', bytes: 123, plan: qoderRawPlan() }, 'linux-amd64-baseline')).toThrow();
  expect(() => validateQoderPlan(qoderRawPlan('windows-arm64-standard'), 'linux-amd64-baseline')).toThrow();
});
it.each(['', '\u0000bad', 'x'.repeat(4097)])('rejects invalid paths before dispatch', async path => {
  await expect(bridge.verifyQoderPackage('linux-amd64-baseline', path)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});
it('rejects ambiguous targets before dispatch', async () => {
  await expect(bridge.qoderDistributionPlan('windows-amd64')).rejects.toThrow();
  await expect(bridge.verifyQoderPackage('auto', '/path')).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});
it.each(['/', '/?demo=1'])('never reads in browser or demo mode %s', async url => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', url);
  await expect(bridge.qoderDistributionPlan('linux-amd64-baseline')).rejects.toThrow();
  await expect(bridge.verifyQoderPackage('linux-amd64-baseline', '/path')).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});

it.each(['staged_archive_verified', 'unpacked_payload_verified', 'incomplete', 'conflict', 'unknown_ownership'])('projects cache state %s without backend strings', async state => {
  const recovery_plan = state.endsWith('_verified') ? 'retain_cache_no_action' : 'preserve_and_review';
  vi.mocked(invoke).mockResolvedValue({ state, recovery_plan, deletion_authorized: false, manual_review_required: true, installed: false, limitations: 'SECRET', path: 'SECRET' });
  const result = await bridge.inspectQoderCache('/explicit/cache');
  expect(result.state).toBe(state); expect(result.deletionAuthorized).toBe(false);
  expect(JSON.stringify(result)).not.toContain('SECRET');
  expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_qoder_cache', { cachePath: '/explicit/cache' });
});
it.each(['state', 'recovery_plan', 'deletion_authorized', 'manual_review_required', 'installed'])('rejects inconsistent cache %s', async field => {
  vi.mocked(invoke).mockResolvedValue({ state: 'incomplete', recovery_plan: 'preserve_and_review', deletion_authorized: false, manual_review_required: true, installed: false, [field]: 'SECRET' });
  await expect(bridge.inspectQoderCache('/explicit/cache')).rejects.toThrow();
});
it.each(['', ' ', 'SECRET\npath', 'x'.repeat(4097), '字'.repeat(1366)])('blocks bad cache path before IPC', async path => {
  await expect(bridge.inspectQoderCache(path)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});
it.each(['/', '/?demo=1'])('never fakes cache observation in browser %s', async url => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', url);
  await expect(bridge.inspectQoderCache('/explicit/cache')).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});

it.each(['stage','unpack'] as const)('strict write %s response is projected and flags cannot grant execution', async operation => {
  const state=operation === 'stage' ? 'staged_archive_verified' : 'unpacked_payload_verified';
  const raw={operation,state,target:'linux-amd64-baseline',version:'1.1.85',installed:false,executable:false,path:'SECRET'};
  vi.mocked(invoke).mockResolvedValue(raw);
  expect(await bridge.writeQoderCache(operation,'/source','/new',true)).toEqual({operation,state});
  expect(invoke).toHaveBeenCalledExactlyOnceWith('write_qoder_cache',{operation,target:'linux-amd64-baseline',source:'/source',destination:'/new',confirmed:true});
  for (const patch of [{installed:true},{executable:true},{version:'latest'},{operation:'execute'},{state:'incomplete'},{target:'other'}]) {
    vi.mocked(invoke).mockResolvedValue({...raw,...patch});
    await expect(bridge.writeQoderCache(operation,'/source','/new',true)).rejects.toThrow();
  }
});
it('write rejects missing confirmation, relative or unbounded input and demo without invoking', async () => {
  await expect(bridge.writeQoderCache('stage','/source','/new',false)).rejects.toThrow();
  for (const path of ['relative','/bad\npath','/'+ 'x'.repeat(4096), '/' + '界'.repeat(1400)]) await expect(bridge.writeQoderCache('stage',path,'/new',true)).rejects.toThrow();
  await expect(bridge.writeQoderCache('execute' as 'stage','/source','/new',true)).rejects.toThrow();
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  await expect(bridge.writeQoderCache('stage','/source','/new',true)).rejects.toThrow();
  window.history.replaceState({},'', '/?demo=1');
  await expect(bridge.writeQoderCache('stage','/source','/new',true)).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
