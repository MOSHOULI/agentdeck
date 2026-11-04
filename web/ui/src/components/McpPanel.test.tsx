import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { McpPanel } from './McpPanel';
import type { AgentInfo, McpServer } from '../bridge';
import { useAgentStore } from '../stores/agentStore';

vi.mock('../bridge', () => ({
  runtimeMode: vi.fn(() => 'native'),
  getMcpServers: vi.fn(),
  toggleMcpForAgent: vi.fn(),
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
  vi.mocked(bridge.toggleMcpForAgent).mockResolvedValue();
});

describe('McpPanel', () => {


  it('展示服务器与按 agent 开关状态', async () => {
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText('ragflow')).toBeInTheDocument());
    const codex = screen.getByRole('checkbox', { name: 'ragflow 的 Codex 配置' }) as HTMLInputElement;
    const opencode = screen.getByRole('checkbox', { name: 'ragflow 的 OpenCode 配置' }) as HTMLInputElement;
    expect(codex.checked).toBe(true);
    expect(opencode.checked).toBe(false);
  });

  it('切换开关调用后端并乐观更新', async () => {
    vi.mocked(bridge.toggleMcpForAgent).mockResolvedValue(undefined);
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText('ragflow')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('checkbox', { name: 'ragflow 的 OpenCode 配置' }));
    await waitFor(() =>
      expect(bridge.toggleMcpForAgent).toHaveBeenCalledWith('ragflow', 'opencode', true)
    );
    expect((screen.getByRole('checkbox', { name: 'ragflow 的 OpenCode 配置' }) as HTMLInputElement).checked).toBe(true);
  });

  it('切换失败时回滚开关', async () => {
    vi.mocked(bridge.toggleMcpForAgent).mockRejectedValue(new Error('boom'));
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText('ragflow')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('checkbox', { name: 'ragflow 的 OpenCode 配置' }));
    await waitFor(() => expect(screen.getByText(/切换失败/)).toBeInTheDocument());
    expect((screen.getByRole('checkbox', { name: 'ragflow 的 OpenCode 配置' }) as HTMLInputElement).checked).toBe(false);
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
const checkbox = (name: string) => screen.getByRole('checkbox', { name: `ragflow 的 ${name} 配置` });

it('only renders supported Agent configuration targets without claiming server health', async () => {
  useAgentStore.setState({ agents: [...agents, { ...agents[0], id: 'unsupported', name: 'Unsupported Agent' }] });
  render(<McpPanel />);
  await screen.findByRole('heading', { name: 'ragflow' });
  expect(screen.getAllByRole('checkbox')).toHaveLength(2);
  expect(screen.queryByRole('checkbox', { name: /Unsupported/ })).not.toBeInTheDocument();
  expect(screen.getByRole('group', { name: '包含此服务的 Agent 配置' })).toBeInTheDocument();
  expect(screen.getByText('开关代表配置包含状态')).toBeInTheDocument();
  expect(screen.getByText(/此处不检测服务器是否在线/)).toBeInTheDocument();
  expect(screen.getByText('HTTP')).toBeInTheDocument();
});

it('guards repeated toggles and refresh while a write is pending', async () => {
  const pending = deferred<void>();
  vi.mocked(bridge.toggleMcpForAgent).mockReturnValue(pending.promise);
  render(<McpPanel />);
  await screen.findByText('ragflow');
  fireEvent.click(checkbox('OpenCode'));
  fireEvent.click(checkbox('OpenCode'));
  fireEvent.click(checkbox('Codex'));
  fireEvent.click(screen.getByRole('button', { name: '刷新配置' }));
  expect(bridge.toggleMcpForAgent).toHaveBeenCalledTimes(1);
  expect(bridge.getMcpServers).toHaveBeenCalledTimes(1);
  expect(checkbox('Codex')).toBeDisabled();
  expect(checkbox('OpenCode')).toBeChecked();
  expect(screen.getByText('正在写入')).toBeInTheDocument();
  await act(async () => pending.resolve());
  expect(checkbox('Codex')).toBeEnabled();
  expect(screen.getByRole('status')).toHaveTextContent('配置状态不代表服务已启动或连接成功');
});

it('also restores an existing inclusion when removing it fails', async () => {
  vi.mocked(bridge.toggleMcpForAgent).mockRejectedValue(new Error('file locked'));
  render(<McpPanel />);
  await screen.findByText('ragflow');
  fireEvent.click(checkbox('Codex'));
  expect(await screen.findByRole('alert')).toHaveClass('notice-error');
  expect(checkbox('Codex')).toBeChecked();
  expect(bridge.toggleMcpForAgent).toHaveBeenCalledWith('ragflow', 'codex', false);
});

it('makes a load failure distinct from an empty registry and supports retry', async () => {
  vi.mocked(bridge.getMcpServers).mockRejectedValueOnce(new Error('registry unavailable'));
  render(<McpPanel />);
  expect(await screen.findByRole('alert')).toHaveTextContent('加载 MCP 列表失败');
  expect(screen.getByText('暂时无法读取 MCP 配置')).toBeInTheDocument();
  expect(screen.queryByText('暂无可用 MCP 服务器')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '重试加载' }));
  await screen.findByText('ragflow');
  expect(bridge.getMcpServers).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

it('shows an empty state when no registered services are returned', async () => {
  vi.mocked(bridge.getMcpServers).mockResolvedValue([]);
  render(<McpPanel />);
  await screen.findByText('暂无可用 MCP 服务器');
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '刷新配置' })).toBeEnabled();
});

