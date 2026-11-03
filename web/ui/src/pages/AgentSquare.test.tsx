import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { AgentSquare } from './AgentSquare';
import { useAgentStore } from '../stores/agentStore';
import type { AgentInfo } from '../bridge';

vi.mock('../bridge', () => ({
  runtimeMode: vi.fn(() => 'native'),
  getAgents: vi.fn(), installAgent: vi.fn(), uninstallAgent: vi.fn(),
  verifyAgentAuth: vi.fn(), collectMemory: vi.fn(), searchMemory: vi.fn(),
  injectMemory: vi.fn(), toggleAutoInject: vi.fn(),
  listApiKeys: vi.fn(), listBackups: vi.fn(), getMcpServers: vi.fn(), checkUpdates: vi.fn(),
}));
import * as bridge from '../bridge';

const base: AgentInfo = {
  id: 'opencode', name: 'OpenCode', installed: false,
  canInstall: true, canUninstall: true, canVerifyInstall: true,
  authVerified: false, source: 'open', uiType: 'all', stars: 194000,
  popular: true, description: '开源 AI 编程代理', icon: 'opencode', category: 'ai-coding',
};
const agents: AgentInfo[] = [
  base,
  { ...base, id: 'cli-tool', name: 'CLI Tool', installed: true, version: '1.0.0', uiType: 'cli', stars: 100, popular: false, description: '终端工具' },
  { ...base, id: 'manual', name: 'Manual Agent', uiType: 'vscode-extension', canInstall: false, canVerifyInstall: false, stars: 200, popular: false, description: '手动管理扩展' },
];

describe('AgentSquare', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(bridge.getAgents).mockReset().mockResolvedValue(agents);
    vi.mocked(bridge.runtimeMode).mockReturnValue('native');
    useAgentStore.setState({ agents: [], loading: false, error: null, notice: null, selectedAgentId: null, memories: {} });
  });

  it('renders the heading, three filters, search and observed rows only', async () => {
    render(<AgentSquare />);
    expect(screen.getByRole('heading', { name: 'Agent 广场' })).toBeInTheDocument();
    for (const name of ['全部', '热度推荐', '已安装']) expect(screen.getByRole('button', { name })).toBeInTheDocument();
    expect(screen.getByRole('searchbox', { name: '搜索 Agent' })).toBeInTheDocument();
    await screen.findByRole('article', { name: 'OpenCode' });
    expect(screen.getAllByRole('article')).toHaveLength(3);
    expect(screen.getByText('显示 3 / 3 款 Agent')).toBeInTheDocument();
    expect(screen.getByText('安装状态未知')).toBeInTheDocument();
  });

  it('never mounts other management workspaces or loads their data', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    for (const read of [bridge.listApiKeys, bridge.listBackups, bridge.getMcpServers, bridge.checkUpdates, bridge.collectMemory]) expect(read).not.toHaveBeenCalled();
  });

  it('selects an agent without starting an install and preserves selection while filtering', async () => {
    render(<AgentSquare />);
    const select = await screen.findByRole('button', { name: '选择 CLI Tool' });
    fireEvent.click(select);
    expect(useAgentStore.getState().selectedAgentId).toBe('cli-tool');
    expect(select).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText('当前选择：CLI Tool')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '热度推荐' }));
    expect(screen.queryByRole('article', { name: 'CLI Tool' })).not.toBeInTheDocument();
    expect(screen.getByText('当前选择：CLI Tool')).toBeInTheDocument();
    expect(bridge.installAgent).not.toHaveBeenCalled();
  });

  it('filters popular and installed records, excluding unknown installation status', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    fireEvent.click(screen.getByRole('button', { name: '热度推荐' }));
    expect(screen.getAllByRole('article')).toHaveLength(1);
    expect(screen.getByRole('article', { name: 'OpenCode' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '已安装' }));
    expect(screen.getAllByRole('article')).toHaveLength(1);
    expect(screen.getByRole('article', { name: 'CLI Tool' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '已安装' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: '全部' }));
    expect(screen.getAllByRole('article')).toHaveLength(3);
  });

  it('searches actual names, IDs and descriptions locally with active filters', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    const search = screen.getByRole('searchbox');
    fireEvent.change(search, { target: { value: '  OPENcode  ' } });
    expect(screen.getAllByRole('article')).toHaveLength(1);
    expect(screen.getByRole('article', { name: 'OpenCode' })).toBeInTheDocument();
    fireEvent.change(search, { target: { value: 'cli-tool' } });
    expect(screen.getByRole('article', { name: 'CLI Tool' })).toBeInTheDocument();
    fireEvent.change(search, { target: { value: '手动管理' } });
    expect(screen.getByRole('article', { name: 'Manual Agent' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '已安装' }));
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    expect(screen.getByText('没有匹配的 Agent')).toBeInTheDocument();
    expect(bridge.getAgents).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: '查看全部 Agent' }));
    expect(search).toHaveValue('');
    expect(screen.getAllByRole('article')).toHaveLength(3);
  });

  it('shows a truthful empty installed filter and a recovery action', async () => {
    vi.mocked(bridge.getAgents).mockResolvedValue([base]);
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    fireEvent.click(screen.getByRole('button', { name: '已安装' }));
    expect(screen.getByText('还没有检测到已安装的 Agent')).toBeInTheDocument();
    expect(screen.getByText('无法自动检测的 Agent 不会计入已安装列表。')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看全部 Agent' }));
    expect(screen.getByRole('article', { name: 'OpenCode' })).toBeInTheDocument();
  });

  it('keeps load errors distinct from an empty catalog and can retry', async () => {
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('boom'));
    render(<AgentSquare />);
    expect(await screen.findByRole('alert')).toHaveTextContent('加载 Agent 列表失败');
    expect(screen.getByText('暂时无法读取 Agent 列表')).toBeInTheDocument();
    expect(screen.queryByText('暂无可显示的 Agent')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重新加载' }));
    await screen.findByRole('article', { name: 'OpenCode' });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(bridge.getAgents).toHaveBeenCalledTimes(2);
  });

  it('retains observed data after refresh failure and dismisses the error', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('offline'));
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await screen.findByRole('alert');
    expect(screen.getAllByRole('article')).toHaveLength(3);
    fireEvent.click(screen.getByRole('button', { name: '关闭错误提示' }));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows initial loading separately and prevents repeated refresh requests', async () => {
    let finish!: (value: AgentInfo[]) => void;
    vi.mocked(bridge.getAgents).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    render(<AgentSquare />);
    expect(screen.getByRole('status')).toHaveTextContent('正在读取 Agent 状态');
    expect(screen.getByText('正在加载 Agent')).toBeInTheDocument();
    expect(screen.queryByText('暂无可显示的 Agent')).not.toBeInTheDocument();
    const refresh = screen.getByRole('button', { name: '刷新列表' });
    expect(refresh).toBeDisabled();
    fireEvent.click(refresh);
    expect(bridge.getAgents).toHaveBeenCalledTimes(1);
    await act(async () => finish(agents));
    expect(screen.getAllByRole('article')).toHaveLength(3);
    expect(refresh).toBeEnabled();
  });

  it('preserves a real partial-configuration notice', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    act(() => useAgentStore.setState({ notice: '软件已安装；部分配置未应用：缺少适配器' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('部分配置未应用'));
    expect(screen.queryByText('全部配置成功')).not.toBeInTheDocument();
  });
});
