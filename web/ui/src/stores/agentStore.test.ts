import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAgentStore } from './agentStore';
import * as bridge from '../bridge';
import { agentCatalog, installedAgent, notDetectedAgent, ambiguousAgent, npmMatchedAgent, activeInstance } from '../test-fixtures/agents';

vi.mock('../bridge', async (importOriginal) => ({
  ...await importOriginal<typeof import('../bridge')>(),
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
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    expect(bridge.installAgent).toHaveBeenCalledWith('opencode', 'a'.repeat(64));
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
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
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

  it('blocks all uninstall management even when complete npm source evidence and forged capabilities are present', async () => {
    seedFresh([{ ...npmMatchedAgent, canUninstall: true }]);
    await useAgentStore.getState().uninstallAgent('opencode');
    expect(bridge.uninstallAgent).not.toHaveBeenCalled();
    expect(bridge.getAgents).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('自动卸载功能尚未提供');
    expect(useAgentStore.getState().error).toContain('来源目录匹配不代表安装历史或管理权限');
    expect(useAgentStore.getState().loading).toBe(false);
  });

  it('rejects ambiguous configuration even if the caller bypasses the disabled card', async () => {
    seedFresh([ambiguousAgent]);
    await useAgentStore.getState().configureAgent('opencode');
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(bridge.getAgents).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('PATH 候选优先级存在歧义');
    expect(useAgentStore.getState().dataStale).toBe(false);
  });

  it.each([
    null, {}, { instances: null }, { instances: [null], ambiguous: false },
    { instances: [activeInstance, { ...activeInstance, path: '/synthetic/duplicate' }], ambiguous: false },
  ])('blocks configuration on inconsistent or incomplete instance details (%j)', async (installationDetails) => {
    seedFresh([{ ...installedAgent, installationDetails } as bridge.AgentInfo]);
    await useAgentStore.getState().configureAgent('opencode');
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('已暂停安装与应用配置');
  });

  it('permits separately requested configuration only after a fresh read removes the ambiguity', async () => {
    seedFresh([ambiguousAgent]);
    await useAgentStore.getState().configureAgent('opencode');
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    vi.mocked(bridge.getAgents).mockResolvedValue([npmMatchedAgent]);
    await useAgentStore.getState().loadAgents();
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    await useAgentStore.getState().configureAgent('opencode');
    expect(bridge.configureAgent).toHaveBeenCalledTimes(1);
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(bridge.uninstallAgent).not.toHaveBeenCalled();
  });

  it.each([undefined, null, {}, { status: 'error' }, { status: 'not_detected' }, { status: 'unsupported' }, { status: 'forged' }, { status: 42 }])('rejects direct configuration with an invalid or contradictory probe (%j)', async (probe) => {
    seedFresh([{ ...npmMatchedAgent, installationDetails: {
      ...npmMatchedAgent.installationDetails!, probe,
    } } as unknown as bridge.AgentInfo]);
    await useAgentStore.getState().configureAgent('opencode');
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(bridge.getAgents).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('安装检测证据不完整或状态不一致');
    expect(useAgentStore.getState().loading).toBe(false);
  });

  it.each([undefined, null, {}, { status: 'error' }, { status: 'installed' }, { status: 'unsupported' }, { status: 'forged' }])('rejects direct installation with an invalid or contradictory probe (%j)', async (probe) => {
    seedFresh([{ ...notDetectedAgent, installationDetails: {
      ...installedAgent.installationDetails!, probe,
    } } as unknown as bridge.AgentInfo]);
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(bridge.getAgents).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('安装检测证据不完整或状态不一致');
  });

  it.each([
    { installed: false, installationStatus: 'installed', probeStatus: 'installed' },
    { installed: true, installationStatus: 'not_detected', probeStatus: 'not_detected' },
  ] as const)('rejects all writes when status strings agree but the installed boolean contradicts them (%j)', async ({ installed, installationStatus, probeStatus }) => {
    seedFresh([{ ...installedAgent, installed, installationStatus, installationDetails: {
      ...installedAgent.installationDetails!, probe: { status: probeStatus, message: null },
    } }]);
    await useAgentStore.getState().configureAgent('opencode');
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('安装检测证据不完整或状态不一致');
  });

  it.each(['install', 'configure'] as const)('blocks %s when nonempty observations have no active candidate', async (operation) => {
    const target = operation === 'install' ? notDetectedAgent : installedAgent;
    seedFresh([{ ...target, installationDetails: {
      probe: { status: target.installationStatus!, message: null },
      instances: [{ ...activeInstance, active: false }], ambiguous: false, message: null,
    } }]);
    await (operation === 'install' ? useAgentStore.getState().installAgent('opencode', 'a'.repeat(64)) : useAgentStore.getState().configureAgent('opencode'));
    expect(bridge.configureAgent).not.toHaveBeenCalled();
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('当前实例未确认');
  });

  it('allows a consistent not-detected custom probe with an empty candidate list', async () => {
    seedFresh([{ ...notDetectedAgent, installationDetails: {
      probe: { status: 'not_detected', message: null }, instances: [], ambiguous: false, message: null,
    } }]);
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    expect(bridge.installAgent).toHaveBeenCalledExactlyOnceWith('opencode', 'a'.repeat(64));
    expect(bridge.configureAgent).not.toHaveBeenCalled();
  });

  it('never uninstalls a claimed npm source with missing canonical evidence', async () => {
    seedFresh([{ ...npmMatchedAgent, canUninstall: true, installationDetails: {
      ...npmMatchedAgent.installationDetails!, instances: [{ ...npmMatchedAgent.installationDetails!.instances[0], resolvedPath: null }],
    } }]);
    await useAgentStore.getState().uninstallAgent('opencode');
    expect(bridge.uninstallAgent).not.toHaveBeenCalled();
    expect(useAgentStore.getState().error).toContain('自动卸载功能尚未提供');
  });

  it('serializes installation, configuration, uninstall and refresh before React rerenders', async () => {
    seedFresh();
    let finish!: (outcome: bridge.InstallOutcome) => void;
    vi.mocked(bridge.installAgent).mockImplementationOnce(() => new Promise((done) => { finish = done; }));
    const first = useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
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
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    expect(useAgentStore.getState().agents[0].installed).toBe(true);
    expect(useAgentStore.getState().error).toContain('软件可能已安装');
    expect(useAgentStore.getState().error).toContain('installer exited');
    expect(useAgentStore.getState()).toMatchObject({ loading: false, dataStale: false });
  });

  it('reports successful install plus failed refresh as completed but stale, without retrying either operation', async () => {
    seedFresh();
    vi.mocked(bridge.getAgents).mockRejectedValueOnce(new Error('offline'));
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    expect(useAgentStore.getState().notice).toContain('软件安装操作已完成');
    expect(useAgentStore.getState().error).toBeNull();
    expect(useAgentStore.getState().refreshError).toContain('offline');
    expect(useAgentStore.getState()).toMatchObject({ dataStale: true, loading: false, agents: agentCatalog });
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(bridge.getAgents).toHaveBeenCalledTimes(1);
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    await useAgentStore.getState().configureAgent('cli-tool');
    expect(bridge.installAgent).toHaveBeenCalledTimes(1);
    expect(bridge.configureAgent).not.toHaveBeenCalled();
  });

  it('preserves the actual operation failure separately from repeated refresh failures', async () => {
    seedFresh();
    vi.mocked(bridge.installAgent).mockRejectedValueOnce(new Error('real install error'));
    vi.mocked(bridge.getAgents).mockRejectedValue(new Error('read error'));
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    await useAgentStore.getState().loadAgents();
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
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
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    expect(bridge.installAgent).not.toHaveBeenCalled();
    await useAgentStore.getState().loadAgents();
    expect(useAgentStore.getState()).toMatchObject({ dataStale: false, refreshError: null });
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
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

describe('preview request serialization', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useAgentStore.setState({ agents: [notDetectedAgent], loading: false, previewing: false, dataStale: false, error: null });
  });
  it('never dispatches absent or malformed review revisions', async () => {
    for (const revision of [undefined, null, '', 'x'.repeat(64), 'A'.repeat(64)]) {
      await useAgentStore.getState().installAgent('opencode', revision as unknown as string);
    }
    expect(bridge.installAgent).not.toHaveBeenCalled();
  });
  it('keeps global busy until preview settles, serializing preview, install and refresh', async () => {
    let settle!: (value: bridge.AgentInstallPreview) => void;
    const preview = vi.spyOn(bridge, 'previewAgentInstall').mockReturnValueOnce(new Promise(resolve => { settle = resolve; }));
    const first = useAgentStore.getState().prepareInstall('opencode');
    expect(useAgentStore.getState()).toMatchObject({ loading: true, previewing: true, dataStale: false });
    await useAgentStore.getState().prepareInstall('opencode');
    await useAgentStore.getState().installAgent('opencode', 'a'.repeat(64));
    await useAgentStore.getState().loadAgents();
    expect(preview).toHaveBeenCalledTimes(1);
    expect(bridge.installAgent).not.toHaveBeenCalled();
    expect(bridge.getAgents).not.toHaveBeenCalled();
    const { installPlan } = await import('../test-fixtures/installPlan');
    settle(installPlan);
    await expect(first).resolves.toEqual(installPlan);
    expect(useAgentStore.getState()).toMatchObject({ loading: false, previewing: false, dataStale: false });
  });
  it('discards preview when source data changes and releases busy even on failure', async () => {
    let settle!: (value: bridge.AgentInstallPreview) => void;
    vi.spyOn(bridge, 'previewAgentInstall').mockReturnValueOnce(new Promise(resolve => { settle = resolve; }));
    const first = useAgentStore.getState().prepareInstall('opencode');
    useAgentStore.setState({ agents: [{ ...notDetectedAgent }] });
    const { installPlan } = await import('../test-fixtures/installPlan');
    settle(installPlan);
    await expect(first).resolves.toBeNull();
    vi.mocked(bridge.previewAgentInstall).mockRejectedValueOnce('private npm configuration');
    await expect(useAgentStore.getState().prepareInstall('opencode')).resolves.toBeNull();
    expect(useAgentStore.getState()).toMatchObject({ loading: false, previewing: false, error: '无法取得有效安装预览，请刷新后重新检查。' });
  });
  it('checks fresh install eligibility and evidence before invoking preview', async () => {
    const preview = vi.spyOn(bridge, 'previewAgentInstall');
    for (const agent of [installedAgent, ambiguousAgent, { ...notDetectedAgent, canInstall: false }]) {
      useAgentStore.setState({ agents: [agent] });
      await expect(useAgentStore.getState().prepareInstall(agent.id)).resolves.toBeNull();
    }
    useAgentStore.setState({ agents: [notDetectedAgent], dataStale: true });
    await expect(useAgentStore.getState().prepareInstall('opencode')).resolves.toBeNull();
    expect(preview).not.toHaveBeenCalled();
  });
});
