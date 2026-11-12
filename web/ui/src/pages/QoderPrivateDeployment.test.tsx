import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import { QoderPrivateDeployment } from './QoderPrivateDeployment';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
vi.mock('../bridge', () => ({ runtimeMode: vi.fn(() => 'native'), previewQoderDeployment: vi.fn(), deployQoderPrivate: vi.fn() }));
const token = 'a'.repeat(64);
const pending = <T,>() => { let resolve!: (v: T) => void; let reject!: (v: unknown) => void; const promise = new Promise<T>((r, j) => { resolve = r; reject = j; }); return { promise, resolve, reject }; };
function fill() { fireEvent.change(screen.getByLabelText('Qoder 已解包缓存绝对目录'), { target: { value: '/cache with space' } }); fireEvent.change(screen.getByLabelText('Qoder 全新私有部署绝对目录'), { target: { value: '/private/new' } }); }
const preview = () => fireEvent.click(screen.getByRole('button', { name: '预览 Qoder 私有部署' }));
const confirm = () => fireEvent.click(screen.getByRole('button', { name: '确认部署 Qoder 可执行文件' }));
beforeEach(() => {
  vi.clearAllMocks(); vi.mocked(bridge.runtimeMode).mockReturnValue('native');
  vi.mocked(bridge.previewQoderDeployment).mockResolvedValue({ token });
  vi.mocked(bridge.deployQoderPrivate).mockResolvedValue({ state: 'private_executable_deployed_not_activated' });
  const current = useWorkspaceActivityStore.getState();
  for (const id of Object.keys(current.owners) as (keyof typeof current.owners)[]) for (const owner of current.owners[id]) current.setActivity(id, owner, false);
});
afterEach(cleanup);
it('requires a real preview and explicit confirmation preserving exact paths', async () => {
  render(<QoderPrivateDeployment />); fill();
  expect(bridge.previewQoderDeployment).not.toHaveBeenCalled();
  expect(bridge.deployQoderPrivate).not.toHaveBeenCalled();
  preview(); await screen.findByRole('dialog');
  expect(bridge.previewQoderDeployment).toHaveBeenCalledWith('/cache with space', '/private/new', '1.1.85');
  expect(screen.getByText('已审核部署来源：/cache with space')).toBeInTheDocument();
  expect(screen.getByText('载荷字节：182543840；新文件权限：0700')).toBeInTheDocument();
  confirm(); await screen.findByText('Qoder 私有可执行文件部署已完成；未激活、未运行。');
  expect(bridge.deployQoderPrivate).toHaveBeenCalledExactlyOnceWith('/cache with space', '/private/new', token, true, '1.1.85');
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});
it('cancel and edited inputs invalidate confirmation with no write', async () => {
  render(<QoderPrivateDeployment />); fill(); preview(); await screen.findByRole('dialog');
  fireEvent.click(screen.getByRole('button', { name: '取消 Qoder 部署确认' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.deployQoderPrivate).not.toHaveBeenCalled();
  preview(); await screen.findByRole('dialog');
  fireEvent.change(screen.getByLabelText('Qoder 全新私有部署绝对目录'), { target: { value: '/private/other' } });
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.deployQoderPrivate).not.toHaveBeenCalled();
});
it('cancel pending preview drops late response while holding operation lock', async () => {
  const p = pending<{ token: string }>(); vi.mocked(bridge.previewQoderDeployment).mockReturnValue(p.promise);
  render(<QoderPrivateDeployment />); fill(); preview();
  fireEvent.click(screen.getByRole('button', { name: '取消 Qoder 部署预览结果' }));
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
  expect(screen.getByRole('button', { name: '预览 Qoder 私有部署' })).toBeDisabled();
  await act(async () => p.resolve({ token }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false);
});
it('navigation invalidates existing and late previews without releasing in-flight work', async () => {
  const { rerender } = render(<QoderPrivateDeployment />); fill(); preview(); await screen.findByRole('dialog');
  rerender(<QoderPrivateDeployment active={false} />); rerender(<QoderPrivateDeployment active />);
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  const p = pending<{ token: string }>(); vi.mocked(bridge.previewQoderDeployment).mockReturnValue(p.promise); preview();
  rerender(<QoderPrivateDeployment active={false} />); rerender(<QoderPrivateDeployment active />);
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
  await act(async () => p.resolve({ token })); expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});
it('unmount does not release global lock before deployment settles or allow replay', async () => {
  const p = pending<{ state: 'private_executable_deployed_not_activated' }>(); vi.mocked(bridge.deployQoderPrivate).mockReturnValue(p.promise);
  const { unmount } = render(<QoderPrivateDeployment />); fill(); preview(); await screen.findByRole('dialog'); confirm();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '清除 Qoder 部署选择与结果' })).toBeDisabled();
  unmount(); expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
  render(<QoderPrivateDeployment />); fill(); expect(screen.getByRole('button', { name: '预览 Qoder 私有部署' })).toBeDisabled();
  await act(async () => p.resolve({ state: 'private_executable_deployed_not_activated' }));
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false); expect(bridge.deployQoderPrivate).toHaveBeenCalledTimes(1);
});
it('failure retains submitted paths and warns executable partial output with no automatic retry', async () => {
  vi.mocked(bridge.deployQoderPrivate).mockRejectedValue(new Error('SECRET RAW BACKEND'));
  const { rerender } = render(<QoderPrivateDeployment />); fill(); preview(); await screen.findByRole('dialog'); confirm();
  await screen.findByRole('alert');
  expect(screen.getByRole('alert')).toHaveTextContent('目标可能已有可执行文件');
  expect(screen.queryByText(/SECRET/)).not.toBeInTheDocument();
  rerender(<QoderPrivateDeployment active={false} />); rerender(<QoderPrivateDeployment active />);
  expect(screen.getByText('实际提交目标：/private/new')).toBeInTheDocument(); expect(bridge.deployQoderPrivate).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});
