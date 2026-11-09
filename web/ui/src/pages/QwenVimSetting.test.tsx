import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import { qwenDemoSummary, type QwenRecovery } from '../qwenContract';
import { QwenSettings } from './QwenSettings';
const token = 'a'.repeat(64);
const preview = { field: 'general.vimMode' as const, before: null, after: false, changes: true, token };
beforeEach(() => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'inspectQwenSettings');
  vi.spyOn(bridge, 'previewQwenVim').mockResolvedValue(preview);
  vi.spyOn(bridge, 'applyQwenVim').mockResolvedValue(true);
  vi.spyOn(bridge, 'inspectQwenVimRecovery').mockResolvedValue({ field: 'general.vimMode', state: 'no_receipt', changesSettings: false });
  vi.spyOn(bridge, 'restoreQwenVim').mockResolvedValue(true);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
function fill() {
  fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/selected/settings.json' } });
  fireEvent.change(screen.getByLabelText('运行时 package.json 绝对路径'), { target: { value: '/runtime/package.json' } });
}
function defer<T>() { let resolve!: (value: T) => void; let reject!: (value: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
async function showPreview() { fireEvent.click(screen.getByRole('button', { name: '预览 Vim 变更' })); await screen.findByRole('dialog', { name: '确认 Vim 变更' }); }
describe('Qwen single field management', () => {
  it('does not write until explicit confirmation, and cancel never writes', async () => {
    render(<QwenSettings />); fill(); await showPreview();
    expect(bridge.applyQwenVim).not.toHaveBeenCalled();
    expect(screen.queryByText(token)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '取消 Vim 变更' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(bridge.applyQwenVim).not.toHaveBeenCalled();
    await showPreview(); fireEvent.click(screen.getByRole('button', { name: '确认应用 Vim 变更' }));
    await screen.findByText('所选文件的 Vim 模式字段已应用。尚未验证 Qwen 实际运行效果。');
    expect(bridge.applyQwenVim).toHaveBeenCalledExactlyOnceWith('/selected/settings.json', '/runtime/package.json', false, token);
  });
  it('locks paths and blocks duplicate apply while mutation is in flight', async () => {
    const work = defer<boolean>(); vi.mocked(bridge.applyQwenVim).mockReturnValue(work.promise);
    render(<QwenSettings />); fill(); await showPreview();
    const button = screen.getByRole('button', { name: '确认应用 Vim 变更' });
    fireEvent.click(button); fireEvent.click(button);
    expect(bridge.applyQwenVim).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText('settings.json 绝对路径')).toBeDisabled();
    expect(screen.getByLabelText('运行时 package.json 绝对路径')).toBeDisabled();
    expect(screen.getByRole('button', { name: '清除路径与结果' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '检查所选文件' })).toBeDisabled();
    expect(screen.getByLabelText('启用 Vim 模式')).toBeDisabled();
    await act(async () => { work.resolve(true); });
    expect(screen.getByLabelText('settings.json 绝对路径')).toBeEnabled();
  });
  it('discards pending preview after path change and blocks duplicate preview', async () => {
    const work = defer<typeof preview>(); vi.mocked(bridge.previewQwenVim).mockReturnValue(work.promise);
    render(<QwenSettings />); fill();
    fireEvent.click(screen.getByRole('button', { name: '预览 Vim 变更' }));
    fireEvent.click(screen.getByRole('button', { name: '预览 Vim 变更' }));
    fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/other/settings.json' } });
    await act(async () => { work.resolve(preview); });
    expect(bridge.previewQwenVim).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(bridge.applyQwenVim).not.toHaveBeenCalled();
  });
  it('invalidates ready preview when value changes or paths are cleared', async () => {
    render(<QwenSettings />); fill(); await showPreview();
    fireEvent.click(screen.getByLabelText('启用 Vim 模式'));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    await showPreview(); fireEvent.click(screen.getByRole('button', { name: '清除路径与结果' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(bridge.applyQwenVim).not.toHaveBeenCalled();
  });
  it('requires separate restore confirmation and cancellation does nothing', async () => {
    const work = defer<boolean>(); vi.mocked(bridge.restoreQwenVim).mockReturnValue(work.promise);
    render(<QwenSettings />); fill();
    fireEvent.click(screen.getByRole('button', { name: '撤销受管 Vim 设置' }));
    expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '取消撤销' }));
    fireEvent.click(screen.getByRole('button', { name: '撤销受管 Vim 设置' }));
    const confirm = screen.getByRole('button', { name: '确认撤销 Vim 设置' });
    fireEvent.click(confirm); fireEvent.click(confirm);
    expect(bridge.restoreQwenVim).toHaveBeenCalledExactlyOnceWith('/selected/settings.json', '/runtime/package.json');
    expect(screen.getByRole('button', { name: '清除路径与结果' })).toBeDisabled();
    await act(async () => { work.resolve(true); });
    await screen.findByText('受管字段已撤销。尚未验证 Qwen 实际运行效果。');
  });
  it('does not present success or raw exceptions on ambiguous mutation errors', async () => {
    vi.mocked(bridge.applyQwenVim).mockRejectedValue(new Error('SECRET /private/settings.json'));
    render(<QwenSettings />); fill(); await showPreview(); fireEvent.click(screen.getByRole('button', { name: '确认应用 Vim 变更' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('不要假定配置未改变');
    expect(screen.getByRole('alert')).not.toHaveTextContent('SECRET');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
  it('keeps no-op apply disabled', async () => {
    vi.mocked(bridge.previewQwenVim).mockResolvedValue({ ...preview, before: false, changes: false });
    render(<QwenSettings />); fill(); await showPreview();
    expect(screen.getByRole('button', { name: '确认应用 Vim 变更' })).toBeDisabled();
  });
  it('demo and unavailable modes never dispatch management', () => {
    for (const mode of ['demo', 'unavailable'] as const) {
      vi.mocked(bridge.runtimeMode).mockReturnValue(mode); const view = render(<QwenSettings />);
      if (mode === 'demo') fill();
      expect(screen.getByRole('button', { name: '预览 Vim 变更' })).toBeDisabled();
      expect(screen.getByRole('button', { name: '撤销受管 Vim 设置' })).toBeDisabled(); view.unmount();
    }
    expect(bridge.previewQwenVim).not.toHaveBeenCalled(); expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
  });
  it.each(['apply', 'restore'] as const)('clears stale inspection before %s even if mutation fails', async action => {
    vi.mocked(bridge.inspectQwenSettings).mockResolvedValue({ summary: qwenDemoSummary(), synthetic: false });
    vi.mocked(bridge.applyQwenVim).mockRejectedValue(new Error('SECRET'));
    vi.mocked(bridge.restoreQwenVim).mockRejectedValue(new Error('SECRET'));
    render(<QwenSettings />); fill();
    fireEvent.submit(screen.getByRole('form', { name: 'Qwen 检查路径' }));
    await screen.findByRole('region', { name: 'Qwen 检查结果' });
    if (action === 'apply') { await showPreview(); fireEvent.click(screen.getByRole('button', { name: '确认应用 Vim 变更' })); }
    else { fireEvent.click(screen.getByRole('button', { name: '撤销受管 Vim 设置' })); fireEvent.click(screen.getByRole('button', { name: '确认撤销 Vim 设置' })); }
    await screen.findByRole('alert');
    expect(screen.queryByRole('region', { name: 'Qwen 检查结果' })).not.toBeInTheDocument();
  });
  it('unmounted preview cannot return into a new view', async () => {
    const work = defer<typeof preview>(); vi.mocked(bridge.previewQwenVim).mockReturnValue(work.promise);
    const view = render(<QwenSettings />); fill(); fireEvent.click(screen.getByRole('button', { name: '预览 Vim 变更' })); view.unmount();
    await act(async () => { work.resolve(preview); }); render(<QwenSettings />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});

describe('Qwen recovery observation UI', () => {
  const observe = () => fireEvent.click(screen.getByRole('button', { name: '只读检查 Vim 恢复状态' }));
  const report: QwenRecovery = { field: 'general.vimMode', state: 'restore_field', changesSettings: true };
  it.each(['no_receipt', 'restore_field', 'cleanup_receipt', 'conflict'] as const)('shows %s as observation, never mutation authorization', async state => {
    vi.mocked(bridge.inspectQwenVimRecovery).mockResolvedValue({ ...report, state, changesSettings: state === 'restore_field' });
    render(<QwenSettings />); fill(); observe();
    const region = await screen.findByRole('region', { name: 'Vim 恢复状态观察' });
    expect(region).toHaveTextContent('不是恢复授权');
    expect(region).toHaveTextContent('后端会再次检查');
    expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
    expect(bridge.applyQwenVim).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '撤销受管 Vim 设置' }));
    expect(screen.queryByRole('region', { name: 'Vim 恢复状态观察' })).not.toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: '确认撤销 Vim 设置' })).toBeInTheDocument();
    expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
  });
  it.each(['cancel', 'path', 'clear', 'unmount'])('discards late observation after %s and blocks duplicate dispatch', async kind => {
    const work = defer<QwenRecovery>(); vi.mocked(bridge.inspectQwenVimRecovery).mockReturnValue(work.promise);
    const view = render(<QwenSettings />); fill(); observe(); observe();
    expect(bridge.inspectQwenVimRecovery).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText('settings.json 绝对路径')).toBeEnabled();
    expect(screen.getByRole('button', { name: '预览 Vim 变更' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '撤销受管 Vim 设置' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '检查所选文件' })).toBeDisabled();
    if (kind === 'cancel') fireEvent.click(screen.getByRole('button', { name: '取消恢复诊断结果' }));
    if (kind === 'path') fireEvent.change(screen.getByLabelText('运行时 package.json 绝对路径'), { target: { value: '/other/package.json' } });
    if (kind === 'clear') fireEvent.click(screen.getByRole('button', { name: '清除路径与结果' }));
    if (kind === 'unmount') view.unmount();
    await act(async () => { work.resolve(report); });
    if (kind === 'unmount') render(<QwenSettings />);
    expect(screen.queryByRole('region', { name: 'Vim 恢复状态观察' })).not.toBeInTheDocument();
    expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
  });
  it('diagnosis invalidates preview and restore confirmation, and preview invalidates diagnosis', async () => {
    render(<QwenSettings />); fill(); await showPreview(); observe();
    await screen.findByRole('region', { name: 'Vim 恢复状态观察' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    await showPreview();
    expect(screen.queryByRole('region', { name: 'Vim 恢复状态观察' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '撤销受管 Vim 设置' })); observe();
    await screen.findByRole('region', { name: 'Vim 恢复状态观察' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '清除恢复诊断结果' }));
    expect(screen.queryByRole('region', { name: 'Vim 恢复状态观察' })).not.toBeInTheDocument();
  });
  it('reports fixed errors without treating failed observation as no receipt', async () => {
    vi.mocked(bridge.inspectQwenVimRecovery).mockRejectedValue(new Error('SECRET'));
    render(<QwenSettings />); fill(); observe();
    expect(await screen.findByRole('alert')).toHaveTextContent('未获得有效观察结果');
    expect(screen.getByRole('alert')).not.toHaveTextContent('SECRET');
    expect(screen.queryByRole('region', { name: 'Vim 恢复状态观察' })).not.toBeInTheDocument();
  });
  it.each(['demo', 'unavailable'] as const)('never observes in %s', mode => {
    vi.mocked(bridge.runtimeMode).mockReturnValue(mode); render(<QwenSettings />);
    if (mode === 'demo') fill();
    expect(screen.getByRole('button', { name: '只读检查 Vim 恢复状态' })).toBeDisabled(); observe();
    expect(bridge.inspectQwenVimRecovery).not.toHaveBeenCalled();
  });
});
