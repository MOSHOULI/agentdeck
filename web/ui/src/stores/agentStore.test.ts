import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAgentStore } from './agentStore';
import * as bridge from '../bridge';
import { agentCatalog, installedAgent, notDetectedAgent } from '../test-fixtures/agents';

vi.mock('../bridge', () => ({
  getAgents: vi.fn(), installAgent: vi.fn(), configureAgent: vi.fn(), uninstallAgent: vi.fn(),
  verifyAgentAuth: vi.fn(), collectMemory: vi.fn(), searchMemory: vi.fn(),
  injectMemory: vi.fn(), toggleAutoInject: vi.fn(),
}));

function seedFresh(agents = agentCatalog) {
  useAgentStore.setState({ agents, dataStale: false });
}

describe('agentStore', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useAgentStore.setState({ agents: [], loading: false, error: null, notice: null, refreshError: null, dataStale: true });
    vi.mocked(bridge.getAgents).mockResolvedValue(agentCatalog);
    vi.mocked(bridge.installAgent).mockResolvedValue({ warnings: [] });
    vi.mocked(bridge.configureAgent).mockResolvedValue({ warnings: [] });
  });

  it('starts stale and only a successful read makes the list fresh', async () => {
    expect(useAgentStore.getInitialState().dataStale).toBe(true);
    await useAgentStore.getState().loadAgents();
    expect(useAgentStore.getState()).toMatchObject({ agents: agentCatalog, loading: false, dataStale: false, refreshError: null });
  });

  it('keeps failed loads separate from operation errors', async () => {
    vi.mocked(bridge.getAgents).mockRejectedValue(new Error('boom'));
    await useAgentStore.getState().loadAgents();
    expect(useAgentStore.getState().refreshError).toContain('加载 Agent 列表失败');
    expect(useAgentStore.getState()).toMatchObject({ error: null, agents: [], loading: false, dataStale: true });
  });

  it('installs software without applying configuration or inventing a version', async () => {
    seedFresh();
    vi.mocked(bridge.getAgents).mockResolvedValueOnce([installedAgent]);
    await useAgentStore.getState().installAgent('opencode');
    expect(bridge.installAgent).toHaveBeenCalledWith('opencode');
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(useAgentStore.getState().notice).toContain('软件安装操作已完成，未应用配置');
    expect(useAgentStore.getState().agents[0].version).toBeUndefined();
    expect(useAgentStore.getState()).toMatchObject({ error: null, refreshError: null, dataStale: false });
  });

  it('applies configuration through a separate command and preserves warnings', async () => {
    seedFresh([installedAgent]);
    vi.mocked(bridge.configureAgent).mockResolvedValueOnce({ warnings: ['model_config: no adapter'] });
    vi.mocked(bridge.getAgents).mockResolvedValueOnce([installedAgent]);
    await useAgentStore.getState().configureAgent('opencode');
    expect(bridge.configureAgent).toHaveBeenCalledWith('opencode');
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(useAgentStore.getState().notice).toContain('默认 profile 配置操作已完成');
    expect(useAgentStore.getState().notice).toContain('model_config: no adapter');
    expect(useAgentStore.getState().error).toBeNull();
  });

  it.each(['error', 'unsupported', undefined] as const)('refuses to install when detection is %s despite declared install capability', async (installationStatus) => {
    seedFresh([{ ...notDetectedAgent, installationStatus }]);
    await useAgentStore.getState().installAgent('opencode');
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('当前状态不允许自动安装');
  });

  it.each([
    notDetectedAgent,
    { ...installedAgent, canConfigure: false },
    { ...installedAgent, canConfigure: undefined },
    { ...installedAgent, installationStatus: undefined },
    { ...installedAgent, installationStatus: 'error' as const },
  ])('refuses configuration without an installed, supported, fresh target (%j)', async (agent) => {
    seedFresh([agent]);
    await useAgentStore.getState().configureAgent(agent.id);
    expect(bridge.configureAgent).not.toHaveBeenCalled();
  });

  it.each(['unknown', undefined] as const)('never uninstalls an unverified origin (%s), even with legacy canUninstall=true', async (installSource) => {
    seedFresh([{ ...installedAgent, installSource, canUninstall: true }]);
    await useAgentStore.getState().uninstallAgent('opencode');
    expect(bridge.uninstallAgent).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('安装来源未验证');
  });

  it('serializes installation, configuration, uninstall and refresh before React rerenders', async () => {
    seedFresh();
    let finish!: (outcome: bridge.InstallOutcome) => void;
    vi.mocked(bridge.installAgent).mockImplementationOnce(() => new Promise((done) => { finish = done; }));
    const first = useAgentStore.getState().installAgent('opencode');
    await useAgentStore.getState().installAgent('opencode');
    await useAgentStore.getState().configureAgent('cli-tool');
    await useAgentStore.getState().uninstallAgent('cli-tool');
    await useAgentStore.getState().loadAgents();
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(bridge.uninstallAgent).not.toHaveBeenCalled();
    expect(bridge.getAgents).not.toHaveBeenCalled();
    finish({ warnings: [] });
    await first;
    expect(useAgentStore.getState().loading).toBe(false);
    expect(bridge.getAgents).toHaveBeenCalledTimes(1);
  });

  it('refreshes after an installation error without claiming software was rolled back', async () => {
    seedFresh();
    vi.mocked(bridge.installAgent).mockRejectedValueOnce(new Error('installer exited'));
    vi.mocked(bridge.getAgents).mockResolvedValueOnce([installedAgent]);
    await useAgentStore.getState().installAgent('opencode');
    expect(useAgentStore.getState().agents[0].installed).toBe(true);
    expect(useAgentStore.getState().error).toContain('软件可能已安装');
    expect(useAgentStore.getState().error).toContain('installer exited');
    expect(useAgentStore.getState()).toMatchObject({ loading: false, dataStale: false });
  });

  it('reports successful install plus failed refresh as completed but stale, without retrying either operation', async () => {
    seedFresh();
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('offline'));
    await useAgentStore.getState().installAgent('opencode');
    expect(useAgentStore.getState().notice).toContain('软件安装操作已完成');
    expect(useAgentStore.getState().error).toBeNull();
    expect(useAgentStore.getState().refreshError).toContain('offline');
    expect(useAgentStore.getState()).toMatchObject({ dataStale: true, loading: false, agents: agentCatalog });
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(bridge.getAgents).toHaveBeenCalledTimes(1);
    await useAgentStore.getState().installAgent('opencode');
    await useAgentStore.getState().configureAgent('cli-tool');
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(bridge.configureAgent).not.toHaveBeenCalled();
  });

  it('preserves the actual operation failure separately from repeated refresh failures', async () => {
    seedFresh();
    vi.mocked(bridge.installAgent).mockRejectedValueOnce(new Error('real install error'));
    vi.mocked(bridge.getAgents).mockRejectedValue(new Error('read error'));
    await useAgentStore.getState().installAgent('opencode');
    await useAgentStore.getState().loadAgents();
    await useAgentStore.getState().installAgent('opencode');
    expect(useAgentStore.getState().error).toContain('real install error');
    expect(useAgentStore.getState().error).not.toContain('read error');
    expect(useAgentStore.getState().refreshError).toContain('read error');
    expect(useAgentStore.getState().dataStale).toBe(true);
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(bridge.getAgents).toHaveBeenCalledTimes(2);
  });

  it('keeps old data disabled after a second refresh failure and re-enables only after a successful read', async () => {
    seedFresh();
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('offline')).mockRejectedValueOnce(new Error('still offline'));
    await useAgentStore.getState().loadAgents();
    await useAgentStore.getState().loadAgents();
    expect(useAgentStore.getState()).toMatchObject({ agents: agentCatalog, dataStale: true });
    await useAgentStore.getState().installAgent('opencode');
    expect(bridge.installAgent).not.toHaveBeenCalled();
    await useAgentStore.getState().loadAgents();
    expect(useAgentStore.getState()).toMatchObject({ dataStale: false, refreshError: null });
    await useAgentStore.getState().installAgent('opencode');
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
  });

  it('also keeps a completed configure operation distinct from refresh failure', async () => {
    seedFresh([installedAgent]);
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('offline'));
    await useAgentStore.getState().configureAgent('opencode');
    expect(useAgentStore.getState().notice).toContain('默认 profile 配置操作已完成');
    expect(useAgentStore.getState()).toMatchObject({ error: null, dataStale: true });
    expect(useAgentStore.getState().refreshError).toContain('offline');
    expect(bridge.configureAgent).toHaveBeenCalledTimes(1);
  });
});
