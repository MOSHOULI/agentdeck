import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import type { QwenVimRestorePreview } from '../qwenContract';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
import { QwenSettings } from './QwenSettings';
const token = 'c'.repeat(64);
const preview: QwenVimRestorePreview = { field: 'general.vimMode', state: 'restore_field', current: true, initial: false, changesSettings: true, token };
function defer<T>() { let resolve!: (value: T) => void; let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
function fill() {
  fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/selected/settings.json' } });
  fireEvent.change(screen.getByLabelText('运行时 package.json 绝对路径'), { target: { value: '/runtime/package.json' } });
}
const start = () => fireEvent.click(screen.getByRole('button', { name: '撤销受管 Vim 设置' }));
const ready = () => screen.findByRole('dialog', { name: '确认撤销 Vim 设置' });
beforeEach(() => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'previewQwenVimRestore').mockResolvedValue(preview);
  vi.spyOn(bridge, 'restoreQwenVim').mockResolvedValue(true);
  vi.spyOn(bridge, 'previewQwenVim').mockRejectedValue(new Error('SECRET'));
  vi.spyOn(bridge, 'inspectQwenSettings').mockRejectedValue(new Error('SECRET'));
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
describe('confirmed Qwen restoration snapshot', () => {
  it.each([
    { current: true, initial: false, text: '当前值：启用；首次原值：关闭。' },
    { current: false, initial: true, text: '当前值：关闭；首次原值：启用。' },
    { current: true, initial: null, text: '当前值：启用；首次原值：未声明。' },
    { current: false, initial: null, text: '当前值：关闭；首次原值：未声明。' },
    { current: null, initial: null, text: '当前值：未声明；首次原值：未声明。' },
    { current: true, initial: true, text: '当前值：启用；首次原值：启用。' },
    { current: false, initial: false, text: '当前值：关闭；首次原值：关闭。' },
  ])('renders one accessible summary text node: $text', async ({ current, initial, text }) => {
    const cleanupOnly = current === initial;
    vi.mocked(bridge.previewQwenVimRestore).mockResolvedValue({ ...preview, current, initial,
      state: cleanupOnly ? 'cleanup_receipt' : 'restore_field', changesSettings: !cleanupOnly });
    render(<QwenSettings />); fill(); start(); const dialog = await ready();
    const summary = screen.getByText(text, { exact: true });
    expect(dialog).toContainElement(summary);
    // Mixed JSX text fragments become separate Windows UIA text names. Keep
    // this concrete confirmation summary available as one exact text child.
    expect(summary.childNodes).toHaveLength(1);
    expect(summary.firstChild?.nodeType).toBe(Node.TEXT_NODE);
    expect(summary.firstChild?.textContent).toBe(text);
    expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
  });

  it.each([preview, { ...preview, state: 'cleanup_receipt' as const, current: false, changesSettings: false }])('shows concrete $state then submits captured token only once', async report => {
    vi.mocked(bridge.previewQwenVimRestore).mockResolvedValue(report);
    const work = defer<boolean>(); vi.mocked(bridge.restoreQwenVim).mockReturnValue(work.promise);
    render(<QwenSettings />); fill(); start(); const dialog = await ready();
    expect(dialog).toHaveTextContent(report.changesSettings ? '当前值：启用；首次原值：关闭。' : '当前值：关闭；首次原值：关闭。');
    expect(dialog).toHaveTextContent(report.changesSettings ? '动作：恢复首次原值并清理所有权记录。' : '动作：仅清理所有权记录，不改变配置字段。');
    expect(dialog).not.toHaveTextContent(token); expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
    const button = screen.getByRole('button', { name: '确认撤销 Vim 设置' }); fireEvent.click(button); fireEvent.click(button);
    expect(bridge.restoreQwenVim).toHaveBeenCalledExactlyOnceWith('/selected/settings.json', '/runtime/package.json', token);
    expect(screen.getByLabelText('settings.json 绝对路径')).toBeDisabled();
    expect(useWorkspaceActivityStore.getState().busy.qwen).toBe(true);
    await act(async () => work.resolve(report.changesSettings));
    expect(useWorkspaceActivityStore.getState().busy.qwen).toBe(false);
  });
  it.each(['cancel', 'settings', 'manifest', 'clear', 'unmount'])('discards late preview success/failure after %s while retaining ownership until settlement', async kind => {
    for (const failure of [false, true]) {
      const work = defer<QwenVimRestorePreview>(); vi.mocked(bridge.previewQwenVimRestore).mockReturnValue(work.promise);
      const view = render(<QwenSettings />); fill(); start(); start();
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      if (kind === 'cancel') fireEvent.click(screen.getByRole('button', { name: '取消撤销预览' }));
      if (kind === 'settings' || kind === 'manifest') fireEvent.change(screen.getByLabelText(kind === 'settings' ? 'settings.json 绝对路径' : '运行时 package.json 绝对路径'), { target: { value: '/other/' + (kind === 'settings' ? 'settings.json' : 'package.json') } });
      if (kind === 'clear') fireEvent.click(screen.getByRole('button', { name: '清除路径与结果' }));
      if (kind === 'unmount') { view.unmount(); render(<QwenSettings />); fill(); }
      expect(useWorkspaceActivityStore.getState().busy.qwen).toBe(true);
      expect(screen.getByRole('button', { name: '撤销受管 Vim 设置' })).toBeDisabled();
      await act(async () => { if (failure) work.reject(new Error('SECRET')); else work.resolve(preview); });
      expect(useWorkspaceActivityStore.getState().busy.qwen).toBe(false);
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      expect(bridge.restoreQwenVim).not.toHaveBeenCalled(); cleanup();
    }
    expect(bridge.previewQwenVimRestore).toHaveBeenCalledTimes(2);
  });
  it.each(['cancel', 'value', 'clear', 'preview', 'inspect'])('invalidates ready confirmation on %s', async kind => {
    render(<QwenSettings />); fill(); start(); await ready();
    if (kind === 'cancel') fireEvent.click(screen.getByRole('button', { name: '取消撤销' }));
    if (kind === 'value') fireEvent.click(screen.getByLabelText('启用 Vim 模式'));
    if (kind === 'clear') fireEvent.click(screen.getByRole('button', { name: '清除路径与结果' }));
    if (kind === 'preview') { fireEvent.click(screen.getByRole('button', { name: '预览 Vim 变更' })); await screen.findByRole('alert'); }
    if (kind === 'inspect') { fireEvent.submit(screen.getByRole('form', { name: 'Qwen 检查路径' })); await screen.findByRole('alert'); }
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
  });
  it.each([false, true])('discards late restore result after unmount (failure=%s), retaining owner', async failure => {
    const work = defer<boolean>(); vi.mocked(bridge.restoreQwenVim).mockReturnValue(work.promise);
    const view = render(<QwenSettings />); fill(); start(); await ready();
    fireEvent.click(screen.getByRole('button', { name: '确认撤销 Vim 设置' })); view.unmount();
    render(<QwenSettings />); fill(); expect(screen.getByRole('button', { name: '撤销受管 Vim 设置' })).toBeDisabled();
    await act(async () => { if (failure) work.reject(new Error('SECRET')); else work.resolve(true); });
    expect(useWorkspaceActivityStore.getState().busy.qwen).toBe(false);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByText('受管字段已撤销。尚未验证 Qwen 实际运行效果。')).not.toBeInTheDocument();
    expect(bridge.restoreQwenVim).toHaveBeenCalledTimes(1);
  });
  it('invalidates ready confirmation when another workspace owner starts', async () => {
    render(<QwenSettings />); fill(); start(); await ready();
    const owner = Symbol('external');
    act(() => useWorkspaceActivityStore.getState().setActivity('qwen', owner, true));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    act(() => useWorkspaceActivityStore.getState().setActivity('qwen', owner, false));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
  });
  it('rejects preview failure without authorizing restore or revealing errors', async () => {
    vi.mocked(bridge.previewQwenVimRestore).mockRejectedValue(new Error('SECRET'));
    render(<QwenSettings />); fill(); start(); expect(await screen.findByRole('alert')).toHaveTextContent('无法生成兼容预览');
    expect(screen.getByRole('alert')).not.toHaveTextContent('SECRET'); expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
  });
  it('never silently refreshes or retries a rejected checked restore', async () => {
    vi.mocked(bridge.restoreQwenVim).mockRejectedValue(new Error('SECRET stale'));
    render(<QwenSettings />); fill(); start(); await ready(); fireEvent.click(screen.getByRole('button', { name: '确认撤销 Vim 设置' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('重新预览'); expect(screen.getByRole('alert')).not.toHaveTextContent('SECRET');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.previewQwenVimRestore).toHaveBeenCalledTimes(1); expect(bridge.restoreQwenVim).toHaveBeenCalledTimes(1);
  });
});
