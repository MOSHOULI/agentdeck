import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { UpdatePanel } from './UpdatePanel';
import type { UpdateSummary } from '../bridge';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';

vi.mock('../bridge', () => ({
  runtimeMode: vi.fn(),
  checkUpdates: vi.fn(),
  applyUpdates: vi.fn(),
  getAppVersion: vi.fn(),
}));

import * as bridge from '../bridge';

const summary: UpdateSummary = {
  version: '1.1.0', released: '2026-08-11', files: ['registry-v3.json'], pubkeyFp: 'dc83b35677939e36',
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

describe('UpdatePanel', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(bridge.runtimeMode).mockReturnValue('native');
    vi.mocked(bridge.getAppVersion).mockResolvedValue('v1.0.0');
    vi.mocked(bridge.checkUpdates).mockResolvedValue(summary);
    vi.mocked(bridge.applyUpdates).mockResolvedValue(['registry-v3.json']);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('启动与隐藏工作区不自动联网、不占用全局活动，也不声称已检查', async () => {
    const { rerender } = render(<div hidden={false}><UpdatePanel /></div>);
    expect(screen.getByText('尚未检查注册表更新。')).toBeInTheDocument();
    expect(screen.getByText(/启动时不自动联网检查/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '检查更新' })).toBeEnabled();
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(false);
    await act(async () => {});
    rerender(<div hidden><UpdatePanel /></div>);
    rerender(<div hidden={false}><UpdatePanel /></div>);
    expect(bridge.checkUpdates).not.toHaveBeenCalled();
    expect(bridge.applyUpdates).not.toHaveBeenCalled();
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(false);
    expect(screen.queryByRole('button', { name: '应用更新' })).not.toBeInTheDocument();
  });

  it('检查成功且无可应用更新时如实报告，并区分应用版本', async () => {
    vi.mocked(bridge.checkUpdates).mockResolvedValue(null);
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    expect(await screen.findByText('暂无可应用的注册表更新。')).toBeInTheDocument();
    expect(screen.queryByText('已是最新注册表。')).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveClass('notice-success');
    expect(screen.getByText('桌面应用版本')).toBeInTheDocument();
    expect(screen.getByText('v1.0.0')).toBeInTheDocument();
    expect(screen.getByText(/此处仅更新注册表数据，不会升级 AgentDeck/)).toBeInTheDocument();
  });

  it('展示可验证的更新详情并调用真实应用接口', async () => {
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    expect(await screen.findByRole('heading', { name: '发现新版本 1.1.0' })).toBeInTheDocument();
    expect(screen.getByText('dc83b35677939e36')).toBeInTheDocument();
    expect(screen.getByText('registry-v3.json')).toBeInTheDocument();
    expect(screen.getByText('注册表发布于 2026-08-11')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '应用更新' }));
    expect(await screen.findByText('已更新：registry-v3.json')).toBeInTheDocument();
    expect(bridge.applyUpdates).toHaveBeenCalledTimes(1);
    expect(screen.getByText('v1.0.0')).toBeInTheDocument();
    expect(screen.getByText(/桌面应用版本保持不变/)).toBeInTheDocument();
  });

  it('检查失败保留具体签名错误，不伪装为已是最新', async () => {
    vi.mocked(bridge.checkUpdates).mockRejectedValue(new Error('签名校验失败'));
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    const error = await screen.findByRole('alert');
    expect(error).toHaveTextContent('检查更新失败');
    expect(error).toHaveTextContent('签名校验失败');
    expect(error).toHaveClass('notice-error');
    expect(error).not.toHaveClass('notice-success');
    expect(screen.queryByText('已是最新注册表。')).not.toBeInTheDocument();
    expect(screen.getByText('尚无记录')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '应用更新' })).not.toBeInTheDocument();
  });

  it('应用中的同步锁阻止重复点击和检查，隐藏工作区不会解除锁', async () => {
    const pending = deferred<string[]>();
    vi.mocked(bridge.applyUpdates).mockReturnValue(pending.promise);
    const { rerender } = render(<div hidden={false}><UpdatePanel /></div>);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    const apply = await screen.findByRole('button', { name: '应用更新' });
    fireEvent.click(apply);
    fireEvent.click(apply);
    expect(bridge.applyUpdates).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: '检查更新' })).toBeDisabled();
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(true);
    rerender(<div hidden><UpdatePanel /></div>);
    fireEvent.click(apply);
    expect(bridge.applyUpdates).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve(['registry-v3.json']));
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(false);
    expect(screen.getByText('已更新：registry-v3.json')).toBeInTheDocument();
  });

  it('检查期间阻止重复检查并显示活动状态', async () => {
    const pending = deferred<UpdateSummary | null>();
    vi.mocked(bridge.checkUpdates).mockReturnValue(pending.promise);
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    const check = screen.getByRole('button', { name: '检查中…' });
    expect(check).toBeDisabled();
    fireEvent.click(check);
    fireEvent.click(check);
    expect(bridge.checkUpdates).toHaveBeenCalledTimes(1);
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(true);
    await act(async () => pending.resolve(null));
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(false);
    expect(screen.getByRole('button', { name: '检查更新' })).toBeEnabled();
  });

  it('显式网络检查隐藏后仍占用活动，失败真正结算后才释放', async () => {
    const pending = deferred<UpdateSummary | null>();
    vi.mocked(bridge.checkUpdates).mockReturnValue(pending.promise);
    const { rerender } = render(<div hidden={false}><UpdatePanel /></div>);
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    rerender(<div hidden><UpdatePanel /></div>);
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(true);
    expect(bridge.checkUpdates).toHaveBeenCalledTimes(1);
    await act(async () => pending.reject(new Error('offline')));
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(false);
    rerender(<div hidden={false}><UpdatePanel /></div>);
    expect(screen.getByRole('alert')).toHaveTextContent('检查更新失败');
  });

  it('手动检查同样阻止快速重复触发', async () => {
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    await screen.findByRole('button', { name: '应用更新' });
    const pending = deferred<UpdateSummary | null>();
    vi.mocked(bridge.checkUpdates).mockReturnValue(pending.promise);
    const check = screen.getByRole('button', { name: '检查更新' });
    fireEvent.click(check);
    fireEvent.click(check);
    expect(bridge.checkUpdates).toHaveBeenCalledTimes(2);
    await act(async () => pending.resolve(summary));
  });

  it('应用失败展示校验错误且解锁重新检查', async () => {
    vi.mocked(bridge.applyUpdates).mockRejectedValue(new Error('文件 SHA-256 校验失败'));
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    fireEvent.click(await screen.findByRole('button', { name: '应用更新' }));
    const error = await screen.findByRole('alert');
    expect(error).toHaveTextContent('应用更新失败');
    expect(error).toHaveTextContent('SHA-256 校验失败');
    expect(screen.queryByText('注册表已应用')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '检查更新' })).toBeEnabled();
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    await screen.findByRole('button', { name: '应用更新' });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(bridge.checkUpdates).toHaveBeenCalledTimes(2);
  });

  it('应用版本读取失败仍明确显示未知，不用注册表版本替代', async () => {
    vi.mocked(bridge.getAppVersion).mockRejectedValue(new Error('health unavailable'));
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    await screen.findByRole('button', { name: '应用更新' });
    expect(screen.getByText('未知')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '应用更新' }));
    await screen.findByText('注册表已应用');
    expect(screen.getByText('未知')).toBeInTheDocument();
  });

  it('空应用版本不会显示为一个已知版本', async () => {
    vi.mocked(bridge.getAppVersion).mockResolvedValue(' ');
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    await screen.findByRole('button', { name: '应用更新' });
    expect(screen.getByText('未知')).toBeInTheDocument();
  });

  it.each(['demo', 'unavailable'] as const)('在 %s 模式不假装已检查成功，也不调用写入接口', async (mode) => {
    vi.mocked(bridge.runtimeMode).mockReturnValue(mode);
    render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    expect(screen.getByRole('button', { name: '检查更新' })).toBeDisabled();
    expect(screen.getByText(/当前注册表是否最新尚未验证/)).toBeInTheDocument();
    expect(screen.getByText('未知')).toBeInTheDocument();
    expect(screen.queryByText('已是最新注册表。')).not.toBeInTheDocument();
    expect(bridge.checkUpdates).not.toHaveBeenCalled();
    expect(bridge.applyUpdates).not.toHaveBeenCalled();
    expect(bridge.getAppVersion).not.toHaveBeenCalled();
  });

  it('卸载工作区后清除活动标记', async () => {
    const pending = deferred<UpdateSummary | null>();
    vi.mocked(bridge.checkUpdates).mockReturnValue(pending.promise);
    const { unmount } = render(<UpdatePanel />);
    fireEvent.click(screen.getByRole('button', { name: '检查更新' }));
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(true);
    unmount();
    expect(useWorkspaceActivityStore.getState().busy.updates).toBe(false);
    await act(async () => pending.reject(new Error('offline')));
  });
});
