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
    expect(await bridge.restoreQwenVim('/settings.json', '/package.json')).toBe(true);
    expect(invoke).toHaveBeenLastCalledWith('restore_qwen_vim', { settingsPath: '/settings.json', runtimeManifest: '/package.json' });
  });
  it.each([null, {}, { ...preview, field: 'security' }, { ...preview, before: 'SECRET' }, { ...preview, changes: false }, { ...preview, after: false }, { ...preview, token: 'SECRET' }])('rejects incompatible previews', response => {
    expect(() => validateQwenVimPreview(response, true)).toThrow();
  });
  it('rejects non-boolean mutation responses', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true, secret: 'SECRET' });
    await expect(bridge.applyQwenVim('/settings.json', '/package.json', true, token)).rejects.toThrow();
    await expect(bridge.restoreQwenVim('/settings.json', '/package.json')).rejects.toThrow();
  });
  it.each(['', '\u0000bad', 'x'.repeat(4097)])('rejects invalid paths before IPC', async path => {
    await expect(bridge.previewQwenVim(path, '/package.json', true)).rejects.toThrow();
    await expect(bridge.applyQwenVim('/settings.json', path, true, token)).rejects.toThrow();
    await expect(bridge.restoreQwenVim(path, '/package.json')).rejects.toThrow();
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
    await expect(bridge.restoreQwenVim('/settings.json', '/package.json')).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
});
