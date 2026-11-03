import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAgentStore } from './agentStore';
import * as bridge from '../bridge';
import type { AgentInfo } from '../bridge';

vi.mock('../bridge', () => ({
  getAgents: vi.fn(),
  installAgent: vi.fn(),
  uninstallAgent: vi.fn(),
  verifyAgentAuth: vi.fn(),
  collectMemory: vi.fn(),
  searchMemory: vi.fn(),
  injectMemory: vi.fn(),
  toggleAutoInject: vi.fn(),
}));

const agents: AgentInfo[] = [
  {
    id: 'opencode',
    name: 'OpenCode',
    installed: false,
    canInstall: true,
    canUninstall: true,
    authVerified: false,
    source: 'open',
    uiType: 'all',
    stars: 194000,
    popular: true,
    description: '开源 AI 编程代理',
    icon: 'opencode',
    category: 'ai-coding',
  },
  {
    id: 'cli-tool',
    name: 'CLI Tool',
    installed: false,
    canInstall: true,
    canUninstall: true,
    authVerified: false,
    source: 'open',
    uiType: 'cli',
    stars: 100,
    popular: false,
    description: '终端工具',
    icon: 'cli-tool',
    category: 'ai-coding',
  },
];

describe('agentStore', () => {
  beforeEach(() => {
    useAgentStore.setState({ agents: [], loading: false, error: null });
    vi.mocked(bridge.getAgents).mockReset();
  });

  it('loadAgents 成功填充列表', async () => {
    vi.mocked(bridge.getAgents).mockResolvedValue(agents);
    await useAgentStore.getState().loadAgents();
    expect(useAgentStore.getState().agents).toHaveLength(2);
    expect(useAgentStore.getState().loading).toBe(false);
    expect(useAgentStore.getState().error).toBeNull();
  });

  it('loadAgents 失败写入 error', async () => {
    vi.mocked(bridge.getAgents).mockRejectedValue(new Error('boom'));
    await useAgentStore.getState().loadAgents();
    expect(useAgentStore.getState().error).toContain('加载 Agent 列表失败');
    expect(useAgentStore.getState().agents).toHaveLength(0);
    expect(useAgentStore.getState().loading).toBe(false);
  });
});

it('serializes installer calls even before a React rerender', async () => {
  useAgentStore.setState({ loading: false });
  let resolve!: (outcome: bridge.InstallOutcome) => void;
  vi.mocked(bridge.installAgent).mockReset().mockImplementation(() => new Promise((done) => { resolve = done; }));
  vi.mocked(bridge.getAgents).mockResolvedValue(agents);
  const first = useAgentStore.getState().installAgent('opencode');
  await useAgentStore.getState().installAgent('opencode');
  await useAgentStore.getState().uninstallAgent('opencode');
  expect(bridge.installAgent).toHaveBeenCalledTimes(1);
  resolve({ warnings: [] });
  await first;
  expect(useAgentStore.getState().loading).toBe(false);
});

it('refreshes installed state after a configuration failure without claiming rollback', async () => {
  useAgentStore.setState({ agents, loading: false, error: null });
  vi.mocked(bridge.installAgent).mockRejectedValueOnce(new Error('configuration write failed'));
  vi.mocked(bridge.getAgents).mockResolvedValueOnce([{ ...agents[0], installed: true }]);
  await useAgentStore.getState().installAgent('opencode');
  expect(useAgentStore.getState().agents[0].installed).toBe(true);
  expect(useAgentStore.getState().error).toContain('软件可能已安装');
  expect(useAgentStore.getState().error).toContain('configuration write failed');
  expect(useAgentStore.getState().loading).toBe(false);
});

it('surfaces skipped configuration warnings without calling installation failed', async () => {
  useAgentStore.setState({ loading: false, error: null, notice: null });
  vi.mocked(bridge.installAgent).mockResolvedValueOnce({ warnings: ['model_config: no adapter'] });
  vi.mocked(bridge.getAgents).mockResolvedValueOnce([{ ...agents[0], installed: true }]);
  await useAgentStore.getState().installAgent('opencode');
  expect(useAgentStore.getState().error).toBeNull();
  expect(useAgentStore.getState().notice).toContain('model_config: no adapter');
  expect(useAgentStore.getState().notice).toContain('软件已安装');
});
