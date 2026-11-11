import { StrictMode } from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { installPlan } from './test-fixtures/installPlan';
import App from './App';
import { useAgentStore } from './stores/agentStore';
import { useWorkspaceActivityStore } from './stores/workspaceActivity';
import * as bridge from './bridge';
import { installedAgent, notDetectedAgent } from './test-fixtures/agents';
import { dshDemoReport } from './dshContract';

beforeEach(() => {
  window.history.replaceState({}, '', '/?demo=1');
  window.localStorage.clear();
  act(() => useAgentStore.setState({ installSessions: {}, agents: [], selectedAgentId: null, loading: false, error: null, notice: null, refreshError: null, dataStale: true }));
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  window.history.replaceState({}, '', '/');
  window.localStorage.clear();
  act(() => useAgentStore.setState({ installSessions: {}, agents: [], selectedAgentId: null, loading: false, error: null, notice: null, refreshError: null, dataStale: true }));
});

async function renderReadyApp(strict = false) {
  const view = render(strict ? <StrictMode><App /></StrictMode> : <App />);
  await screen.findByRole('button', { name: '选择 OpenCode' });
  return view;
}

it('visibly labels read-only demo data in every workspace', async () => {
  await renderReadyApp();
  expect(screen.getByText(/只读演示模式 · 所有数据均为示例/)).toBeVisible();
  for (const name of ['记忆管理', 'MCP 服务', '密钥与备份', 'DSH 声明盘点', '注册表更新', 'Agent 管理']) {
    fireEvent.click(screen.getByRole('tab', { name: new RegExp(`^${name}`) }));
    expect(screen.getAllByRole('tabpanel')).toHaveLength(1);
    expect(screen.getByText(/只读演示模式 · 所有数据均为示例/)).toBeVisible();
  }
});

it('never renders installed sample agents without explicit demo', async () => {
  window.history.replaceState({}, '', '/');
  render(<App />);
  expect(screen.getByText(/未连接桌面后端 · 请在/)).toBeInTheDocument();
  await screen.findByRole('alert');
  expect(screen.queryByText('OpenCode')).not.toBeInTheDocument();
});

it('omits the demo banner for a native runtime', async () => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  await renderReadyApp();
  expect(screen.getByText('本地桌面模式')).toBeVisible();
  expect(screen.queryByText(/所有数据均为示例/)).not.toBeInTheDocument();
  expect(screen.queryByText(/未连接桌面后端 · 请在/)).not.toBeInTheDocument();
});

it('keeps all eight workspace trees mounted and retains local search state', async () => {
  const view = await renderReadyApp(true);
  expect(screen.getAllByRole('tabpanel', { hidden: true })).toHaveLength(8);
  const agents = screen.getByRole('tabpanel', { name: /^Agent 管理/ });
  const search = within(agents).getByRole('searchbox', { name: '搜索 Agent' });
  fireEvent.change(search, { target: { value: 'OpenCode' } });
  fireEvent.click(screen.getByRole('tab', { name: /^记忆管理/ }));
  expect(agents).not.toBeVisible();
  expect(view.container.contains(search)).toBe(true);
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  expect(within(agents).getByRole('searchbox', { name: '搜索 Agent' })).toBe(search);
  expect(search).toHaveValue('OpenCode');
});

it('retains an in-flight operation and its busy lock across navigation', async () => {
  let resolveSearch!: (value: bridge.MemoryEntry[]) => void;
  const pendingSearch = new Promise<bridge.MemoryEntry[]>((resolve) => { resolveSearch = resolve; });
  const search = vi.spyOn(bridge, 'searchMemory').mockReturnValue(pendingSearch);
  await renderReadyApp(true);
  fireEvent.click(screen.getByRole('tab', { name: /^记忆管理/ }));
  const memory = screen.getByRole('tabpanel', { name: /^记忆管理/ });
  fireEvent.change(within(memory).getByRole('textbox', { name: '搜索本地记忆' }), { target: { value: 'retained query' } });
  fireEvent.submit(memory.querySelector('form')!);
  expect(search).toHaveBeenCalledTimes(1);
  expect(useWorkspaceActivityStore.getState().busy.memory).toBe(true);
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  expect(screen.getByText(/进行中：.*记忆管理/)).toBeVisible();
  expect(screen.getByRole('tab', { name: '记忆管理，操作进行中' })).toBeVisible();
  fireEvent.click(screen.getByRole('tab', { name: /^记忆管理/ }));
  expect(within(memory).getByRole('textbox', { name: '搜索本地记忆' })).toHaveValue('retained query');
  expect(memory.querySelector('button[type="submit"]')).toBeDisabled();
  fireEvent.submit(memory.querySelector('form')!);
  expect(search).toHaveBeenCalledTimes(1);
  await act(async () => { resolveSearch([]); await pendingSearch; });
  await waitFor(() => expect(useWorkspaceActivityStore.getState().busy.memory).toBe(false));
  expect(memory.querySelector('button[type="submit"]')).toBeEnabled();
  expect(screen.queryByText(/进行中：.*记忆管理/)).not.toBeInTheDocument();
});

