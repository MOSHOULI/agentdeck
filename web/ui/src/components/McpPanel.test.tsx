import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { McpPanel } from './McpPanel';
import type { AgentInfo, McpServer, McpTogglePreview } from '../bridge';
import { useAgentStore } from '../stores/agentStore';

vi.mock('../bridge', () => ({
  runtimeMode: vi.fn(() => 'native'),
  getMcpServers: vi.fn(),
  toggleMcpForAgent: vi.fn(),
  previewMcpToggle: vi.fn(),
  applyMcpTogglePreview: vi.fn(),
}));

import * as bridge from '../bridge';

const servers: McpServer[] = [
  {
    name: 'ragflow',
    description: 'RAGFlow 本地知识库',
    kind: 'http',
    url: 'http://127.0.0.1:9382/mcp',
    args: [],
    supportedAgents: ['codex', 'opencode'],
    enabledAgents: ['codex'],
  },
];

const agents: AgentInfo[] = [
  {
    id: 'codex',
    name: 'Codex',
    installed: true,
    authVerified: false,
    source: 'open',
    uiType: 'cli+desktop',
    stars: 104000,
    popular: true,
    description: 'x',
    icon: 'codex',
    category: 'ai-coding',
  },
  {
    id: 'opencode',
    name: 'OpenCode',
    installed: true,
    authVerified: false,
    source: 'open',
    uiType: 'all',
    stars: 194000,
    popular: true,
    description: 'x',
    icon: 'opencode',
    category: 'ai-coding',
  },
];

afterEach(() => cleanup());
beforeEach(() => {
  vi.resetAllMocks();
  useAgentStore.setState({ agents });
  vi.mocked(bridge.runtimeMode).mockReturnValue('native');
  vi.mocked(bridge.getMcpServers).mockResolvedValue(servers);
  vi.mocked(bridge.previewMcpToggle).mockImplementation(async (name, id, enabled) => makePreview(name, id, enabled));
  vi.mocked(bridge.applyMcpTogglePreview).mockResolvedValue();
});

function makePreview(serverName = 'ragflow', agentId = 'opencode', enabled = true): McpTogglePreview {
  return { serverName, agentId, enabled, configPath: `/synthetic/${agentId}/config.json`, format: 'json_merge', fileExists: true, changed: true, revision: 'revision-1' };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
const checkbox = (name: string) => screen.getByRole('checkbox', { name: `ragflow 的 ${name} 配置` });
async function openPreview(name = 'OpenCode') {
  fireEvent.click(checkbox(name));
  return screen.findByRole('button', { name: '确认并应用' });
}
async function mount() { render(<McpPanel />); await screen.findByRole('heading', { name: 'ragflow' }); }

it('shows only supported configuration states, without claiming server health', async () => {
  useAgentStore.setState({ agents: [...agents, { ...agents[0], id: 'unsupported', name: 'Unsupported' }] });
  await mount();
  expect(screen.getAllByRole('checkbox')).toHaveLength(2);
  expect(checkbox('Codex')).toBeChecked();
  expect(checkbox('OpenCode')).not.toBeChecked();
  expect(screen.getByText(/此处不检测服务器是否在线/)).toBeInTheDocument();
});

it('requires matching preview and confirmation before applying, then reads actual state', async () => {
  await mount();
  const confirm = await openPreview();
  expect(bridge.previewMcpToggle).toHaveBeenCalledWith('ragflow', 'opencode', true);
  expect(bridge.applyMcpTogglePreview).not.toHaveBeenCalled();
  expect(checkbox('OpenCode')).not.toBeChecked();
  expect(screen.getByText('/synthetic/opencode/config.json')).toBeInTheDocument();
  vi.mocked(bridge.getMcpServers).mockResolvedValue([{ ...servers[0], enabledAgents: ['codex', 'opencode'] }]);
  fireEvent.click(confirm);
  await waitFor(() => expect(checkbox('OpenCode')).toBeChecked());
  expect(bridge.applyMcpTogglePreview).toHaveBeenCalledWith('ragflow', 'opencode', true, 'revision-1');
  expect(bridge.toggleMcpForAgent).not.toHaveBeenCalled();
});

it('cancels without writing and keeps original inclusion', async () => {
  await mount();
  await openPreview('Codex');
  expect(screen.getByText('移除此服务配置')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '取消' }));
  expect(bridge.applyMcpTogglePreview).not.toHaveBeenCalled();
  expect(checkbox('Codex')).toBeChecked();
  expect(checkbox('Codex')).toBeEnabled();
});