it('retains last read on refresh failure and blocks editing until a successful refresh', async () => {
  render(<McpPanel />);
  await screen.findByText('ragflow');
  vi.mocked(bridge.getMcpServers).mockRejectedValueOnce(new Error('read denied'));
  fireEvent.click(screen.getByRole('button', { name: '刷新配置' }));
  await screen.findByRole('alert');
  expect(screen.getByText(/以下保留上次成功读取的配置/)).toBeInTheDocument();
  expect(checkbox('Codex')).toBeChecked();
  expect(checkbox('Codex')).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '重试加载' }));
  await waitFor(() => expect(checkbox('Codex')).toBeEnabled());
});

it('keeps demo configuration read-only without calling mutation APIs', async () => {
  vi.mocked(bridge.runtimeMode).mockReturnValue('demo');
  render(<McpPanel />);
  await screen.findByText('ragflow');
  expect(checkbox('Codex')).toBeDisabled();
  expect(checkbox('OpenCode')).toBeDisabled();
  fireEvent.click(checkbox('OpenCode'));
  expect(bridge.toggleMcpForAgent).not.toHaveBeenCalled();
  expect(screen.getByText('只读视图，配置修改不可用')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '刷新配置' })).toBeEnabled();
});

it('describes removal as configuration removal, not process shutdown', async () => {
  render(<McpPanel />);
  await screen.findByText('ragflow');
  fireEvent.click(checkbox('Codex'));
  expect(await screen.findByRole('status')).toHaveTextContent('此操作不会停止服务进程');
  expect(checkbox('Codex')).not.toBeChecked();
});

it('renders stdio commands and unsupported-target state without invented controls', async () => {
  vi.mocked(bridge.getMcpServers).mockResolvedValue([{ name: 'local-tools', description: 'Local tools', kind: 'stdio', command: 'tool-server', args: ['--stdio'], supportedAgents: [], enabledAgents: [] }]);
  render(<McpPanel />);
  await screen.findByText('local-tools');
  expect(screen.getByText('STDIO')).toBeInTheDocument();
  expect(screen.getByText('tool-server --stdio')).toBeInTheDocument();
  expect(screen.getByText('此服务尚无受支持的 Agent 配置目标。')).toBeInTheDocument();
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
});

it('reports an unavailable backend and disables reads and writes instead of showing sample services', async () => {
  vi.mocked(bridge.runtimeMode).mockReturnValue('unavailable');
  vi.mocked(bridge.getMcpServers).mockRejectedValue(new Error('未连接桌面后端'));
  render(<McpPanel />);
  expect(await screen.findByRole('alert')).toHaveTextContent('未连接桌面后端');
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '刷新配置' })).toBeDisabled();
  expect(screen.getByRole('button', { name: '重试加载' })).toBeDisabled();
  expect(bridge.toggleMcpForAgent).not.toHaveBeenCalled();
});
