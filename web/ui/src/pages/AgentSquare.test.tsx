import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { AgentSquare } from './AgentSquare';
import { useAgentStore } from '../stores/agentStore';
import type { AgentInfo } from '../bridge';
import { notDetectedAgent as base, installedAgent, agentCatalog as agents, ambiguousAgent, npmMatchedAgent } from '../test-fixtures/agents';

vi.mock('../bridge', async (importOriginal) => ({
  ...await importOriginal<typeof import('../bridge')>(),
  runtimeMode: vi.fn(() => 'native'),
  getAgents: vi.fn(), installAgent: vi.fn(), configureAgent: vi.fn(), uninstallAgent: vi.fn(),
  verifyAgentAuth: vi.fn(), collectMemory: vi.fn(), searchMemory: vi.fn(),
  injectMemory: vi.fn(), toggleAutoInject: vi.fn(),
  listApiKeys: vi.fn(), listBackups: vi.fn(), getMcpServers: vi.fn(), checkUpdates: vi.fn(),
}));
import * as bridge from '../bridge';

describe('AgentSquare', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.mocked(bridge.installAgent).mockReset().mockResolvedValue({ warnings: [] });
    vi.mocked(bridge.configureAgent).mockReset().mockResolvedValue({ warnings: [] });
    vi.mocked(bridge.getAgents).mockReset().mockResolvedValue(agents);
    vi.mocked(bridge.runtimeMode).mockReturnValue('native');
    useAgentStore.setState({ agents: [], loading: false, error: null, notice: null, refreshError: null, dataStale: true, selectedAgentId: null, memories: {} });
  });

  it('renders the heading, two status filters, search and observed rows only', async () => {
    render(<AgentSquare />);
    expect(screen.getByRole('heading', { name: 'Agent 广场' })).toBeInTheDocument();
    for (const name of ['全部', '已安装']) expect(screen.getByRole('button', { name })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '热度推荐' })).not.toBeInTheDocument();
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
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'OpenCode' } });
    expect(screen.queryByRole('article', { name: 'CLI Tool' })).not.toBeInTheDocument();
    expect(screen.getByText('当前选择：CLI Tool')).toBeInTheDocument();
    expect(bridge.installAgent).not.toHaveBeenCalled();
  });

  it('filters installed records, excluding unknown installation status', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    fireEvent.click(screen.getByRole('button', { name: '已安装' }));
    expect(screen.getAllByRole('article')).toHaveLength(1);
    expect(screen.getByRole('article', { name: 'CLI Tool' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '已安装' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: '全部' }));
    expect(screen.getAllByRole('article')).toHaveLength(3);
  });

  it('keeps all unranked catalog entries visible without an empty recommendation filter', async () => {
    vi.mocked(bridge.getAgents).mockResolvedValue(agents.map((entry) => ({ ...entry, popular: false, stars: 0 })));
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    expect(screen.getAllByRole('article')).toHaveLength(3);
    expect(screen.getByText('显示 3 / 3 款 Agent')).toBeInTheDocument();
    expect(screen.queryByText('暂无热度推荐')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '热度推荐' })).not.toBeInTheDocument();
  });

  it('sorts by name without treating legacy popularity or star counts as recommendations', async () => {
    vi.mocked(bridge.getAgents).mockResolvedValue([
      { ...base, id: 'zulu', name: 'Zulu Agent', popular: true, stars: 999999 },
      { ...base, id: 'alpha', name: 'Alpha Agent', popular: false, stars: 0 },
    ]);
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'Zulu Agent' });
    expect(screen.getAllByRole('article').map((row) => row.getAttribute('aria-label'))).toEqual(['Alpha Agent', 'Zulu Agent']);
    expect(screen.getByText('按名称排序')).toBeInTheDocument();
    expect(screen.queryByText('热度推荐')).not.toBeInTheDocument();
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

  it('keeps old data visible but locked after repeated failed refreshes, then recovers', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('offline')).mockRejectedValueOnce(new Error('still offline'));
    const install = screen.getByRole('button', { name: '安装 OpenCode' });
    const configure = screen.getByRole('button', { name: '应用默认配置 CLI Tool' });
    const refresh = screen.getByRole('button', { name: '刷新列表' });
    fireEvent.click(refresh);
    await screen.findByRole('alert');
    expect(screen.getAllByRole('article')).toHaveLength(3);
    expect(install).toBeDisabled();
    expect(configure).toBeDisabled();
    expect(screen.getByText(/显示上次检测结果，当前状态待刷新/)).toBeInTheDocument();
    fireEvent.click(refresh);
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('still offline'));
    expect(install).toBeDisabled();
    expect(configure).toBeDisabled();
    fireEvent.click(install);
    fireEvent.click(configure);
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    fireEvent.click(refresh);
    await waitFor(() => expect(install).toBeEnabled());
    expect(configure).toBeEnabled();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByText(/显示上次检测结果，当前状态待刷新/)).not.toBeInTheDocument();
  });

  it('reports installation success and refresh failure separately without offering a blind retry', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('refresh offline'));
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('软件安装操作已完成，未应用配置'));
    expect(await screen.findByRole('alert')).toHaveTextContent('状态刷新失败');
    expect(screen.queryByText(/安装失败/)).not.toBeInTheDocument();
    expect(screen.queryByText('无法完成操作')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeDisabled();
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(bridge.getAgents).toHaveBeenCalledTimes(2);
  });

  it('keeps the global install lock when filtering unmounts and remounts its card', async () => {
    let finish!: (outcome: bridge.InstallOutcome) => void;
    vi.mocked(bridge.installAgent).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    fireEvent.click(screen.getByRole('button', { name: '已安装' }));
    expect(screen.queryByRole('article', { name: 'OpenCode' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '全部' }));
    const install = screen.getByRole('button', { name: '安装 OpenCode' });
    expect(install).toBeDisabled();
    fireEvent.click(install);
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(window.confirm).toHaveBeenCalledTimes(1);
    await act(async () => finish({ warnings: [] }));
    await waitFor(() => expect(install).toBeEnabled());
  });

  it('allows configuration only after a separate confirmation, including after installation', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    vi.mocked(bridge.getAgents).mockResolvedValue([installedAgent]);
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    const configure = await screen.findByRole('button', { name: '应用默认配置 OpenCode' });
    await waitFor(() => expect(configure).toBeEnabled());
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    vi.mocked(window.confirm).mockReturnValue(false);
    fireEvent.click(configure);
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    vi.mocked(window.confirm).mockReturnValue(true);
    fireEvent.click(configure);
    await waitFor(() => expect(bridge.configureAgent).toHaveBeenCalledWith('opencode'));
    await waitFor(() => expect(configure).toBeEnabled());
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(bridge.uninstallAgent).not.toHaveBeenCalled();
  });

  it('dismissing an operation error never makes stale state actionable', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    vi.mocked(bridge.installAgent).mockRejectedValueOnce(new Error('installer failed'));
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('refresh failed'));
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    await screen.findByRole('button', { name: '关闭错误提示' });
    await waitFor(() => expect(useAgentStore.getState().loading).toBe(false));
    expect(screen.getAllByRole('alert')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: '关闭错误提示' }));
    expect(screen.getByRole('alert')).toHaveTextContent('状态刷新失败');
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeDisabled();
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

  it('preserves a real partial-configuration notice from the separate configuration operation', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    act(() => useAgentStore.setState({ notice: '默认 profile 配置操作已完成。部分配置未应用：缺少适配器' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('部分配置未应用'));
    expect(screen.queryByText('全部配置成功')).not.toBeInTheDocument();
  });

  it('keeps source evidence readable and configuration blocked across filtering until a fresh result resolves ambiguity', async () => {
    vi.mocked(bridge.getAgents).mockResolvedValue([ambiguousAgent]);
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    const configure = () => screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    expect(configure()).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('检测到 2 个 PATH 候选，其中 1 个为其他候选。')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'does-not-match' } });
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: '' } });
    expect(configure()).toBeDisabled();
    fireEvent.click(configure());
    expect(window.confirm).not.toHaveBeenCalled();
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    vi.mocked(bridge.getAgents).mockResolvedValue([npmMatchedAgent]);
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await waitFor(() => expect(configure()).toBeEnabled());
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('npm 全局目录匹配，安装历史未知')).toBeInTheDocument();
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('synthetic refresh failure'));
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await screen.findByRole('alert');
    expect(configure()).toBeDisabled();
    expect(screen.getByText('npm 全局目录匹配，安装历史未知')).toBeInTheDocument();
    expect(screen.getByText(/显示上次检测结果，当前状态待刷新/)).toBeInTheDocument();
  });

  it('combines capability, installed and search filters, then clears every filter together', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    const capability = screen.getByRole('combobox', { name: '支持能力与状态' });
    fireEvent.change(capability, { target: { value: 'automatic' } });
    expect(screen.getAllByRole('article')).toHaveLength(1);
    expect(screen.getByRole('article', { name: 'OpenCode' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled();
    fireEvent.click(screen.getByRole('button', { name: '已安装' }));
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    expect(screen.getByText('没有符合当前筛选条件的 Agent')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'manual' } });
    expect(screen.getByText('没有匹配的 Agent')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看全部 Agent' }));
    expect(capability).toHaveValue('all');
    expect(screen.getByRole('searchbox')).toHaveValue('');
    expect(screen.getByRole('button', { name: '全部' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getAllByRole('article')).toHaveLength(3);
    expect(bridge.getAgents).toHaveBeenCalledTimes(1);
    expect(bridge.installAgent).not.toHaveBeenCalled();
  });

  it('keeps passive observations visible under manual and uncertain filters without granting actions', async () => {
    const manual = { ...agents[2], id: 'qwen-code', name: 'Qwen Code', description: '被动观察', installationDetails: {
      probe: { status: 'unsupported' as const, message: null }, instances: [{ path: '/synthetic/qwen', active: true, resolvedPath: null, entrypointPath: null, version: null, source: { kind: 'unknown' as const, reason: 'Passive only' } }], ambiguous: false, message: null,
    } };
    vi.mocked(bridge.getAgents).mockResolvedValue([base, manual]);
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'Qwen Code' });
    const capability = screen.getByRole('combobox');
    fireEvent.change(capability, { target: { value: 'manual' } });
    expect(screen.getAllByRole('article')).toHaveLength(1);
    expect(screen.getByRole('article', { name: 'Qwen Code' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '选择 Qwen Code' }));
    fireEvent.change(capability, { target: { value: 'attention' } });
    expect(screen.getByRole('article', { name: 'Qwen Code' })).toBeInTheDocument();
    expect(screen.getByText(/不等于软件损坏/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '已安装' }));
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    expect(screen.getByText('当前选择：Qwen Code')).toBeInTheDocument();
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(bridge.uninstallAgent).not.toHaveBeenCalled();
  });

  it('removes stale automatic candidates until a successful refresh and keeps the selected capability', async () => {
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    const capability = screen.getByRole('combobox');
    fireEvent.change(capability, { target: { value: 'automatic' } });
    expect(screen.getByRole('article', { name: 'OpenCode' })).toBeInTheDocument();
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('offline'));
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await screen.findByRole('alert');
    expect(capability).toHaveValue('automatic');
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    expect(screen.getByText('当前检测状态待刷新，可自动安装列表暂不展示候选。')).toBeInTheDocument();
    fireEvent.change(capability, { target: { value: 'attention' } });
    expect(screen.getAllByRole('article')).toHaveLength(3);
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeDisabled();
    fireEvent.change(capability, { target: { value: 'automatic' } });
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await screen.findByRole('article', { name: 'OpenCode' });
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled();
    expect(capability).toHaveValue('automatic');
    expect(bridge.installAgent).not.toHaveBeenCalled();
  });

  it('does not advertise actionable automatic installation in read-only mode', async () => {
    vi.mocked(bridge.runtimeMode).mockReturnValue('demo');
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'automatic' } });
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    expect(screen.getByText('当前为只读模式。请在桌面应用中刷新检测后查看可自动安装项。')).toBeInTheDocument();
    expect(bridge.installAgent).not.toHaveBeenCalled();
  });

  it('excludes invalid evidence from installed results and surfaces it for review', async () => {
    const invalid = { ...installedAgent, installationDetails: { probe: { status: 'not_detected' as const, message: null }, instances: [], ambiguous: false, message: null } };
    vi.mocked(bridge.getAgents).mockResolvedValue([invalid]);
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    fireEvent.click(screen.getByRole('button', { name: '已安装' }));
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '全部' }));
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'attention' } });
    expect(screen.getByRole('article', { name: 'OpenCode' })).toBeInTheDocument();
    expect(screen.getByText('安装状态未知')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '应用默认配置 OpenCode' })).toBeDisabled();
    expect(bridge.configureAgent).not.toHaveBeenCalled();
  });


  it('preserves cancellation and the operation lock while automatic candidates disappear during installation', async () => {
    let finish!: (value: bridge.InstallOutcome) => void;
    vi.mocked(bridge.installAgent).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    render(<AgentSquare />);
    await screen.findByRole('article', { name: 'OpenCode' });
    const capability = screen.getByRole('combobox');
    fireEvent.change(capability, { target: { value: 'automatic' } });
    vi.mocked(window.confirm).mockReturnValueOnce(false);
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(capability).toHaveValue('automatic');
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    fireEvent.change(capability, { target: { value: 'all' } });
    const lockedInstall = screen.getByRole('button', { name: '安装 OpenCode' });
    expect(lockedInstall).toBeDisabled();
    fireEvent.click(lockedInstall);
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    fireEvent.change(capability, { target: { value: 'automatic' } });
    vi.mocked(bridge.getAgents).mockResolvedValue([installedAgent]);
    await act(async () => finish({ warnings: [] }));
    await waitFor(() => expect(useAgentStore.getState().loading).toBe(false));
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    expect(screen.getByText('没有符合当前筛选条件的 Agent')).toBeInTheDocument();
    fireEvent.change(capability, { target: { value: 'all' } });
    expect(screen.getByRole('button', { name: '应用默认配置 OpenCode' })).toBeEnabled();
    expect(bridge.configureAgent).not.toHaveBeenCalled();
  });

});
