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
    ['codex', '@openai/codex', 'codex', 'disabled'],
    ['gemini-cli', '@google/gemini-cli', 'gemini', 'npm_configured'],
    ['pi', '@earendil-works/pi-coding-agent', 'pi', 'disabled'],
  ])('validates exact supported package policy for %s', (id, pkg, executable, scriptPolicy) => {
    expect(parseInstallPreview({ ...installPlan, agentId: id, package: pkg, executable, scriptPolicy, packageRoot: `${installPlan.globalRoot}/${pkg}` }, id).package).toBe(pkg);
    expect(() => parseInstallPreview({ ...installPlan, agentId: id, package: pkg, executable, scriptPolicy: scriptPolicy === 'disabled' ? 'npm_configured' : 'disabled', packageRoot: `${installPlan.globalRoot}/${pkg}` }, id)).toThrow();
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
