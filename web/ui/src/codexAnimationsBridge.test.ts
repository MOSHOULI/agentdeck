import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import * as bridge from './bridge';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const token='a'.repeat(64);
beforeEach(() => { Object.assign(window,{__TAURI_INTERNALS__:{}}); });
afterEach(() => { delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__; window.history.replaceState({},'', '/'); vi.clearAllMocks(); });
it('uses only explicit bounded fields and checked restore token', async () => {
  vi.mocked(invoke).mockResolvedValue({field:'tui.animations',before:true,after:false,changes:true,token,secret:'secret'});
  expect(await bridge.previewCodexAnimations(false)).not.toHaveProperty('secret'); expect(invoke).toHaveBeenLastCalledWith('preview_codex_animations',{enabled:false});
  vi.mocked(invoke).mockResolvedValue(true); await bridge.applyCodexAnimations(false,token); expect(invoke).toHaveBeenLastCalledWith('apply_codex_animations',{enabled:false,token});
  vi.mocked(invoke).mockResolvedValue({state:'restore_original',token}); await bridge.inspectCodexAnimationsRecovery(); expect(invoke).toHaveBeenLastCalledWith('inspect_codex_animations_recovery');
  vi.mocked(invoke).mockResolvedValue(true); await bridge.restoreCodexAnimations(token); expect(invoke).toHaveBeenLastCalledWith('restore_codex_animations',{token});
});
it('rejects malformed input before IPC', async () => {
  await expect(bridge.applyCodexAnimations(false,'bad')).rejects.toThrow(); await expect(bridge.restoreCodexAnimations('bad')).rejects.toThrow(); await expect(bridge.previewCodexAnimations('false' as unknown as boolean)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});
it.each(['/', '/?demo=1'])('does not perform or simulate file operations outside native %s', async url => {
  delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__; window.history.replaceState({},'',url);
  await expect(bridge.previewCodexAnimations(false)).rejects.toThrow(); await expect(bridge.applyCodexAnimations(false,token)).rejects.toThrow(); await expect(bridge.inspectCodexAnimationsRecovery()).rejects.toThrow(); await expect(bridge.restoreCodexAnimations(token)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});
