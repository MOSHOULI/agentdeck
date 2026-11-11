import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { deployQoderPrivate, previewQoderDeployment } from './bridge';
import { validateQoderDeploymentPreview, validateQoderDeploymentOutcome } from './qoderDeploymentContract';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const raw = { version: '1.1.85', target: 'linux-amd64-baseline', executable_mode: '0700', payload_bytes: 182543840, payload_sha256: '8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b', token: 'a'.repeat(64), limitations: 'SECRET RAW TEXT' };
const outcome = { version: '1.1.85', target: 'linux-amd64-baseline', executable_mode: '0700', state: 'private_executable_deployed_not_activated', execution_verified: false, activated: false, limitations: 'SECRET' };
afterEach(() => { vi.clearAllMocks(); window.history.replaceState({}, '', '/'); delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; });
it('projects only fixed verified preview and result fields, never backend prose', () => {
  expect(validateQoderDeploymentPreview(raw)).toEqual({ token: 'a'.repeat(64) });
  expect(validateQoderDeploymentOutcome(outcome)).toEqual({ state: 'private_executable_deployed_not_activated' });
});
it.each([{ version: '1.1.86' }, { target: 'linux-arm64' }, { executable_mode: '0755' }, { payload_bytes: 1 }, { payload_sha256: 'a'.repeat(64) }, { token: 'A'.repeat(64) }])('rejects mismatched preview %j', patch => { expect(() => validateQoderDeploymentPreview({ ...raw, ...patch })).toThrow(); });
it.each([{ activated: true }, { execution_verified: true }, { state: 'installed' }, { executable_mode: '0755' }, { version: '1.1.86' }])('rejects incompatible outcome %j', patch => { expect(() => validateQoderDeploymentOutcome({ ...outcome, ...patch })).toThrow(); });
it('native bridge preserves exact paths and requires confirmation/token before IPC', async () => {
  Object.assign(window, { __TAURI_INTERNALS__: {} }); vi.mocked(invoke).mockResolvedValue(raw);
  await previewQoderDeployment('/source with space', '/new');
  expect(invoke).toHaveBeenCalledExactlyOnceWith('preview_qoder_deployment', { source: '/source with space', destination: '/new' });
  vi.mocked(invoke).mockClear();
  await expect(deployQoderPrivate('/source', '/new', raw.token, false)).rejects.toThrow();
  await expect(deployQoderPrivate('/source', '/new', 'bad', true)).rejects.toThrow();
  for (const path of ['relative', '', '/bad\npath', '/' + '中'.repeat(1366)]) await expect(previewQoderDeployment(path, '/new')).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
  vi.mocked(invoke).mockResolvedValue(outcome); await deployQoderPrivate('/source with space', '/new', raw.token, true);
  expect(invoke).toHaveBeenCalledExactlyOnceWith('deploy_qoder_private', { source: '/source with space', destination: '/new', token: raw.token, confirmed: true });
});
it('browser/demo never simulate deployment', async () => {
  await expect(previewQoderDeployment('/source', '/new')).rejects.toThrow();
  window.history.replaceState({}, '', '/?demo=1');
  await expect(previewQoderDeployment('/source', '/new')).rejects.toThrow();
  await expect(deployQoderPrivate('/source', '/new', raw.token, true)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});
it('malformed post-write outcome rejects rather than claiming completion', async () => {
  Object.assign(window, { __TAURI_INTERNALS__: {} }); vi.mocked(invoke).mockResolvedValue({ ...outcome, execution_verified: true });
  await expect(deployQoderPrivate('/source', '/new', raw.token, true)).rejects.toThrow(); expect(invoke).toHaveBeenCalledTimes(1);
});
