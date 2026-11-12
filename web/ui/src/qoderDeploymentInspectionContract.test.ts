import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { validateDeploymentInspection } from './qoderDeploymentInspectionContract';
import { inspectQoderDeployment } from './bridge';
import { qoderPayloads } from './qoderReleases';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const raw = { version: '1.1.85', target: 'linux-amd64-baseline', state: 'payload_verified', payload_bytes: qoderPayloads['1.1.85'].bytes, payload_sha256: qoderPayloads['1.1.85'].sha256, execution_verified: false, activated: false, ownership_proven: false, deletion_authorized: false, repair_authorized: false, manual_review_required: true, limitations: 'secret raw prose' };
afterEach(() => { vi.clearAllMocks(); delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; history.replaceState(null, '', '/'); });
it('projects fixed states, never backend prose or paths', () => {
  for (const state of ['payload_verified','missing','incomplete','unsupported_or_changed']) expect(validateDeploymentInspection({ ...raw, state }, '1.1.85')).toEqual({ state });
});
it('fails closed on unknown states, authority flags, missing fields and mixed version evidence', () => {
  for (const patch of [{ state: 'installed' }, { version: '1.1.86' }, { target: 'other' }, { payload_bytes: 1 }, { payload_sha256: 'bad' }, { manual_review_required: false }, { installed: true }, { path: 'secret' }, { limitations: '' }, { limitations: ' ' }, { limitations: null }, { limitations: 'x'.repeat(4097) }, ...['execution_verified','activated','ownership_proven','deletion_authorized','repair_authorized'].map(k => ({[k]:true}))]) expect(() => validateDeploymentInspection({ ...raw, ...patch }, '1.1.85')).toThrow();
  for (const key of Object.keys(raw)) { const missing = { ...raw } as Record<string, unknown>; delete missing[key]; expect(() => validateDeploymentInspection(missing,'1.1.85')).toThrow(); }
});
it('validates paths/releases before IPC and preserves exact selected path', async () => {
  Object.assign(window, { __TAURI_INTERNALS__: {} });
  for (const path of ['', 'relative', '/bad\npath', '/'+'x'.repeat(4096)]) await expect(inspectQoderDeployment(path,'1.1.85')).rejects.toThrow();
  await expect(inspectQoderDeployment('/existing', 'latest' as never)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
  vi.mocked(invoke).mockResolvedValue(raw);
  expect(await inspectQoderDeployment('/private with spaces/deployed','1.1.85')).toEqual({state:'payload_verified'});
  expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_qoder_deployment',{deploymentPath:'/private with spaces/deployed',release:'1.1.85'});
});
it('does not invoke in demo mode', async () => { history.replaceState(null,'','/?demo=1'); await expect(inspectQoderDeployment('/existing','1.1.85')).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled(); });

it('187 inspection preserves all authority boundaries and rejects any older payload evidence',async()=>{
 const next={...raw,version:'1.1.87',payload_bytes:182466016,payload_sha256:'61e318f6c4d2ebae319aecfce2f18472c51e7b413a546accddaeb82e2592af2e'};
 expect(validateDeploymentInspection(next,'1.1.87')).toEqual({state:'payload_verified'});
 for(const release of ['1.1.85','1.1.86'] as const){expect(()=>validateDeploymentInspection(next,release)).toThrow();for(const patch of [{version:release},{payload_bytes:qoderPayloads[release].bytes},{payload_sha256:qoderPayloads[release].sha256}])expect(()=>validateDeploymentInspection({...next,...patch},'1.1.87')).toThrow();}
 Object.assign(window,{__TAURI_INTERNALS__:{}});vi.mocked(invoke).mockResolvedValue(next);await inspectQoderDeployment('/private','1.1.87');expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_qoder_deployment',{deploymentPath:'/private',release:'1.1.87'});
});
