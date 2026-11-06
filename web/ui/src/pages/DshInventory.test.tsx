import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import * as bridge from '../bridge';
import { dshDemoReport } from '../dshContract';
import type { DshIssue } from '../dshContract';
import { DshInventory } from './DshInventory';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';

const result = () => ({ report: dshDemoReport(false), synthetic: false });
beforeEach(() => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'inspectDshProfile').mockResolvedValue(result());
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
function fillProfile(value = '/explicit/profile') {
  fireEvent.change(screen.getByLabelText('Profile 目录（绝对路径）'), { target: { value } });
}
function submit() { fireEvent.submit(screen.getByRole('form', { name: 'DSH 盘点路径' })); }
function deferred() {
  let resolve!: (value: bridge.DshInspectionResult) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<bridge.DshInspectionResult>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

describe('DSH inventory workspace', () => {
  it('never scans on mount or input changes, and exposes no lifecycle action', async () => {
    render(<StrictMode><DshInventory /></StrictMode>);
    expect(bridge.inspectDshProfile).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: '盘点所选声明' })).toBeDisabled();
    fillProfile();
    expect(bridge.inspectDshProfile).not.toHaveBeenCalled();
    submit();
    await screen.findByRole('region', { name: 'DSH 声明盘点结果' });
    expect(bridge.inspectDshProfile).toHaveBeenCalledExactlyOnceWith('/explicit/profile', null);
    expect(screen.getAllByRole('button')).toHaveLength(1);
    expect(screen.getByText('全部关闭')).toBeInTheDocument();
    expect(screen.getAllByText('未知')).toHaveLength(3);
    expect(screen.getByText('未知 / 未知')).toBeInTheDocument();
    expect(screen.queryByText('已安装')).not.toBeInTheDocument();
  });
  it('passes a separately selected runtime path without automatic discovery', async () => {
    vi.mocked(bridge.inspectDshProfile).mockResolvedValue({ report: dshDemoReport(true), synthetic: false });
    render(<DshInventory />);
    fillProfile();
    fireEvent.change(screen.getByLabelText('Runtime package.json（可选，绝对路径）'), { target: { value: '/runtime/package.json' } });
    submit();
    await screen.findByText('999.0.0-demo');
    expect(bridge.inspectDshProfile).toHaveBeenCalledExactlyOnceWith('/explicit/profile', '/runtime/package.json');
    expect(screen.getByText(/不是实测版本/)).toBeInTheDocument();
  });
  it('blocks duplicate submits and discards results after a path edit', async () => {
    const pending = deferred();
    vi.mocked(bridge.inspectDshProfile).mockReturnValue(pending.promise);
    render(<DshInventory />);
    fillProfile(); submit(); submit();
    expect(bridge.inspectDshProfile).toHaveBeenCalledTimes(1);
    expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(true);
    fillProfile('/changed/profile');
    expect(screen.getByText(/路径已更改，旧结果已清除/)).toBeInTheDocument();
    await act(async () => { pending.resolve(result()); await pending.promise; });
    expect(screen.queryByRole('region', { name: 'DSH 声明盘点结果' })).not.toBeInTheDocument();
    expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(false);
    vi.mocked(bridge.inspectDshProfile).mockResolvedValue(result());
    submit();
    await screen.findByRole('region', { name: 'DSH 声明盘点结果' });
    expect(bridge.inspectDshProfile).toHaveBeenLastCalledWith('/changed/profile', null);
  });
  it('clears a completed report when either selected path changes', async () => {
    render(<DshInventory />); fillProfile(); submit();
    await screen.findByRole('region', { name: 'DSH 声明盘点结果' });
    fireEvent.change(screen.getByLabelText('Runtime package.json（可选，绝对路径）'), { target: { value: '/runtime/package.json' } });
    expect(screen.queryByRole('region', { name: 'DSH 声明盘点结果' })).not.toBeInTheDocument();
    expect(bridge.inspectDshProfile).toHaveBeenCalledTimes(1);
  });
  it('sanitizes backend failures and allows an explicit retry', async () => {
    vi.mocked(bridge.inspectDshProfile).mockRejectedValueOnce(new Error('PRIVATE_PATH_OR_SECRET'));
    render(<DshInventory />); fillProfile(); submit();
    await screen.findByRole('alert');
    expect(screen.queryByText(/PRIVATE_PATH_OR_SECRET/)).not.toBeInTheDocument();
    submit();
    await screen.findByRole('region', { name: 'DSH 声明盘点结果' });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
  it('does not show an obsolete failure after editing a path', async () => {
    const pending = deferred();
    vi.mocked(bridge.inspectDshProfile).mockReturnValue(pending.promise);
    render(<DshInventory />); fillProfile(); submit(); fillProfile('/changed/profile');
    await act(async () => { pending.reject(new Error('old failure')); await pending.promise.catch(() => undefined); });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
  it('removes its activity owner on unmount and ignores late completion', async () => {
    const pending = deferred();
    vi.mocked(bridge.inspectDshProfile).mockReturnValue(pending.promise);
    const view = render(<StrictMode><DshInventory /></StrictMode>); fillProfile(); submit();
    view.unmount();
    expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(false);
    await act(async () => { pending.resolve(result()); await pending.promise; });
    expect(screen.queryByRole('region', { name: 'DSH 声明盘点结果' })).not.toBeInTheDocument();
  });
  it.each(['missing', 'corrupt', 'unreadable', 'unsupported'] as const)('keeps %s separate from installed state', async (status) => {
    const next = result();
    const issues: Record<typeof status, DshIssue> = { missing: 'missing', corrupt: 'invalid_json', unreadable: 'permission_denied', unsupported: 'unsupported_shape' };
    next.report.profile_manifest = { status, issue: issues[status], metadata: null };
    vi.mocked(bridge.inspectDshProfile).mockResolvedValue(next);
    render(<DshInventory />); fillProfile(); submit();
    await screen.findByRole('region', { name: 'DSH 声明盘点结果' });
    expect(screen.queryByText('synthetic-demo-profile')).not.toBeInTheDocument();
    expect(screen.queryByText('已安装')).not.toBeInTheDocument();
  });
  it('disables local inspection in an ordinary browser', () => {
    vi.mocked(bridge.runtimeMode).mockReturnValue('unavailable');
    render(<DshInventory />);
    expect(screen.getByRole('button', { name: '盘点所选声明' })).toBeDisabled();
    expect(screen.getByLabelText('Profile 目录（绝对路径）')).toBeDisabled();
    submit();
    expect(bridge.inspectDshProfile).not.toHaveBeenCalled();
  });
  it('labels synthetic demo input and output visibly', async () => {
    vi.mocked(bridge.runtimeMode).mockReturnValue('demo');
    vi.mocked(bridge.inspectDshProfile).mockResolvedValue({ report: dshDemoReport(false), synthetic: true });
    render(<DshInventory />);
    expect(screen.getByText(/合成示例模式/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看合成示例' }));
    await waitFor(() => expect(screen.getByText('合成示例结果，不代表本机状态。')).toBeInTheDocument());
  });
});
