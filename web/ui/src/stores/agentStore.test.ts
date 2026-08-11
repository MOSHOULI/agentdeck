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