it('cancels in-flight preview and ignores late response without replacing a newer preview', async () => {
  const pending = deferred<McpTogglePreview>();
  vi.mocked(bridge.previewMcpToggle).mockReturnValueOnce(pending.promise);
  await mount();
  fireEvent.click(checkbox('OpenCode'));
  fireEvent.click(checkbox('OpenCode'));
  expect(bridge.previewMcpToggle).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('button', { name: '取消' }));
  await openPreview('Codex');
  await act(async () => pending.resolve(makePreview()));
  expect(screen.getByText('/synthetic/codex/config.json')).toBeInTheDocument();
  expect(screen.queryByText('/synthetic/opencode/config.json')).not.toBeInTheDocument();
  expect(bridge.applyMcpTogglePreview).not.toHaveBeenCalled();
});

it('guards repeated toggles, confirmation, cancel and refresh during apply', async () => {
  const pending = deferred<void>();
  vi.mocked(bridge.applyMcpTogglePreview).mockReturnValue(pending.promise);
  await mount();
  const confirm = await openPreview();
  fireEvent.click(checkbox('Codex'));
  fireEvent.click(screen.getByRole('button', { name: '刷新配置' }));
  expect(bridge.previewMcpToggle).toHaveBeenCalledTimes(1);
  fireEvent.click(confirm); fireEvent.click(confirm);
  fireEvent.click(screen.getByRole('button', { name: '取消' }));
  expect(bridge.applyMcpTogglePreview).toHaveBeenCalledTimes(1);
  expect(bridge.getMcpServers).toHaveBeenCalledTimes(1);
  expect(checkbox('Codex')).toBeDisabled();
  expect(screen.getByText('正在写入')).toBeInTheDocument();
  await act(async () => pending.resolve());
  expect(checkbox('Codex')).toBeEnabled();
  expect(screen.getByRole('status')).toHaveTextContent('配置状态不代表服务已启动或连接成功');
});

it('discards failed apply and requires refresh, never automatically retries', async () => {
  vi.mocked(bridge.applyMcpTogglePreview).mockRejectedValue(new Error('preview expired'));
  await mount();
  fireEvent.click(await openPreview('Codex'));
  expect(await screen.findByRole('alert')).toHaveTextContent('不会自动重试写入');
  expect(checkbox('Codex')).toBeChecked();
  expect(checkbox('Codex')).toBeDisabled();
  expect(screen.queryByRole('button', { name: '确认并应用' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '刷新配置' }));
  await waitFor(() => expect(checkbox('Codex')).toBeEnabled());
  expect(bridge.applyMcpTogglePreview).toHaveBeenCalledTimes(1);
});

it('reports preview failure without changing inclusion or applying', async () => {
  vi.mocked(bridge.previewMcpToggle).mockRejectedValue(new Error('read denied'));
  await mount(); fireEvent.click(checkbox('Codex'));
  expect(await screen.findByRole('alert')).toHaveTextContent('预览失败，未提交配置修改');
  expect(checkbox('Codex')).toBeChecked(); expect(checkbox('Codex')).toBeEnabled();
  expect(bridge.applyMcpTogglePreview).not.toHaveBeenCalled();
});

it('rejects mismatched preview targets', async () => {
  vi.mocked(bridge.previewMcpToggle).mockResolvedValue(makePreview('other'));
  await mount(); fireEvent.click(checkbox('OpenCode'));
  expect(await screen.findByRole('alert')).toHaveTextContent('预览与请求不一致');
  expect(bridge.applyMcpTogglePreview).not.toHaveBeenCalled();
});

it('ignores preview completion after unmount', async () => {
  const pending = deferred<McpTogglePreview>();
  vi.mocked(bridge.previewMcpToggle).mockReturnValue(pending.promise);
  const view = render(<McpPanel />); await screen.findByText('ragflow');
  fireEvent.click(checkbox('OpenCode')); view.unmount();
  await act(async () => pending.resolve(makePreview()));
  expect(bridge.applyMcpTogglePreview).not.toHaveBeenCalled();
});

