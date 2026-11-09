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

const cacheResult = { state: 'incomplete' as const, recoveryPlan: 'preserve_and_review' as const, deletionAuthorized: false as const, manualReviewRequired: true as const, installed: false as const };
function fillCache() { fireEvent.change(screen.getByLabelText('Qoder 缓存绝对目录'), { target: { value: '/selected/cache' } }); }
function inspectCache() { fireEvent.click(screen.getByRole('button', { name: '只读诊断 Qoder 缓存' })); }
it('renders fixed preservation advice, no deletion action, and clears completed cache results', async () => {
  vi.spyOn(bridge, 'inspectQoderCache').mockResolvedValue(cacheResult);
  render(<QoderDistribution />); fillCache(); inspectCache();
  expect(await screen.findByRole('region', { name: 'Qoder 缓存诊断结果' })).toHaveTextContent('本报告不授权删除');
  expect(bridge.inspectQoderCache).toHaveBeenCalledExactlyOnceWith('/selected/cache');
  fill(); expect(screen.queryByRole('region', { name: 'Qoder 缓存诊断结果' })).not.toBeInTheDocument();
});
it.each(['path', 'target', 'clear', 'cancel', 'unmount'])('discards pending cache observation after %s', async action => {
  const work = defer<typeof cacheResult>(); vi.spyOn(bridge, 'inspectQoderCache').mockReturnValue(work.promise);
  const view = render(<QoderDistribution />); fill(); fillCache(); inspectCache(); inspectCache();
  expect(bridge.inspectQoderCache).toHaveBeenCalledTimes(1);
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
  expect(screen.getByRole('button', { name: '查看 Qoder 分发计划' })).toBeDisabled();
  if (action === 'path') fireEvent.change(screen.getByLabelText('Qoder 缓存绝对目录'), { target: { value: '/changed' } });
  if (action === 'target') fireEvent.click(screen.getByRole('radio', { name: 'windows-amd64-standard' }));
  if (action === 'clear') fireEvent.click(screen.getByRole('button', { name: '清除 Qoder 选择与结果' }));
  if (action === 'cancel') fireEvent.click(screen.getByRole('button', { name: '取消 Qoder 结果' }));
  if (action === 'unmount') view.unmount();
  await act(async () => { work.resolve(cacheResult); });
  expect(screen.queryByRole('region', { name: 'Qoder 缓存诊断结果' })).not.toBeInTheDocument();
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false);
});
it('reports unsupported hosts distinctly and never renders raw errors', async () => {
  vi.spyOn(bridge, 'inspectQoderCache').mockRejectedValueOnce('QODER_CACHE_UNSUPPORTED_HOST').mockRejectedValueOnce('SECRET');
  render(<QoderDistribution />); fillCache(); inspectCache();
  expect(await screen.findByRole('alert')).toHaveTextContent('当前主机不支持');
  inspectCache(); expect(await screen.findByRole('alert')).toHaveTextContent('没有获得完整性或清理授权');
  expect(screen.getByRole('alert')).not.toHaveTextContent('SECRET');
  expect(screen.queryByRole('region', { name: 'Qoder 缓存诊断结果' })).not.toBeInTheDocument();
});
it('coordinates cache action with other workspaces and with a pending package read', async () => {
  vi.spyOn(bridge, 'inspectQoderCache').mockResolvedValue(cacheResult);
  const owner = Symbol(); useWorkspaceActivityStore.getState().setActivity('qwen', owner, true);
  const work = defer<typeof result>(); vi.mocked(bridge.verifyQoderPackage).mockReturnValue(work.promise);
  render(<QoderDistribution />); fillCache(); inspectCache(); expect(bridge.inspectQoderCache).not.toHaveBeenCalled();
  act(() => useWorkspaceActivityStore.getState().setActivity('qwen', owner, false));
  fill(); verify(); expect(screen.getByRole('button', { name: '只读诊断 Qoder 缓存' })).toBeDisabled(); inspectCache();
  expect(bridge.inspectQoderCache).not.toHaveBeenCalled();
  await act(async () => { work.resolve(result); });
  inspectCache(); await screen.findByRole('region', { name: 'Qoder 缓存诊断结果' });
  expect(screen.queryByRole('region', { name: 'Qoder 归档校验结果' })).not.toBeInTheDocument();
});
it.each(['staged_archive_verified', 'unpacked_payload_verified', 'incomplete', 'conflict', 'unknown_ownership'] as const)('shows fixed safe cache state %s', async state => {
  vi.spyOn(bridge, 'inspectQoderCache').mockResolvedValue({ ...cacheResult, state, recoveryPlan: state.endsWith('_verified') ? 'retain_cache_no_action' : 'preserve_and_review' });
  render(<QoderDistribution />); fillCache(); inspectCache();
  expect(await screen.findByRole('region', { name: 'Qoder 缓存诊断结果' })).toHaveTextContent('任何清理均须独立人工复核');
  fireEvent.click(screen.getByRole('button', { name: '清除 Qoder 选择与结果' }));
  expect(screen.getByLabelText('Qoder 缓存绝对目录')).toHaveValue('');
  expect(screen.queryByRole('region', { name: 'Qoder 缓存诊断结果' })).not.toBeInTheDocument();
});
it('suppresses late cache failure after cancel and keeps busy until the reader finishes', async () => {
  const work = defer<typeof cacheResult>(); vi.spyOn(bridge, 'inspectQoderCache').mockReturnValue(work.promise);
  render(<QoderDistribution />); fillCache(); inspectCache();
  fireEvent.click(screen.getByRole('button', { name: '取消 Qoder 结果' }));
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
  expect(screen.getByRole('button', { name: '只读诊断 Qoder 缓存' })).toBeDisabled();
  await act(async () => { work.reject(new Error('SECRET')); });
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false);
});
it.each(['demo', 'unavailable'] as const)('does not dispatch cache inspection in %s UI', mode => {
  vi.mocked(bridge.runtimeMode).mockReturnValue(mode); vi.spyOn(bridge, 'inspectQoderCache');
  render(<QoderDistribution />);
  expect(screen.getByLabelText('Qoder 缓存绝对目录')).toBeDisabled();
  expect(screen.getByRole('button', { name: '只读诊断 Qoder 缓存' })).toBeDisabled();
  inspectCache(); expect(bridge.inspectQoderCache).not.toHaveBeenCalled();
});