it.each(['install', 'configure'] as const)('retains the in-flight %s lock across workspace navigation', async (operation) => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  vi.spyOn(bridge, 'previewAgentInstall').mockResolvedValue(installPlan);
  const target = operation === 'install' ? notDetectedAgent : installedAgent;
  vi.spyOn(bridge, 'getAgents').mockResolvedValue([target]);
  let finish!: (outcome: bridge.InstallOutcome) => void;
  const pending = new Promise<bridge.InstallOutcome>((resolve) => { finish = resolve; });
  const mutation = vi.spyOn(bridge, operation === 'install' ? 'installAgent' : 'configureAgent').mockReturnValue(pending);
  await renderReadyApp(true);
  const actionName = operation === 'install' ? '安装 OpenCode' : '应用默认配置 OpenCode';
  const action = screen.getByRole('button', { name: actionName });
  fireEvent.click(action);
  if (operation === 'install') fireEvent.click(await screen.findByRole('button', { name: '确认安装软件' }));
  fireEvent.click(action);
  expect(action).toBeDisabled();
  expect(mutation).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('tab', { name: /^记忆管理/ }));
  expect(screen.getByText(/进行中：.*Agent 管理/)).toBeVisible();
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  expect(screen.getByRole('button', { name: actionName })).toBe(action);
  expect(action).toBeDisabled();
  fireEvent.click(action);
  expect(mutation).toHaveBeenCalledTimes(1);
  expect(window.confirm).toHaveBeenCalledTimes(operation === 'install' ? 0 : 1);
  await act(async () => { finish({ warnings: [] }); await pending; });
  await waitFor(() => expect(useAgentStore.getState().loading).toBe(false));
  expect(action).toBeEnabled();
  expect(within(screen.getByRole('tabpanel', { name: /^Agent 管理/ })).getByRole('status')).toHaveTextContent(operation === 'install' ? '软件安装操作已完成' : '默认 profile 配置操作已完成');
});

it('keeps preview activity visible through navigation while discarding the hidden review', async () => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'getAgents').mockResolvedValue([notDetectedAgent]);
  let finish!: (value: typeof installPlan) => void;
  const preview = vi.spyOn(bridge, 'previewAgentInstall').mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const install = vi.spyOn(bridge, 'installAgent');
  await renderReadyApp(true);
  fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
  expect(preview).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('tab', { name: /^记忆管理/ }));
  expect(screen.getByText(/进行中：.*Agent 管理/)).toBeVisible();
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
  expect(preview).toHaveBeenCalledTimes(1);
  await act(async () => finish(installPlan));
  expect(screen.queryByRole('button', { name: '确认安装软件' })).not.toBeInTheDocument();
  expect(install).not.toHaveBeenCalled();
  expect(screen.queryByText(/进行中：.*Agent 管理/)).not.toBeInTheDocument();
});

it('keeps agent activity visible while a different workspace is open', async () => {
  await renderReadyApp();
  act(() => useAgentStore.setState({ loading: true }));
  fireEvent.click(screen.getByRole('tab', { name: /^密钥与备份/ }));
  expect(screen.getByText(/进行中：.*Agent 管理/)).toBeVisible();
  expect(screen.getByRole('tab', { name: 'Agent 管理，操作进行中' })).toHaveAttribute('aria-selected', 'false');
  act(() => useAgentStore.setState({ loading: false }));
  expect(screen.queryByText(/进行中：.*Agent 管理/)).not.toBeInTheDocument();
});