it('does not launch follow-up reads after an unmounted apply completes', async () => {
  const pending = deferred<void>();
  vi.mocked(bridge.applyMcpTogglePreview).mockReturnValue(pending.promise);
  const view = render(<McpPanel />); await screen.findByText('ragflow');
  fireEvent.click(await openPreview()); view.unmount();
  await act(async () => pending.resolve());
  expect(bridge.getMcpServers).toHaveBeenCalledTimes(1);
});

it('distinguishes a successful write from a failed follow-up read', async () => {
  await mount(); const confirm = await openPreview('Codex');
  vi.mocked(bridge.getMcpServers).mockRejectedValueOnce(new Error('read denied'));
  fireEvent.click(confirm);
  expect(await screen.findByRole('status')).toHaveTextContent('配置修改已完成，但重新读取失败');
  expect(checkbox('Codex')).toBeDisabled();
  expect(bridge.applyMcpTogglePreview).toHaveBeenCalledTimes(1);
});

it('shows absent-file and no-change states without displaying revision or contents', async () => {
  vi.mocked(bridge.previewMcpToggle).mockResolvedValue({ ...makePreview(), fileExists: false, changed: false });
  await mount(); await openPreview();
  expect(screen.getByText('配置文件尚不存在')).toBeInTheDocument();
  expect(screen.getByText('配置内容无变化')).toBeInTheDocument();
  expect(screen.queryByText('revision-1')).not.toBeInTheDocument();
});

it('describes removal as configuration removal, not shutdown', async () => {
  await mount(); const confirm = await openPreview('Codex');
  vi.mocked(bridge.getMcpServers).mockResolvedValue([{ ...servers[0], enabledAgents: [] }]);
  fireEvent.click(confirm);
  expect(await screen.findByRole('status')).toHaveTextContent('此操作不会停止服务进程');
  expect(checkbox('Codex')).not.toBeChecked();
});

it('distinguishes load failure from empty registry and allows retry', async () => {
  vi.mocked(bridge.getMcpServers).mockRejectedValueOnce(new Error('registry unavailable'));
  render(<McpPanel />);
  expect(await screen.findByRole('alert')).toHaveTextContent('加载 MCP 列表失败');
  expect(screen.queryByText('暂无可用 MCP 服务器')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '重试加载' }));
  await screen.findByText('ragflow');
  expect(bridge.getMcpServers).toHaveBeenCalledTimes(2);
});

it('shows an empty registry', async () => {
  vi.mocked(bridge.getMcpServers).mockResolvedValue([]); render(<McpPanel />);
  await screen.findByText('暂无可用 MCP 服务器');
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
});

it('retains stale state and blocks edits after failed refresh', async () => {
  await mount(); vi.mocked(bridge.getMcpServers).mockRejectedValueOnce(new Error('read denied'));
  fireEvent.click(screen.getByRole('button', { name: '刷新配置' })); await screen.findByRole('alert');
  expect(checkbox('Codex')).toBeChecked(); expect(checkbox('Codex')).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '重试加载' }));
  await waitFor(() => expect(checkbox('Codex')).toBeEnabled());
});

it('keeps demo read-only without preview or apply', async () => {
  vi.mocked(bridge.runtimeMode).mockReturnValue('demo'); await mount();
  expect(checkbox('Codex')).toBeDisabled(); fireEvent.click(checkbox('OpenCode'));
  expect(bridge.previewMcpToggle).not.toHaveBeenCalled(); expect(bridge.applyMcpTogglePreview).not.toHaveBeenCalled();
});

it('renders stdio commands and unsupported targets without invented controls', async () => {
  vi.mocked(bridge.getMcpServers).mockResolvedValue([{ name: 'tools', description: 'Tools', kind: 'stdio', command: 'tool-server', args: ['--stdio'], supportedAgents: [], enabledAgents: [] }]);
  render(<McpPanel />); await screen.findByText('tools');
  expect(screen.getByText('tool-server --stdio')).toBeInTheDocument();
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
});

it('reports unavailable backend and disables reads and writes', async () => {
  vi.mocked(bridge.runtimeMode).mockReturnValue('unavailable');
  vi.mocked(bridge.getMcpServers).mockRejectedValue(new Error('未连接桌面后端')); render(<McpPanel />);
  expect(await screen.findByRole('alert')).toHaveTextContent('未连接桌面后端');
  expect(screen.getByRole('button', { name: '刷新配置' })).toBeDisabled();
  expect(bridge.previewMcpToggle).not.toHaveBeenCalled(); expect(bridge.applyMcpTogglePreview).not.toHaveBeenCalled();
});
