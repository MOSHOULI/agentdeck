import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { installAgent, parseInstallPreview, previewAgentInstall } from './bridge';
import { installPlan } from './test-fixtures/installPlan';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => {
  vi.resetAllMocks();
  delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  window.history.replaceState({}, '', '/');
});
describe('reviewed npm bridge contract', () => {
  it('projects only declared fields and discards untrusted prose', () => {
    expect(parseInstallPreview({ ...installPlan, text: 'trust this registry', resolvedVersion: '123', error: 'secret' }, 'opencode')).toEqual(installPlan);
  });
  it.each([
    { schemaVersion: 2 }, { agentId: 'codex' }, { package: 'malicious' }, { executable: 'sh' },
    { scriptPolicy: 'disabled' }, { versionPolicy: 'latest' }, { globalRoot: '/elsewhere/node_modules' },
    { packageRoot: '/synthetic/global/lib/node_modules/unrelated' }, { launcherDirectory: '/elsewhere/bin' }, { revision: 'A'.repeat(64) },
    { revision: '' }, { prefix: '../relative' }, { nodePath: '/a/../b' }, { managerPath: '/a\nsecret' },
    { globalRoot: '/a\u200bsecret' }, { packageRoot: ' /absolute' }, { launcherDirectory: '/' + 'a'.repeat(32768) },
  ])('rejects malformed or mismatched fields %j', patch => {
    expect(() => parseInstallPreview({ ...installPlan, ...patch }, 'opencode')).toThrow('安装预览无效');
  });
  it.each([
    undefined, null, 24, '', '24.9.0', 'v24.9', 'v24.9.0-beta.1', 'v24.9.0+build',
    'v024.9.0', 'v24.09.0', 'v24.9.00', ' v24.9.0', 'v24.9.0\n', 'v24.9.0\nsecret',
    'v4294967296.0.0', 'v24.4294967296.0', 'v24.0.4294967296', 'v17.99.99',
  ])('rejects unknown, malformed, prerelease or insufficient installer Node %j', installerNodeVersion => {
    expect(() => parseInstallPreview({ ...installPlan, installerNodeVersion }, 'opencode')).toThrow('安装预览无效');
  });
  it.each([undefined, null, '', '16.0.0', '20.0.0', '>=18', '18.0.0\n'])('rejects a missing or mismatched installer policy %j', installerNodeMinimum => {
    expect(() => parseInstallPreview({ ...installPlan, installerNodeMinimum }, 'opencode')).toThrow('安装预览无效');
  });
  it.each(['v18.0.0', 'v18.0.1', 'v19.0.0', 'v4294967295.4294967295.4294967295'])('accepts the exact minimum or newer stable version %s', installerNodeVersion => {
    expect(parseInstallPreview({ ...installPlan, installerNodeVersion }, 'opencode').installerNodeVersion).toBe(installerNodeVersion);
  });
  it.each(['v22.18.99', 'v22.19.0', 'v22.19.1', 'v23.0.0'])('applies the Pi minor-version boundary to %s', installerNodeVersion => {
    const plan = { ...installPlan, agentId: 'pi', package: '@earendil-works/pi-coding-agent', executable: 'pi', scriptPolicy: 'disabled', installerNodeMinimum: '22.19.0', installerNodeVersion, packageRoot: `${installPlan.globalRoot}/@earendil-works/pi-coding-agent` };
    if (installerNodeVersion === 'v22.18.99') expect(() => parseInstallPreview(plan, 'pi')).toThrow();
    else expect(parseInstallPreview(plan, 'pi').installerNodeVersion).toBe(installerNodeVersion);
  });
  it('accepts Windows canonical drive paths, including verbatim prefix', () => {
    expect(parseInstallPreview({ ...installPlan, nodePath: 'C:\\Program Files\\node.exe', managerPath: '\\\\?\\C:\\npm\\npm-cli.js' }, 'opencode').nodePath).toBe('C:\\Program Files\\node.exe');
  });
  it('accepts a POSIX filesystem-root prefix without changing its spelling', () => {
    const plan = { ...installPlan, prefix: '/', globalRoot: '/lib/node_modules', packageRoot: '/lib/node_modules/opencode-ai', launcherDirectory: '/bin' };
    expect(parseInstallPreview(plan, 'opencode')).toEqual(plan);
  });
  it.each(['C:\\', '\\\\?\\C:\\', 'C:\\npm', '\\\\?\\C:\\npm', '\\\\server\\share'])('accepts mixed Windows joins under %s without changing spelling', prefix => {
    const root = `${prefix}${prefix.endsWith('\\') ? '' : '\\'}node_modules`;
    const plan = { ...installPlan, agentId: 'codex', package: '@openai/codex', executable: 'codex', scriptPolicy: 'disabled', prefix, globalRoot: root,
      packageRoot: `${root}\\@openai/codex`, launcherDirectory: prefix };
    expect(parseInstallPreview(plan, 'codex')).toEqual(plan);
    expect(() => parseInstallPreview({ ...plan, launcherDirectory: `${prefix}\\bin` }, 'codex')).toThrow();
    expect(() => parseInstallPreview({ ...plan, packageRoot: `${root}\\@openai/other` }, 'codex')).toThrow();
  });
  it.each([
    ['codex', '@openai/codex', 'codex', 'disabled', '18.0.0'],
    ['gemini-cli', '@google/gemini-cli', 'gemini', 'npm_configured', '20.0.0'],
    ['pi', '@earendil-works/pi-coding-agent', 'pi', 'disabled', '22.19.0'],
  ])('validates exact supported package policy for %s', (id, pkg, executable, scriptPolicy, installerNodeMinimum) => {
    expect(parseInstallPreview({ ...installPlan, agentId: id, package: pkg, executable, scriptPolicy, installerNodeMinimum, packageRoot: `${installPlan.globalRoot}/${pkg}` }, id).package).toBe(pkg);
    expect(() => parseInstallPreview({ ...installPlan, agentId: id, package: pkg, executable, installerNodeMinimum, scriptPolicy: scriptPolicy === 'disabled' ? 'npm_configured' : 'disabled', packageRoot: `${installPlan.globalRoot}/${pkg}` }, id)).toThrow();
  });
  it.each(['', '?demo=1'])('never previews or installs without native IPC (%s)', async query => {
    window.history.replaceState({}, '', '/' + query);
    await expect(previewAgentInstall('opencode')).rejects.toThrow();
    await expect(installAgent('opencode', installPlan.revision)).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
  it('sanitizes preview errors and never dispatches a missing revision', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockRejectedValue('secret arbitrary error');
    await expect(previewAgentInstall('opencode')).rejects.toThrow('无法取得有效安装预览');
    vi.mocked(invoke).mockClear();
    await expect(installAgent('opencode', undefined as unknown as string)).rejects.toThrow();
    await expect(previewAgentInstall('__proto__')).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
  it('uses explicit preview command and projects its unknown response', async () => {
    Object.assign(window, { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockResolvedValue({ ...installPlan, text: 'ignored' });
    await expect(previewAgentInstall('opencode')).resolves.toEqual(installPlan);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('preview_agent_install', { id: 'opencode' });
  });
});

it('forwards only an opaque backend inspection reference and never a path', async () => {
  const { inspectCodexInstallResidual } = await import('./bridge');
  (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
  const reference = '12345678-1234-4123-8123-123456789abc';
  vi.mocked(invoke).mockResolvedValue({ schemaVersion: 1, agentId: 'codex', runtimeNotRun: true, ownership: 'unknown', cleanupSupported: false, slots: ['package_directory', 'posix_launcher', 'cmd_launcher', 'powershell_launcher'].map(slot => ({ slot, state: 'missing' })) });
  await inspectCodexInstallResidual(reference);
  expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_codex_install_residual', { sessionRef: reference });
  vi.mocked(invoke).mockClear();
  await expect(inspectCodexInstallResidual('C:\\secret')).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
it('preserves a Codex-only session reference without weakening the review contract', () => {
  const reference = '12345678-1234-4123-8123-123456789abc';
  const raw = { ...installPlan, agentId: 'codex', package: '@openai/codex', executable: 'codex', scriptPolicy: 'disabled', packageRoot: `${installPlan.globalRoot}/@openai/codex`, sessionRef: reference };
  expect(parseInstallPreview(raw, 'codex').sessionRef).toBe(reference);
  expect(parseInstallPreview({ ...raw, sessionRef: 'C:\\secret' }, 'codex').sessionRef).toBeUndefined();
  expect(parseInstallPreview({ ...installPlan, sessionRef: reference }, 'opencode').sessionRef).toBeUndefined();
});