it('retains a DSH inspection and its input across navigation without scanning again', async () => {
  let finish!: (value: bridge.DshInspectionResult) => void;
  const pending = new Promise<bridge.DshInspectionResult>((resolve) => { finish = resolve; });
  const inspect = vi.spyOn(bridge, 'inspectDshProfile').mockReturnValue(pending);
  await renderReadyApp(true);
  act(() => useAgentStore.setState({ agents: [{ ...notDetectedAgent, id: 'dsh', name: 'DeepSeek Harness (DSH)', canInstall: false, canConfigure: false, canVerifyInstall: false, installationStatus: 'unsupported' }] }));
  expect(inspect).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '打开 DSH 声明盘点' }));
  const dsh = screen.getByRole('tabpanel', { name: /^DSH 声明盘点/ });
  const input = within(dsh).getByLabelText('Profile 目录（绝对路径）');
  fireEvent.change(input, { target: { value: '/synthetic/explicit-selection' } });
  fireEvent.submit(within(dsh).getByRole('form', { name: 'DSH 盘点路径' }));
  expect(inspect).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  expect(screen.getByText(/进行中：.*DSH 声明盘点/)).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '打开 DSH 声明盘点' }));
  expect(within(dsh).getByLabelText('Profile 目录（绝对路径）')).toBe(input);
  expect(input).toHaveValue('/synthetic/explicit-selection');
  expect(within(dsh).getByRole('button', { name: '盘点中…' })).toBeDisabled();
  fireEvent.submit(within(dsh).getByRole('form', { name: 'DSH 盘点路径' }));
  expect(inspect).toHaveBeenCalledTimes(1);
  await act(async () => { finish({ report: dshDemoReport(false), synthetic: true }); await pending; });
  expect(within(dsh).getByText('合成示例结果，不代表本机状态。')).toBeVisible();
  expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(false);
});

it('supports roving keyboard navigation with arrows, Home and End', async () => {
  await renderReadyApp();
  const agentTab = screen.getByRole('tab', { name: /^Agent 管理/ });
  agentTab.focus();
  fireEvent.keyDown(agentTab, { key: 'ArrowDown' });
  const memoryTab = screen.getByRole('tab', { name: /^记忆管理/ });
  expect(memoryTab).toHaveFocus();
  expect(memoryTab).toHaveAttribute('aria-selected', 'true');
  expect(agentTab).toHaveAttribute('tabindex', '-1');
  fireEvent.keyDown(memoryTab, { key: 'End' });
  const updatesTab = screen.getByRole('tab', { name: /^注册表更新/ });
  expect(updatesTab).toHaveFocus();
  fireEvent.keyDown(updatesTab, { key: 'ArrowDown' });
  expect(agentTab).toHaveFocus();
  fireEvent.keyDown(agentTab, { key: 'ArrowUp' });
  expect(updatesTab).toHaveFocus();
  fireEvent.keyDown(updatesTab, { key: 'Home' });
  expect(agentTab).toHaveFocus();
  expect(screen.getByRole('tabpanel', { name: /^Agent 管理/ })).toBeVisible();
});

it('persists sidebar collapse without losing accessible navigation names', async () => {
  const view = await renderReadyApp();
  fireEvent.click(screen.getByRole('button', { name: '收起侧栏' }));
  expect(view.container.querySelector('.app')).toHaveClass('app-sidebar-collapsed');
  expect(window.localStorage.getItem('agentdeck.sidebar-collapsed')).toBe('true');
  expect(screen.getByRole('tab', { name: /^记忆管理/ })).toBeInTheDocument();
  view.unmount();
  await renderReadyApp();
  expect(screen.getByRole('button', { name: '展开侧栏' })).toHaveAttribute('aria-expanded', 'false');
  fireEvent.click(screen.getByRole('button', { name: '展开侧栏' }));
  expect(window.localStorage.getItem('agentdeck.sidebar-collapsed')).toBe('false');
});

