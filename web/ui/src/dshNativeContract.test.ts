import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import catalog from '../../../docs/upstream/dsh/native-evidence/artifacts.json';
import {dshNativeEvidence, dshNativePackages, requireDshNativePackage, validateDshNativeVerification, type DshNativePackage} from './dshNativeContract';
import * as bridge from './bridge';
vi.mock('@tauri-apps/api/core', () => ({invoke: vi.fn()}));
beforeEach(() => Object.assign(window, {__TAURI_INTERNALS__: {}}));
afterEach(() => { delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', '/'); vi.clearAllMocks(); });
const raw = (id: DshNativePackage = 'node-pty') => ({
  schema_version: 1, state: 'native_archive_static_verified', package_id: id, ...dshNativeEvidence[id],
  archive_verified_during_this_command: true, reviewed_manifest_and_sources_matched: true,
  metadata_archive_agreement: 'reviewed_declaration_fields_match', lifecycle_executed: false,
  native_code_loaded: false, closure_complete: false, install_ready: false, dependencies_resolved: false,
  runtime_verified: false, signature_verified: false, platform_compatibility: 'unverified',
});
it('compact frontend evidence exactly matches all four reviewed artifacts', () => {
  expect(catalog.artifacts.map(a => a.id)).toEqual(dshNativePackages);
  for (const a of catalog.artifacts) {
    requireDshNativePackage(a.id);
    expect(dshNativeEvidence[a.id]).toEqual({package: a.package, version: a.version, archive_name: a.archive_name,
      bytes: a.archive_bytes, sha256: a.archive_sha256, integrity: a.integrity, regular_member_count: a.members.length});
  }
});
it.each(dshNativePackages)('validates exact %s and invokes once with explicit selection', async id => {
  vi.mocked(invoke).mockResolvedValue(raw(id));
  const archivePath = '/selected/' + dshNativeEvidence[id].archive_name;
  expect(await bridge.verifyDshNativePackage(id, archivePath)).toEqual(raw(id));
  expect(invoke).toHaveBeenCalledExactlyOnceWith('verify_dsh_native_package', {packageId: id, archivePath});
});
it.each(dshNativePackages)('rejects every changed, missing or extra field for %s', id => {
  for (const key of Object.keys(raw(id))) {
    expect(() => validateDshNativeVerification({...raw(id), [key]: 'wrong'}, id)).toThrow('DSH_NATIVE_INVALID_RESPONSE');
    const missing: Record<string, unknown> = raw(id); delete missing[key];
    expect(() => validateDshNativeVerification(missing, id)).toThrow('DSH_NATIVE_INVALID_RESPONSE');
  }
  for (const key of ['lifecycle_executed','native_code_loaded','closure_complete','install_ready','dependencies_resolved','runtime_verified','signature_verified']) {
    expect(() => validateDshNativeVerification({...raw(id), [key]: true}, id)).toThrow('DSH_NATIVE_INVALID_RESPONSE');
  }
  for (const value of [null, [], 'SECRET', {...raw(id), installed: true}, {...raw(id), path: 'SECRET'}]) {
    expect(() => validateDshNativeVerification(value, id)).toThrow('DSH_NATIVE_INVALID_RESPONSE');
  }
  for (const other of dshNativePackages.filter(x => x !== id)) expect(() => validateDshNativeVerification(raw(other), id)).toThrow('DSH_NATIVE_INVALID_RESPONSE');
});
it('rejects unsupported package IDs before IPC, including prototype names', async () => {
  for (const id of ['', 'NODE-PTY', 'node-pty@latest', '__proto__', 'constructor', 'koffi-linux-arm64', null, {}, 'x'.repeat(4097)]) {
    await expect(bridge.verifyDshNativePackage(id as DshNativePackage, '/selected/koffi-3.1.1.tgz')).rejects.toThrow('DSH_NATIVE_INVALID_PACKAGE');
  }
  expect(invoke).not.toHaveBeenCalled();
});
it('rejects malformed, relative, oversized, wrong-name or traversal paths before IPC', async () => {
  for (const path of ['', 'koffi-3.1.1.tgz','relative/koffi-3.1.1.tgz','C:koffi-3.1.1.tgz',
    '/wrong.tgz', '/secret\n/koffi-3.1.1.tgz','/secret\u0085/koffi-3.1.1.tgz', '/' + '界'.repeat(1400) + '/koffi-3.1.1.tgz',
    '/a/../koffi-3.1.1.tgz', '//server/koffi-3.1.1.tgz', '\\\\server\\koffi-3.1.1.tgz']) {
    await expect(bridge.verifyDshNativePackage('koffi', path)).rejects.toThrow('DSH_NATIVE_INVALID_PATH');
  }
  expect(invoke).not.toHaveBeenCalled();
});
it('allows drive-absolute selection while backend remains the filesystem authority', async () => {
  vi.mocked(invoke).mockResolvedValue(raw('koffi'));
  await bridge.verifyDshNativePackage('koffi', 'C:\\selected\\koffi-3.1.1.tgz');
  expect(invoke).toHaveBeenCalledTimes(1);
});
it.each(['unavailable', 'demo'])('never simulates verification in %s', async mode => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  if (mode === 'demo') window.history.replaceState({}, '', '/?demo=1');
  await expect(bridge.verifyDshNativePackage('koffi', '/selected/koffi-3.1.1.tgz')).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
