import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import * as bridge from './bridge';
import { validateQwenVimPreview } from './qwenContract';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const token = 'b'.repeat(64);
const preview = { field: 'general.vimMode', before: null, after: true, changes: true, token, limitations: 'SECRET', arbitrary: 'SECRET' };
beforeEach(() => { Object.assign(window, { __TAURI_INTERNALS__: {} }); });
afterEach(() => { delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', '/'); vi.clearAllMocks(); });
describe('Qwen owned setting bridge', () => {
  it('projects only fixed fields and uses explicit command arguments', async () => {
    vi.mocked(invoke).mockResolvedValue(preview);
    expect(JSON.stringify(await bridge.previewQwenVim('/settings.json', '/package.json', true))).not.toContain('SECRET');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('preview_qwen_vim', { settingsPath: '/settings.json', runtimeManifest: '/package.json', enabled: true });
    vi.mocked(invoke).mockResolvedValue(true);
    expect(await bridge.applyQwenVim('/settings.json', '/package.json', true, token)).toBe(true);
    expect(invoke).toHaveBeenLastCalledWith('apply_qwen_vim', { settingsPath: '/settings.json', runtimeManifest: '/package.json', enabled: true, token });
    expect(await bridge.restoreQwenVim('/settings.json', '/package.json', token)).toBe(true);
    expect(invoke).toHaveBeenLastCalledWith('restore_qwen_vim', { settingsPath: '/settings.json', runtimeManifest: '/package.json', token });
  });
  it.each([null, {}, { ...preview, field: 'security' }, { ...preview, before: 'SECRET' }, { ...preview, changes: false }, { ...preview, after: false }, { ...preview, token: 'SECRET' }])('rejects incompatible previews', response => {
    expect(() => validateQwenVimPreview(response, true)).toThrow();
  });
  it('rejects non-boolean mutation responses', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true, secret: 'SECRET' });
    await expect(bridge.applyQwenVim('/settings.json', '/package.json', true, token)).rejects.toThrow();
    await expect(bridge.restoreQwenVim('/settings.json', '/package.json', token)).rejects.toThrow();
  });
  it.each(['', '\u0000bad', 'x'.repeat(4097)])('rejects invalid paths before IPC', async path => {
    await expect(bridge.previewQwenVim(path, '/package.json', true)).rejects.toThrow();
    await expect(bridge.applyQwenVim('/settings.json', path, true, token)).rejects.toThrow();
    await expect(bridge.restoreQwenVim(path, '/package.json', token)).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
  it('rejects invalid tokens before IPC', async () => {
    await expect(bridge.applyQwenVim('/settings.json', '/package.json', true, 'bad')).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
  it.each(['/', '/?demo=1'])('never invokes management outside native mode %s', async url => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', url);
    await expect(bridge.previewQwenVim('/settings.json', '/package.json', true)).rejects.toThrow();
    await expect(bridge.applyQwenVim('/settings.json', '/package.json', true, token)).rejects.toThrow();
    await expect(bridge.restoreQwenVim('/settings.json', '/package.json', token)).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('Qwen read-only recovery bridge', () => {
  it.each(['no_receipt', 'restore_field', 'cleanup_receipt', 'conflict'])('projects %s without dynamic data', async state => {
    vi.mocked(invoke).mockResolvedValue({ field: 'general.vimMode', state, changes_settings: state === 'restore_field', limitations: 'SECRET', path: 'SECRET', digest: 'SECRET' });
    expect(await bridge.inspectQwenVimRecovery('/settings.json', '/package.json')).toEqual({ field: 'general.vimMode', state, changesSettings: state === 'restore_field' });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_qwen_vim_recovery', { settingsPath: '/settings.json', runtimeManifest: '/package.json' });
  });
  it.each([null, {}, { field: 'SECRET', state: 'no_receipt', changes_settings: false }, { field: 'general.vimMode', state: 'SECRET', changes_settings: false }, { field: 'general.vimMode', state: 'conflict', changes_settings: true }, { field: 'general.vimMode', state: 'restore_field', changes_settings: false }, { field: 'general.vimMode', state: 'no_receipt', changes_settings: 'false' }])('rejects incompatible recovery responses', async response => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(bridge.inspectQwenVimRecovery('/settings.json', '/package.json')).rejects.toThrow('响应不兼容');
  });
  it.each(['', '\u0000bad', 'x'.repeat(4097)])('rejects invalid path before reading', async path => {
    await expect(bridge.inspectQwenVimRecovery(path, '/package.json')).rejects.toThrow();
    await expect(bridge.inspectQwenVimRecovery('/settings.json', path)).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
  it.each(['/', '/?demo=1'])('does not read outside native mode', async url => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', url);
    await expect(bridge.inspectQwenVimRecovery('/settings.json', '/package.json')).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('Qwen owned update bridge', () => {
  it('projects only fixed fields and uses explicit command arguments', async () => {
    vi.mocked(invoke).mockResolvedValue(preview);
    expect(JSON.stringify(await bridge.previewQwenVimUpdate('/settings.json', '/package.json', true))).not.toContain('SECRET');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('preview_qwen_vim_update', { settingsPath: '/settings.json', runtimeManifest: '/package.json', enabled: true });
    vi.mocked(invoke).mockResolvedValue(true);
    expect(await bridge.updateQwenVim('/settings.json', '/package.json', true, token)).toBe(true);
    expect(invoke).toHaveBeenLastCalledWith('update_qwen_vim', { settingsPath: '/settings.json', runtimeManifest: '/package.json', enabled: true, token });
    expect(await bridge.restoreQwenVim('/settings.json', '/package.json', token)).toBe(true);
    expect(invoke).toHaveBeenLastCalledWith('restore_qwen_vim', { settingsPath: '/settings.json', runtimeManifest: '/package.json', token });
  });
  it.each([null, {}, { ...preview, field: 'security' }, { ...preview, before: 'SECRET' }, { ...preview, changes: false }, { ...preview, after: false }, { ...preview, token: 'SECRET' }])('rejects incompatible previews', response => {
    expect(() => validateQwenVimPreview(response, true)).toThrow();
  });
  it('rejects non-boolean mutation responses', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true, secret: 'SECRET' });
    await expect(bridge.updateQwenVim('/settings.json', '/package.json', true, token)).rejects.toThrow();
    await expect(bridge.restoreQwenVim('/settings.json', '/package.json', token)).rejects.toThrow();
  });
  it.each(['', '\u0000bad', 'x'.repeat(4097)])('rejects invalid paths before IPC', async path => {
    await expect(bridge.previewQwenVimUpdate(path, '/package.json', true)).rejects.toThrow();
    await expect(bridge.updateQwenVim('/settings.json', path, true, token)).rejects.toThrow();
    await expect(bridge.restoreQwenVim(path, '/package.json', token)).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
  it('rejects invalid tokens before IPC', async () => {
    await expect(bridge.updateQwenVim('/settings.json', '/package.json', true, 'bad')).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
  it.each(['/', '/?demo=1'])('never invokes management outside native mode %s', async url => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', url);
    await expect(bridge.previewQwenVimUpdate('/settings.json', '/package.json', true)).rejects.toThrow();
    await expect(bridge.updateQwenVim('/settings.json', '/package.json', true, token)).rejects.toThrow();
    await expect(bridge.restoreQwenVim('/settings.json', '/package.json', token)).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('Qwen checked restoration bridge', () => {
  const restorePreview = { field: 'general.vimMode', state: 'restore_field', current: true, initial: null, changes_settings: true, token, limitations: 'SECRET', extra: 'SECRET' };
  it('reads separate snapshot and projects bounded fields', async () => {
    vi.mocked(invoke).mockResolvedValue(restorePreview);
    expect(await bridge.previewQwenVimRestore('/settings.json', '/package.json')).toEqual({ field: 'general.vimMode', state: 'restore_field', current: true, initial: null, changesSettings: true, token });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('preview_qwen_vim_restore', { settingsPath: '/settings.json', runtimeManifest: '/package.json' });
  });
  it.each([null, {}, ...['no_receipt', 'conflict', 'SECRET'].map(state => ({ ...restorePreview, state })),
    { ...restorePreview, field: 'SECRET' }, { ...restorePreview, current: 'SECRET' }, { ...restorePreview, initial: 0 },
    { ...restorePreview, current: null },
    { ...restorePreview, state: 'cleanup_receipt', changes_settings: false },
    { ...restorePreview, state: 'cleanup_receipt', current: false, initial: true, changes_settings: false },
    { ...restorePreview, changes_settings: false }, { ...restorePreview, token: 'SECRET' },
    { ...restorePreview, token: 'A'.repeat(64) }])('rejects malformed or nonrestorable response', async value => {
    vi.mocked(invoke).mockResolvedValue(value); await expect(bridge.previewQwenVimRestore('/settings.json', '/package.json')).rejects.toThrow('响应不兼容');
  });
  it('accepts restoring equal booleans when receipt recovery still requires a write', async () => {
    vi.mocked(invoke).mockResolvedValue({ ...restorePreview, current: true, initial: true });
    expect(await bridge.previewQwenVimRestore('/settings.json', '/package.json')).toMatchObject({ state: 'restore_field', current: true, initial: true, changesSettings: true });
  });
  it('accepts cleanup with matching current and initial values', async () => {
    vi.mocked(invoke).mockResolvedValue({ ...restorePreview, state: 'cleanup_receipt', current: null, changes_settings: false });
    expect(await bridge.previewQwenVimRestore('/settings.json', '/package.json')).toMatchObject({ state: 'cleanup_receipt', current: null, initial: null, changesSettings: false });
  });
  it.each([undefined, null, '', 'bad', 'A'.repeat(64), 'a'.repeat(65)])('requires a valid token before write IPC', async value => {
    await expect(bridge.restoreQwenVim('/settings.json', '/package.json', value as string)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
  });
  it.each(['', '\u0000bad', 'x'.repeat(4097)])('bounds restore preview paths', async path => {
    await expect(bridge.previewQwenVimRestore(path, '/package.json')).rejects.toThrow();
    await expect(bridge.previewQwenVimRestore('/settings.json', path)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
  });
  it.each(['/', '/?demo=1'])('does not preview outside native mode', async url => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', url);
    await expect(bridge.previewQwenVimRestore('/settings.json', '/package.json')).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
  });
});