it('persists explicit themes and follows subsequent system changes', async () => {
  let systemDark = true;
  const listeners = new Set<() => void>();
  const media = {
    get matches() { return systemDark; },
    addEventListener: vi.fn((_event: string, listener: () => void) => { listeners.add(listener); }),
    removeEventListener: vi.fn((_event: string, listener: () => void) => { listeners.delete(listener); }),
  };
  vi.stubGlobal('matchMedia', vi.fn(() => media));
  const view = await renderReadyApp();
  expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  expect(screen.getByRole('button', { name: '跟随系统' })).toHaveAttribute('aria-pressed', 'true');
  fireEvent.click(screen.getByRole('button', { name: '浅色' }));
  expect(document.documentElement).toHaveAttribute('data-theme', 'light');
  expect(window.localStorage.getItem('agentdeck.theme')).toBe('light');
  act(() => { for (const listener of listeners) listener(); });
  expect(document.documentElement).toHaveAttribute('data-theme', 'light');
  view.unmount();
  const next = await renderReadyApp();
  expect(screen.getByRole('button', { name: '浅色' })).toHaveAttribute('aria-pressed', 'true');
  fireEvent.click(screen.getByRole('button', { name: '跟随系统' }));
  expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  act(() => { systemDark = false; for (const listener of listeners) listener(); });
  expect(document.documentElement).toHaveAttribute('data-theme', 'light');
  fireEvent.click(screen.getByRole('button', { name: '深色' }));
  expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  next.unmount();
  expect(listeners.size).toBe(0);
});

it('ignores invalid stored themes and keeps controls working if storage is blocked', async () => {
  window.localStorage.setItem('agentdeck.theme', 'unexpected-value');
  const view = await renderReadyApp();
  expect(screen.getByRole('button', { name: '跟随系统' })).toHaveAttribute('aria-pressed', 'true');
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('Storage denied'); });
  fireEvent.click(screen.getByRole('button', { name: '深色' }));
  expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  fireEvent.click(screen.getByRole('button', { name: '收起侧栏' }));
  expect(view.container.querySelector('.app')).toHaveClass('app-sidebar-collapsed');
});

it('opens DSH inventory from its catalog card without inspecting or mutating, and preserves selections', async () => {
  const inspect = vi.spyOn(bridge, 'inspectDshProfile');
  await renderReadyApp();
  act(() => useAgentStore.setState({ agents: [{ ...notDetectedAgent, id: 'dsh', name: 'DeepSeek Harness (DSH)', canInstall: false, canConfigure: false, canVerifyInstall: false, installationStatus: 'unsupported' }] }));
  fireEvent.click(screen.getByRole('button', { name: '打开 DSH 声明盘点' }));
  expect(screen.getByRole('tab', { name: /^DSH 声明盘点/ })).toHaveFocus();
  expect(screen.getByRole('tabpanel', { name: /^DSH 声明盘点/ })).toBeVisible();
  expect(inspect).not.toHaveBeenCalled();
  const input = screen.getByRole('textbox', { name: 'Profile 目录（绝对路径）' });
  fireEvent.change(input, { target: { value: '/explicit/selected-profile' } });
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  fireEvent.click(screen.getByRole('button', { name: '打开 DSH 声明盘点' }));
  expect(input).toHaveValue('/explicit/selected-profile');
  expect(inspect).not.toHaveBeenCalled();
});

it('retains Qwen mutation lock across navigation away and back', async () => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'getAgents').mockResolvedValue([notDetectedAgent]);
  vi.spyOn(bridge, 'previewQwenVim').mockResolvedValue({ field: 'general.vimMode', before: null, after: false, changes: true, token: 'c'.repeat(64) });
  let finish!: (value: boolean) => void;
  const pending = new Promise<boolean>(resolve => { finish = resolve; });
  const apply = vi.spyOn(bridge, 'applyQwenVim').mockReturnValue(pending);
  await renderReadyApp(true);
  fireEvent.click(screen.getByRole('tab', { name: /^Qwen 配置检查/ }));
  const qwen = screen.getByRole('tabpanel', { name: /^Qwen 配置检查/ });
  fireEvent.change(within(qwen).getByLabelText('settings.json 绝对路径'), { target: { value: '/selected/settings.json' } });
  fireEvent.change(within(qwen).getByLabelText('运行时 package.json 绝对路径'), { target: { value: '/runtime/package.json' } });
  fireEvent.click(within(qwen).getByRole('button', { name: '预览 Vim 变更' }));
  fireEvent.click(await within(qwen).findByRole('button', { name: '确认应用 Vim 变更' }));
  expect(apply).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  expect(screen.getByText(/进行中：.*Qwen 配置检查/)).toBeVisible();
  fireEvent.click(screen.getByRole('tab', { name: /^Qwen 配置检查/ }));
  expect(within(qwen).getByLabelText('settings.json 绝对路径')).toBeDisabled();
  expect(within(qwen).getByRole('button', { name: '预览 Vim 变更' })).toBeDisabled();
  expect(within(qwen).getByRole('button', { name: '撤销受管 Vim 设置' })).toBeDisabled();
  fireEvent.click(within(qwen).getByRole('button', { name: '预览 Vim 变更' }));
  expect(apply).toHaveBeenCalledTimes(1);
  await act(async () => { finish(true); await pending; });
  expect(within(qwen).getByLabelText('settings.json 绝对路径')).toBeEnabled();
  expect(useWorkspaceActivityStore.getState().busy.qwen).toBe(false);
});