function writeForm() {
  fireEvent.change(screen.getByLabelText('Qoder 缓存写入来源绝对路径'), { target: { value: '/selected/source' } });
  fireEvent.change(screen.getByLabelText('Qoder 全新缓存绝对目录'), { target: { value: '/selected/new-cache' } });
  fireEvent.click(screen.getByRole('button', { name: '预览 Qoder 缓存写入' }));
}
it('requires bound confirmation, clears it on input changes and never writes on cancellation', () => {
  const write = vi.spyOn(bridge, 'writeQoderCache'); render(<QoderDistribution />); writeForm();
  expect(screen.getByRole('dialog')).toHaveTextContent('/selected/new-cache');
  fireEvent.change(screen.getByLabelText('Qoder 全新缓存绝对目录'), { target: { value: '/other' } });
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  writeForm(); fireEvent.click(screen.getByRole('button', { name: '取消 Qoder 写入确认' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(write).not.toHaveBeenCalled();
});
it.each(['stage','unpack'] as const)('confirms %s once, locks input and retains busy through unmount until real settlement', async operation => {
  const work = defer<{ operation: typeof operation; state: 'staged_archive_verified' }>();
  const write = vi.spyOn(bridge, 'writeQoderCache').mockReturnValue(work.promise);
  const view = render(<QoderDistribution />);
  if (operation === 'unpack') fireEvent.click(screen.getByRole('button', { name: '选择解包缓存' }));
  writeForm(); fireEvent.click(screen.getByRole('button', { name: '确认创建 Qoder 缓存' }));
  expect(write).toHaveBeenCalledExactlyOnceWith(operation, '/selected/source', '/selected/new-cache', true);
  for (const label of ['Qoder 官方归档绝对路径','Qoder 缓存绝对目录','Qoder 缓存写入来源绝对路径','Qoder 全新缓存绝对目录']) expect(screen.getByLabelText(label)).toBeDisabled();
  expect(screen.getByRole('button', { name: '清除 Qoder 选择与结果' })).toBeDisabled();
  expect(screen.queryByRole('button', { name: '取消 Qoder 结果' })).not.toBeInTheDocument();
  view.unmount(); expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
  await act(async () => work.resolve({ operation, state: 'staged_archive_verified' }));
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false);
});
it('shows fixed write failure and no automatic retry, then requires a fresh confirmation', async () => {
  const write=vi.spyOn(bridge,'writeQoderCache').mockRejectedValue(new Error('SECRET /private'));
  render(<QoderDistribution />);writeForm();fireEvent.click(screen.getByRole('button',{name:'确认创建 Qoder 缓存'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('可能留下部分私有目录');
  expect(screen.getByRole('alert')).not.toHaveTextContent('SECRET');expect(write).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});
it('renders fixed successful cache result and invalidates confirmation when operation changes', async () => {
  const write=vi.spyOn(bridge,'writeQoderCache').mockResolvedValue({operation:'unpack',state:'unpacked_payload_verified'});
  render(<QoderDistribution />);writeForm();
  fireEvent.click(screen.getByRole('button',{name:'选择解包缓存'}));expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:'预览 Qoder 缓存写入'}));
  expect(screen.getByRole('dialog')).toHaveTextContent('操作：解包缓存');
  fireEvent.click(screen.getByRole('button',{name:'确认创建 Qoder 缓存'}));
  expect(await screen.findByRole('region',{name:'Qoder 缓存写入结果'})).toHaveTextContent('未安装、未运行');
  expect(write).toHaveBeenCalledExactlyOnceWith('unpack','/selected/source','/selected/new-cache',true);
  expect(screen.getByLabelText('Qoder 全新缓存绝对目录')).not.toBeDisabled();
});
