import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import { QwenSettings } from './QwenSettings';
const token = 'b'.repeat(64);
const preview = { field: 'general.vimMode' as const, before: true, after: false, changes: true, token };
beforeEach(() => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'previewQwenVim').mockResolvedValue(preview);
  vi.spyOn(bridge, 'applyQwenVim').mockResolvedValue(true);
  vi.spyOn(bridge, 'previewQwenVimUpdate').mockResolvedValue(preview);
  vi.spyOn(bridge, 'updateQwenVim').mockResolvedValue(true);
  vi.spyOn(bridge, 'restoreQwenVim').mockResolvedValue(true);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
function fill() {
  fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/selected/settings.json' } });
  fireEvent.change(screen.getByLabelText('运行时 package.json 绝对路径'), { target: { value: '/runtime/package.json' } });
}
function defer<T>() { let resolve!: (value: T) => void;
  const promise = new Promise<T>(yes => { resolve = yes; }); return { promise, resolve }; }
async function showUpdate() {
  fireEvent.click(screen.getByRole('button', { name: '预览更新受管 Vim 值' }));
  await screen.findByRole('dialog', { name: '确认更新受管 Vim 值' });
}
describe('Qwen owned value updates', () => {
  it('separates preview and confirmation, explaining initial restore value', async () => {
    render(<QwenSettings />); fill(); await showUpdate();
    expect(screen.getByText(/撤销始终恢复首次管理前的原值/)).toBeInTheDocument();
    expect(bridge.updateQwenVim).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '取消 Vim 变更' }));
    expect(bridge.updateQwenVim).not.toHaveBeenCalled();
    await showUpdate(); fireEvent.click(screen.getByRole('button', { name: '确认更新受管 Vim 值' }));
    await screen.findByText(/受管 Vim 值已更新/);
    expect(bridge.updateQwenVim).toHaveBeenCalledExactlyOnceWith('/selected/settings.json', '/runtime/package.json', false, token);
    expect(bridge.applyQwenVim).not.toHaveBeenCalled();
  });
  it('blocks duplicate update, paths and restore while IPC is pending', async () => {
    const work = defer<boolean>(); vi.mocked(bridge.updateQwenVim).mockReturnValue(work.promise);
    render(<QwenSettings />); fill(); await showUpdate();
    const button = screen.getByRole('button', { name: '确认更新受管 Vim 值' });
    fireEvent.click(button); fireEvent.click(button);
    expect(bridge.updateQwenVim).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText('settings.json 绝对路径')).toBeDisabled();
    expect(screen.getByRole('button', { name: '撤销受管 Vim 设置' })).toBeDisabled();
    await act(async () => work.resolve(true));
  });
  it('discards pending update preview after path change', async () => {
    const work = defer<typeof preview>(); vi.mocked(bridge.previewQwenVimUpdate).mockReturnValue(work.promise);
    render(<QwenSettings />); fill();
    fireEvent.click(screen.getByRole('button', { name: '预览更新受管 Vim 值' }));
    fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/other/settings.json' } });
    await act(async () => work.resolve(preview));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(bridge.updateQwenVim).not.toHaveBeenCalled();
  });
  it('invalidates update confirmation when target changes', async () => {
    render(<QwenSettings />); fill(); await showUpdate();
    fireEvent.click(screen.getByLabelText('启用 Vim 模式'));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(bridge.updateQwenVim).not.toHaveBeenCalled();
  });
  it('does not fallback to apply or restore after conflict or stale update', async () => {
    vi.mocked(bridge.updateQwenVim).mockRejectedValue(new Error('SECRET conflict'));
    render(<QwenSettings />); fill(); await showUpdate();
    fireEvent.click(screen.getByRole('button', { name: '确认更新受管 Vim 值' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('不要假定配置未改变');
    expect(screen.getByRole('alert')).not.toHaveTextContent('SECRET');
    expect(bridge.applyQwenVim).not.toHaveBeenCalled(); expect(bridge.restoreQwenVim).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
  it('does not fallback when no valid ownership preview exists', async () => {
    vi.mocked(bridge.previewQwenVimUpdate).mockRejectedValue(new Error('no receipt'));
    render(<QwenSettings />); fill();
    fireEvent.click(screen.getByRole('button', { name: '预览更新受管 Vim 值' }));
    await screen.findByRole('alert');
    expect(bridge.previewQwenVim).not.toHaveBeenCalled(); expect(bridge.updateQwenVim).not.toHaveBeenCalled();
  });
  it('keeps unchanged update disabled', async () => {
    vi.mocked(bridge.previewQwenVimUpdate).mockResolvedValue({ ...preview, before: false, changes: false });
    render(<QwenSettings />); fill(); await showUpdate();
    expect(screen.getByRole('button', { name: '确认更新受管 Vim 值' })).toBeDisabled();
  });
});