it.each(['completed', 'pending-success', 'pending-failure'] as const)('clears residual observation on real tab navigation while preserving history: %s', async mode => {
  const codex = { ...notDetectedAgent, id: 'codex', name: 'Codex' };
  vi.spyOn(bridge, 'getAgents').mockResolvedValue([codex]);
  const result = ['package_directory', 'posix_launcher', 'cmd_launcher', 'powershell_launcher'].map(slot => ({ slot, state: 'missing' })) as Awaited<ReturnType<typeof bridge.inspectCodexInstallResidual>>;
  let resolve!: (value: typeof result) => void;
  let reject!: (reason: unknown) => void;
  const inspect = vi.spyOn(bridge, 'inspectCodexInstallResidual').mockImplementation(() => mode === 'completed' ? Promise.resolve(result) : new Promise((yes, no) => { resolve = yes; reject = no; }));
  render(<App />);
  await screen.findByRole('button', { name: '选择 Codex' });
  act(() => useAgentStore.setState({ installSessions: { codex: { state: 'unconfirmed', target: null, sessionRef: '12345678-1234-4123-8123-123456789abc' } } }));
  const record = screen.getByTestId('install-session-record');
  (record as HTMLDetailsElement).open = true;
  fireEvent(record, new Event('toggle'));
  const button = await screen.findByRole('button', { name: '只读检查固定残留槽位' });
  fireEvent.click(button);
  if (mode === 'completed') await screen.findByTestId('codex-residual-result');
  fireEvent.click(screen.getByRole('tab', { name: /^Qwen 配置检查/ }));
  expect(screen.getByRole('tab', { name: /^Qwen 配置检查/ })).toHaveAttribute('aria-selected', 'true');
  expect(screen.queryByTestId('codex-residual-result')).not.toBeInTheDocument();
  if (mode !== 'completed') await act(async () => { if (mode === 'pending-success') resolve(result); else reject(new Error('private late response')); });
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  expect(screen.queryByTestId('codex-residual-result')).not.toBeInTheDocument();
  expect(screen.queryByText(/本会话只读检查不可用或路径发生变化/)).not.toBeInTheDocument();
  expect(screen.getByTestId('install-session-record')).toHaveAttribute('data-state', 'unconfirmed');
  expect(inspect).toHaveBeenCalledTimes(1);
});
it.each(['completed', 'pending'] as const)('invalidates Qoder private preview on real navigation: %s', async mode => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  let resolve!: (value: { token: string }) => void;
  const preview = vi.spyOn(bridge, 'previewQoderDeployment').mockImplementation(() => mode === 'completed' ? Promise.resolve({ token: 'a'.repeat(64) }) : new Promise(yes => { resolve = yes; }));
  const deploy = vi.spyOn(bridge, 'deployQoderPrivate');
  await renderReadyApp();
  fireEvent.click(screen.getByRole('tab', { name: /^Qoder 分发与校验/ }));
  fireEvent.change(screen.getByLabelText('Qoder 已解包缓存绝对目录'), { target: { value: '/cache' } });
  fireEvent.change(screen.getByLabelText('Qoder 全新私有部署绝对目录'), { target: { value: '/new' } });
  fireEvent.click(screen.getByRole('button', { name: '预览 Qoder 私有部署' }));
  if (mode === 'completed') await screen.findByRole('dialog', { name: '确认 Qoder 私有可执行部署' });
  fireEvent.click(screen.getByRole('tab', { name: /^Agent 管理/ }));
  if (mode === 'pending') {
    expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);
    await act(async () => resolve({ token: 'a'.repeat(64) }));
  }
  fireEvent.click(screen.getByRole('tab', { name: /^Qoder 分发与校验/ }));
  expect(screen.queryByRole('dialog', { name: '确认 Qoder 私有可执行部署' })).not.toBeInTheDocument();
  expect(preview).toHaveBeenCalledTimes(1); expect(deploy).not.toHaveBeenCalled();
});
