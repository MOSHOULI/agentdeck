import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {act, cleanup, fireEvent, render, screen} from '@testing-library/react';
import * as bridge from '../bridge';
import {dshNativeEvidence, type DshNativeVerification} from '../dshNativeContract';
import {DshNativeArchives} from './DshNativeArchives';
import {DshInventory} from './DshInventory';
import {dshDemoReport} from '../dshContract';
import {dshRootPlan, type DshRootVerification} from '../dshDistributionContract';
import {useWorkspaceActivityStore} from '../stores/workspaceActivity';
const verified: DshNativeVerification = {
  schema_version: 1, state: 'native_archive_static_verified', package_id: 'koffi', ...dshNativeEvidence.koffi,
  archive_verified_during_this_command: true, reviewed_manifest_and_sources_matched: true,
  metadata_archive_agreement: 'reviewed_declaration_fields_match', lifecycle_executed: false,
  native_code_loaded: false, closure_complete: false, install_ready: false, dependencies_resolved: false,
  runtime_verified: false, signature_verified: false, platform_compatibility: 'unverified',
};
const root: DshRootVerification = {state: 'root_archive_verified', bytes: 25197, plan: dshRootPlan};
beforeEach(() => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'verifyDshNativePackage').mockResolvedValue(verified);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); expect(Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean)).toBe(false); });
function select() { fireEvent.change(screen.getByLabelText('DSH 原生依赖包'), {target: {value: 'koffi'}}); }
function fill() { select(); fireEvent.change(screen.getByLabelText('DSH 原生依赖归档绝对路径'), {target: {value: '/selected/koffi-3.1.1.tgz'}}); }
function verify() { fireEvent.click(screen.getByRole('button', {name: '只读校验 DSH 原生依赖归档'})); }
function result() { return screen.queryByRole('region', {name: 'DSH 原生依赖归档校验结果'}); }
function deferred<T>() { let resolve!: (v: T) => void; let reject!: (e: Error) => void; const promise = new Promise<T>((a, b) => { resolve = a; reject = b; }); return {resolve, reject, promise}; }
it('requires explicit package and path, shows only static success and clears settled outcomes on changes', async () => {
  render(<DshNativeArchives/>);
  expect(screen.getByLabelText('DSH 原生依赖包')).toHaveValue(''); verify();
  expect(bridge.verifyDshNativePackage).not.toHaveBeenCalled();
  fill(); verify();
  expect(await screen.findByRole('region', {name: 'DSH 原生依赖归档校验结果'})).toHaveTextContent('依赖闭包未完成');
  expect(result()).toHaveTextContent('未验证运行或独立签名'); expect(result()).toHaveTextContent('302173');
  expect(bridge.verifyDshNativePackage).toHaveBeenCalledExactlyOnceWith('koffi', '/selected/koffi-3.1.1.tgz');
  fireEvent.change(screen.getByLabelText('DSH 原生依赖包'), {target: {value: 'node-pty'}});
  expect(result()).not.toBeInTheDocument(); expect(screen.getByLabelText('DSH 原生依赖归档绝对路径')).toHaveValue('');
  fill(); verify(); await screen.findByRole('region', {name: 'DSH 原生依赖归档校验结果'});
  fireEvent.change(screen.getByLabelText('DSH 原生依赖归档绝对路径'), {target: {value: '/changed/koffi-3.1.1.tgz'}});
  expect(result()).not.toBeInTheDocument();
});
for (const outcome of ['resolve', 'reject'] as const) {
  it.each(['path', 'package', 'clear', 'cancel', 'unmount'])(`drops late ${outcome} after %s and retains ownership until settlement`, async action => {
    const work = deferred<DshNativeVerification>(); vi.mocked(bridge.verifyDshNativePackage).mockReturnValue(work.promise);
    const view = render(<DshNativeArchives/>); fill(); verify(); verify();
    expect(bridge.verifyDshNativePackage).toHaveBeenCalledTimes(1);
    if (action === 'path') fireEvent.change(screen.getByLabelText('DSH 原生依赖归档绝对路径'), {target: {value: '/changed/koffi-3.1.1.tgz'}});
    if (action === 'package') fireEvent.change(screen.getByLabelText('DSH 原生依赖包'), {target: {value: 'node-pty'}});
    if (action === 'clear') fireEvent.click(screen.getByRole('button', {name: '清除 DSH 原生依赖选择与结果'}));
    if (action === 'cancel') fireEvent.click(screen.getByRole('button', {name: '取消 DSH 原生依赖结果'}));
    if (action === 'unmount') view.unmount();
    expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(true);
    if (action !== 'unmount') { verify(); expect(bridge.verifyDshNativePackage).toHaveBeenCalledTimes(1); }
    await act(async () => { if (outcome === 'resolve') work.resolve(verified); else work.reject(new Error('SECRET')); });
    expect(result()).not.toBeInTheDocument(); expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
}
it.each(['resolve', 'reject'] as const)('remount neither inherits old %s nor releases another owner', async outcome => {
  const old = deferred<DshNativeVerification>(); vi.mocked(bridge.verifyDshNativePackage).mockReturnValueOnce(old.promise);
  const view = render(<DshNativeArchives/>); fill(); verify(); view.unmount(); render(<DshNativeArchives/>);
  expect(screen.getByLabelText('DSH 原生依赖包')).toBeDisabled(); verify();
  expect(bridge.verifyDshNativePackage).toHaveBeenCalledTimes(1);
  const other = Symbol(); act(() => useWorkspaceActivityStore.getState().setActivity('dsh', other, true));
  await act(async () => { if (outcome === 'resolve') old.resolve(verified); else old.reject(new Error('OLD_SECRET')); });
  expect(result()).not.toBeInTheDocument(); expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(true);
  act(() => useWorkspaceActivityStore.getState().setActivity('dsh', other, false));
  fill(); verify(); await screen.findByRole('region', {name: 'DSH 原生依赖归档校验结果'});
  expect(bridge.verifyDshNativePackage).toHaveBeenCalledTimes(2);
});
it('hides prior success and redacts failure, clear removes failure and inputs', async () => {
  render(<DshNativeArchives/>); fill(); verify(); await screen.findByRole('region', {name: 'DSH 原生依赖归档校验结果'});
  vi.mocked(bridge.verifyDshNativePackage).mockRejectedValue(new Error('/SECRET/private/path'));
  verify(); expect(await screen.findByRole('alert')).not.toHaveTextContent('SECRET'); expect(result()).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', {name: '清除 DSH 原生依赖选择与结果'}));
  expect(screen.queryByRole('alert')).not.toBeInTheDocument(); expect(screen.getByLabelText('DSH 原生依赖包')).toHaveValue(''); expect(screen.getByLabelText('DSH 原生依赖归档绝对路径')).toHaveValue('');
});
it.each(['demo', 'unavailable'] as const)('never reads in %s mode', mode => {
  vi.mocked(bridge.runtimeMode).mockReturnValue(mode); render(<DshNativeArchives/>); verify();
  expect(screen.getByLabelText('DSH 原生依赖包')).toBeDisabled();
  expect(bridge.verifyDshNativePackage).not.toHaveBeenCalled(); expect(result()).not.toBeInTheDocument();
});
it('inventory and root reads block native verification, including while unmounted', async () => {
  const inventory = deferred<bridge.DshInspectionResult>(); vi.spyOn(bridge, 'inspectDshProfile').mockReturnValueOnce(inventory.promise);
  let view = render(<DshInventory/>); fill();
  fireEvent.change(screen.getByLabelText('Profile 目录（绝对路径）'), {target: {value: '/profile'}});
  fireEvent.click(screen.getByRole('button', {name: '盘点所选声明'}));
  expect(screen.getByRole('button', {name: '只读校验 DSH 原生依赖归档'})).toBeDisabled(); verify(); view.unmount();
  view = render(<DshInventory/>); expect(screen.getByLabelText('DSH 原生依赖包')).toBeDisabled();
  await act(async () => inventory.resolve({report: dshDemoReport(false), synthetic: false}));
  const rootWork = deferred<DshRootVerification>(); vi.spyOn(bridge, 'verifyDshPackage').mockReturnValue(rootWork.promise); fill();
  fireEvent.change(screen.getByLabelText('DSH 官方根归档绝对路径'), {target: {value: '/selected/dsh-0.2.2-rc.1.tgz'}});
  fireEvent.click(screen.getByRole('button', {name: '只读校验 DSH 根归档'}));
  expect(screen.getByRole('button', {name: '只读校验 DSH 原生依赖归档'})).toBeDisabled(); verify();
  expect(bridge.verifyDshNativePackage).not.toHaveBeenCalled(); await act(async () => rootWork.resolve(root));
});
it('native reads block inventory, root and other workspace actions until settled', async () => {
  const work = deferred<DshNativeVerification>(); vi.mocked(bridge.verifyDshNativePackage).mockReturnValue(work.promise);
  vi.spyOn(bridge, 'dshDistributionPlan').mockResolvedValue(dshRootPlan); vi.spyOn(bridge, 'inspectDshProfile').mockResolvedValue({report: dshDemoReport(false), synthetic: false});
  render(<DshInventory/>);
  fireEvent.change(screen.getByLabelText('Profile 目录（绝对路径）'), {target: {value: '/profile'}}); fill(); verify();
  for (const name of ['盘点所选声明', '查看 DSH 根包计划', '只读校验 DSH 根归档']) expect(screen.getByRole('button', {name})).toBeDisabled();
  fireEvent.click(screen.getByRole('button', {name: '查看 DSH 根包计划'})); fireEvent.click(screen.getByRole('button', {name: '盘点所选声明'}));
  expect(bridge.dshDistributionPlan).not.toHaveBeenCalled(); expect(bridge.inspectDshProfile).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', {name: '取消 DSH 原生依赖结果'}));
  expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(true); await act(async () => work.resolve(verified));
});
it.each(['agents','memory','mcp','vault','updates','qwen','qoder'] as const)('%s owner blocks native reads without being released', async workspace => {
  render(<DshNativeArchives/>); fill(); const owner = Symbol(); act(() => useWorkspaceActivityStore.getState().setActivity(workspace, owner, true));
  verify(); expect(bridge.verifyDshNativePackage).not.toHaveBeenCalled(); expect(useWorkspaceActivityStore.getState().busy[workspace]).toBe(true);
  act(() => useWorkspaceActivityStore.getState().setActivity(workspace, owner, false));
  verify(); await screen.findByRole('region', {name: 'DSH 原生依赖归档校验结果'});
});