it('unsupported native backend is explicit and browser/demo cannot preview or deploy', async () => {
  vi.mocked(bridge.previewQoderDeployment).mockRejectedValue('QODER_DEPLOY_UNSUPPORTED_HOST');
  const { unmount } = render(<QoderPrivateDeployment />); fill(); preview(); await screen.findByText('当前主机不支持 Qoder 私有部署；仅支持 Linux x86_64。没有执行部署。');
  expect(bridge.deployQoderPrivate).not.toHaveBeenCalled(); unmount();
  vi.mocked(bridge.runtimeMode).mockReturnValue('demo'); render(<QoderPrivateDeployment />);
  expect(screen.getByRole('button', { name: '预览 Qoder 私有部署' })).toBeDisabled(); expect(screen.getByLabelText('Qoder 已解包缓存绝对目录')).toBeDisabled();
});
it('does not start a preview while another workspace is busy', () => {
  useWorkspaceActivityStore.getState().setActivity('agents', Symbol('other'), true);
  render(<QoderPrivateDeployment />); fill(); preview(); expect(bridge.previewQoderDeployment).not.toHaveBeenCalled();
});
it('preview lock survives unmount until underlying read settles', async () => {
  const p = pending<{ token: string }>(); vi.mocked(bridge.previewQoderDeployment).mockReturnValue(p.promise);
  const view = render(<QoderPrivateDeployment />); fill(); preview(); view.unmount();
  expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
  await act(async () => p.resolve({ token })); expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false);
});
it('release changes discard completed previews and new confirmation is bound to selected release', async () => {
  const { rerender } = render(<QoderPrivateDeployment />); fill(); preview(); await screen.findByRole('dialog');
  rerender(<QoderPrivateDeployment release="1.1.86" />);
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.deployQoderPrivate).not.toHaveBeenCalled();
  preview(); await screen.findByRole('dialog');
  expect(bridge.previewQoderDeployment).toHaveBeenLastCalledWith('/cache with space', '/private/new', '1.1.86');
  expect(screen.getByText('载荷字节：182412768；新文件权限：0700')).toBeInTheDocument();
  confirm(); await screen.findByText('Qoder 私有可执行文件部署已完成；未激活、未运行。');
  expect(bridge.deployQoderPrivate).toHaveBeenCalledExactlyOnceWith('/cache with space', '/private/new', token, true, '1.1.86');
  expect(screen.getByText('实际提交版本：1.1.86')).toBeInTheDocument();
});
it('late old-release preview cannot restore confirmation after release changes', async () => {
  const deferred = pending<{ token: string }>(); vi.mocked(bridge.previewQoderDeployment).mockReturnValueOnce(deferred.promise);
  const { rerender } = render(<QoderPrivateDeployment />); fill(); preview();
  rerender(<QoderPrivateDeployment release="1.1.86" />);
  await act(async () => { deferred.resolve({ token }); await deferred.promise; });
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.deployQoderPrivate).not.toHaveBeenCalled();
  preview(); await screen.findByRole('dialog'); expect(screen.getByText('固定版本：1.1.86；目标：linux-amd64-baseline')).toBeInTheDocument();
});

it('187 release changes discard completed previews and new confirmation is bound to selected release', async () => {
  const { rerender } = render(<QoderPrivateDeployment />); fill(); preview(); await screen.findByRole('dialog');
  rerender(<QoderPrivateDeployment release="1.1.87" />);
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.deployQoderPrivate).not.toHaveBeenCalled();
  preview(); await screen.findByRole('dialog');
  expect(bridge.previewQoderDeployment).toHaveBeenLastCalledWith('/cache with space', '/private/new', '1.1.87');
  expect(screen.getByText('载荷字节：182466016；新文件权限：0700')).toBeInTheDocument();
  confirm(); await screen.findByText('Qoder 私有可执行文件部署已完成；未激活、未运行。');
  expect(bridge.deployQoderPrivate).toHaveBeenCalledExactlyOnceWith('/cache with space', '/private/new', token, true, '1.1.87');
  expect(screen.getByText('实际提交版本：1.1.87')).toBeInTheDocument();
});
it('187 late old-release preview cannot restore confirmation after release changes', async () => {
  const deferred = pending<{ token: string }>(); vi.mocked(bridge.previewQoderDeployment).mockReturnValueOnce(deferred.promise);
  const { rerender } = render(<QoderPrivateDeployment />); fill(); preview();
  rerender(<QoderPrivateDeployment release="1.1.87" />);
  await act(async () => { deferred.resolve({ token }); await deferred.promise; });
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(bridge.deployQoderPrivate).not.toHaveBeenCalled();
  preview(); await screen.findByRole('dialog'); expect(screen.getByText('固定版本：1.1.87；目标：linux-amd64-baseline')).toBeInTheDocument();
});
