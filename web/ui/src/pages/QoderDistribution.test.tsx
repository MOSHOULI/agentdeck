import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import { QoderDistribution } from './QoderDistribution';
import { qoderTargets, validateQoderPlan } from '../qoderContract';
import { qoderRawPlan } from '../test-fixtures/qoder';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
const plan = validateQoderPlan(qoderRawPlan(), 'linux-amd64-baseline');
const result = { plan, bytes: 123 };
beforeEach(() => { vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native'); vi.spyOn(bridge, 'qoderDistributionPlan').mockResolvedValue(plan); vi.spyOn(bridge, 'verifyQoderPackage').mockResolvedValue(result); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
function fill() { fireEvent.click(screen.getByRole('radio', { name: plan.target })); fireEvent.change(screen.getByLabelText('Qoder 官方归档绝对路径'), { target: { value: '/selected/archive' } }); }
function verify() { fireEvent.click(screen.getByRole('button', { name: '只读校验 Qoder 归档' })); }
function defer<T>() { let resolve!: (v: T) => void; let reject!: (v: Error) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { resolve, reject, promise }; }
it('requires explicit target and renders all 11 variants with no install actions', async () => {
  render(<QoderDistribution />); expect(screen.getAllByRole('radio')).toHaveLength(11);
  expect(screen.getByRole('button', { name: '查看 Qoder 分发计划' })).toBeDisabled();
  fill(); fireEvent.click(screen.getByRole('button', { name: '查看 Qoder 分发计划' }));
  expect(await screen.findByRole('region', { name: 'Qoder 分发计划结果' })).toHaveTextContent('未安装、未运行');
  expect(bridge.qoderDistributionPlan).toHaveBeenCalledExactlyOnceWith(plan.target); expect(bridge.verifyQoderPackage).not.toHaveBeenCalled();
  verify(); expect(await screen.findByRole('region', { name: 'Qoder 归档校验结果' })).toHaveTextContent('这不是安装成功');
  expect(bridge.verifyQoderPackage).toHaveBeenCalledExactlyOnceWith(plan.target, '/selected/archive');
});
it.each(['target', 'path', 'clear', 'cancel', 'unmount'])('discards late success after %s and releases busy safely', async change => {
  const work = defer<typeof result>(); vi.mocked(bridge.verifyQoderPackage).mockReturnValue(work.promise);
  const view = render(<QoderDistribution />); fill(); verify(); verify();
  expect(bridge.verifyQoderPackage).toHaveBeenCalledTimes(1); expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
  expect(screen.getByRole('button', { name: '查看 Qoder 分发计划' })).toBeDisabled();
  if (change === 'target') fireEvent.click(screen.getByRole('radio', { name: 'windows-amd64-standard' }));
  if (change === 'path') fireEvent.change(screen.getByLabelText('Qoder 官方归档绝对路径'), { target: { value: '/other/archive' } });
  if (change === 'clear') fireEvent.click(screen.getByRole('button', { name: '清除 Qoder 选择与结果' }));
  if (change === 'cancel') fireEvent.click(screen.getByRole('button', { name: '取消 Qoder 结果' }));
  if (change === 'unmount') view.unmount();
  await act(async () => { work.resolve(result); });
  expect(screen.queryByRole('region', { name: 'Qoder 归档校验结果' })).not.toBeInTheDocument(); expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false);
});
it('suppresses late failure after cancel; a new request can succeed', async () => {
  const work = defer<typeof result>(); vi.mocked(bridge.verifyQoderPackage).mockReturnValueOnce(work.promise);
  render(<QoderDistribution />); fill(); verify(); fireEvent.click(screen.getByRole('button', { name: '取消 Qoder 结果' }));
  await act(async () => { work.reject(new Error('SECRET')); }); expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  verify(); await screen.findByRole('region', { name: 'Qoder 归档校验结果' });
});
it('shows a fixed failure, never raw paths/errors or stale success', async () => {
  render(<QoderDistribution />); fill(); verify(); await screen.findByRole('region', { name: 'Qoder 归档校验结果' });
  vi.mocked(bridge.verifyQoderPackage).mockRejectedValue(new Error('SECRET /private'));
  verify(); expect(await screen.findByRole('alert')).not.toHaveTextContent('SECRET'); expect(screen.queryByRole('region', { name: 'Qoder 归档校验结果' })).not.toBeInTheDocument();
});
it('changing inputs clears an already completed verification', async () => {
  render(<QoderDistribution />); fill(); verify(); await screen.findByRole('region', { name: 'Qoder 归档校验结果' });
  fireEvent.change(screen.getByLabelText('Qoder 官方归档绝对路径'), { target: { value: '/changed' } }); expect(screen.queryByRole('region', { name: 'Qoder 归档校验结果' })).not.toBeInTheDocument();
});
it.each(['demo', 'unavailable'] as const)('does not invoke IPC in %s', mode => {
  vi.mocked(bridge.runtimeMode).mockReturnValue(mode); render(<QoderDistribution />);
  expect(screen.getByRole('button', { name: '查看 Qoder 分发计划' })).toBeDisabled(); verify(); expect(bridge.verifyQoderPackage).not.toHaveBeenCalled(); expect(bridge.qoderDistributionPlan).not.toHaveBeenCalled();
});

it('renders selected identity and archive name as complete accessible text nodes', async () => {
  render(<QoderDistribution />);
  expect(screen.getByText('当前分发目标：未选择').childNodes).toHaveLength(1);
  fill();
  expect(screen.getByText('当前分发目标：linux-amd64-baseline').childNodes).toHaveLength(1);
  fireEvent.click(screen.getByRole('button', { name: '查看 Qoder 分发计划' }));
  const name = await screen.findByText('官方归档名：qodercli-linux-x64-baseline.tar.gz');
  expect(name.childNodes).toHaveLength(1);
  expect(name.firstChild?.nodeType).toBe(Node.TEXT_NODE);
});

it('renders a successful byte count as one complete accessible text node', async () => {
  render(<QoderDistribution />); fill(); verify();
  const bytes = await screen.findByText('已读取字节：123');
  expect(bytes.childNodes).toHaveLength(1);
  expect(bytes.firstChild?.nodeType).toBe(Node.TEXT_NODE);
});

it('offers a labelled group, exclusive target choices and complete clear', () => {
  render(<QoderDistribution />);
  expect(screen.getByRole('group', { name: '明确分发目标（不是本机自动检测）' })).toBeInTheDocument();
  for (const target of qoderTargets) {
    fireEvent.click(screen.getByRole('radio', { name: target }));
    for (const other of qoderTargets) expect((screen.getByRole('radio', { name: other }) as HTMLInputElement).checked).toBe(other === target);
    expect(screen.getByText(`当前分发目标：${target}`)).toBeInTheDocument();
  }
  fireEvent.click(screen.getByRole('button', { name: '清除 Qoder 选择与结果' }));
  for (const radio of screen.getAllByRole('radio')) expect(radio).not.toBeChecked();
  expect(screen.getByText('当前分发目标：未选择')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '查看 Qoder 分发计划' })).toBeDisabled();
});
it.each(['demo', 'unavailable'] as const)('disables all radios in %s', mode => {
  vi.mocked(bridge.runtimeMode).mockReturnValue(mode); render(<QoderDistribution />);
  for (const radio of screen.getAllByRole('radio')) { expect(radio).toBeDisabled(); expect(radio).not.toBeChecked(); }
});
