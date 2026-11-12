import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { installPlan } from '../test-fixtures/installPlan';
import { AgentCard } from './AgentCard';
import { runtimeMode, type AgentInfo } from '../bridge';
import nativeAgentContract from '../../../../tests/fixtures/agent-info-installed.json';
import nativeNpmContract from '../../../../tests/fixtures/agent-info-npm.json';
import { notDetectedAgent as agent, installedAgent, activeInstance, observedAgent, npmMatchedAgent, ambiguousAgent } from '../test-fixtures/agents';

vi.mock('../bridge', async (importOriginal) => ({
  ...await importOriginal<typeof import('../bridge')>(),
  runtimeMode: vi.fn(() => 'native'),
}));

function mountAgent(overrides: Partial<React.ComponentProps<typeof AgentCard>> = {}) {
  const props = { agent, selected: false, dataStale: false, onPrepareInstall: vi.fn().mockResolvedValue(installPlan), onInstall: vi.fn(), onUninstall: vi.fn(), onConfigure: vi.fn(), onSelect: vi.fn(), ...overrides };
  return { ...render(<AgentCard {...props} />), props };
}

describe('AgentCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(runtimeMode).mockReturnValue('native');
    vi.spyOn(window, 'confirm').mockReturnValue(true);
  });

  it('renders the unchanged native serialization fixture with safe configuration and no fabricated version', () => {
    const { props } = mountAgent({ agent: nativeAgentContract as AgentInfo });
    expect(screen.getByText('已安装')).toBeInTheDocument();
    expect(screen.getByText('版本未知')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `应用默认配置 ${nativeAgentContract.name}` })).toBeEnabled();
    const uninstall = screen.getByRole('button', { name: `卸载 ${nativeAgentContract.name}` });
    expect(uninstall).toBeDisabled();
    fireEvent.click(uninstall);
    expect(props.onUninstall).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
    expect(screen.getByText(nativeAgentContract.uninstallHelp)).toBeInTheDocument();
  });

  it('renders a compact row with separate selection and operation controls', () => {
    const { props } = mountAgent();
    expect(screen.getByText('OpenCode')).toBeInTheDocument();
    expect(screen.getByText('开源 AI 编程代理')).toBeInTheDocument();
    const select = screen.getByRole('button', { name: '选择 OpenCode' });
    expect(select).toHaveAttribute('aria-pressed', 'false');
    expect(select.querySelector('button, a')).toBeNull();
    fireEvent.click(select);
    expect(props.onSelect).toHaveBeenCalledWith('opencode');
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled();
  });

  it('does not display legacy popularity flags or unverified star counts as catalog evidence', () => {
    mountAgent({ agent: { ...agent, popular: true, stars: 999999 } });
    expect(screen.queryByText('热度推荐')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.queryByText('注册表热度')).not.toBeInTheDocument();
    expect(screen.queryByText(/stars/)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled();
  });

  it('renders the unchanged npm DTO contract as directory evidence, not installation history or management rights', () => {
    const { props } = mountAgent({ agent: nativeNpmContract as AgentInfo });
    expect(screen.getByText('v1.2.3')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '卸载 Contract Agent' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '查看 Contract Agent 详情' }));
    const evidence = screen.getByRole('region', { name: 'Contract Agent 安装实例与来源证据' });
    expect(within(evidence).getByText('npm 全局目录匹配，安装历史未知')).toBeInTheDocument();
    expect(within(evidence).getByText(/不证明最初安装方式、安装时间或由 AgentDeck 安装/)).toBeInTheDocument();
    expect(within(evidence).getByText(nativeNpmContract.installationDetails.instances[0].source.packageRoot)).toBeInTheDocument();
    expect(within(evidence).getByText(nativeNpmContract.installationDetails.instances[0].source.managerPath)).toBeInTheDocument();
    expect(within(evidence).getByText('实测版本 1.2.3')).toBeInTheDocument();
    expect(screen.queryByText('安装来源未验证')).not.toBeInTheDocument();
    expect(props.onUninstall).not.toHaveBeenCalled();
  });

  it('keeps selected PATH, canonical file and proven entrypoint distinct', () => {
    mountAgent({ agent: npmMatchedAgent });
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    const current = screen.getByRole('listitem', { name: '当前 PATH 候选 1' });
    const instance = npmMatchedAgent.installationDetails!.instances[0];
    expect(within(current).getByText('PATH 路径').nextElementSibling).toHaveTextContent(instance.path);
    expect(within(current).getByText('规范文件路径').nextElementSibling).toHaveTextContent(instance.resolvedPath!);
    expect(within(current).getByText('已证实入口').nextElementSibling).toHaveTextContent(instance.entrypointPath!);
    expect(within(current).getByText('版本检测').nextElementSibling).toHaveTextContent('实测版本 2.4.0');
  });

  it('shows all observed candidates but never assigns a version to an unexecuted candidate', () => {
    const details = ambiguousAgent.installationDetails!;
    mountAgent({ agent: { ...ambiguousAgent, installationDetails: {
      ...details, instances: [details.instances[0], { ...details.instances[1], version: 'forged-other-version' }],
    } } });
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('检测到 2 个 PATH 候选，其中 1 个为其他候选。')).toBeInTheDocument();
    const additional = screen.getByRole('listitem', { name: '其他 PATH 候选 2' });
    expect(within(additional).getByText(details.instances[1].path)).toBeInTheDocument();
    expect(within(additional).getByText('未检测（其他候选未执行）')).toBeInTheDocument();
    expect(screen.queryByText(/forged-other-version/)).not.toBeInTheDocument();
    expect(screen.getByText('实测版本 2.4.0')).toBeInTheDocument();
  });

  it('blocks ambiguous configuration with a visible, programmatically associated reason', () => {
    const { props } = mountAgent({ agent: ambiguousAgent });
    const configure = screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    expect(configure).toBeDisabled();
    expect(configure).toHaveAttribute('title', expect.stringContaining('PATH 候选优先级存在歧义'));
    expect(document.getElementById(configure.getAttribute('aria-describedby')!)).toHaveTextContent('已暂停安装与应用配置');
    fireEvent.click(configure);
    expect(props.onConfigure).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText(ambiguousAgent.installationDetails!.message!)).toBeInTheDocument();
  });

  it('does not elect or measure an instance when duplicate active markers contradict a false ambiguity flag', () => {
    mountAgent({ agent: { ...observedAgent, installationDetails: {
      ...observedAgent.installationDetails!,
      instances: [activeInstance, { ...activeInstance, path: '/synthetic/duplicate/example-agent', version: '9.9.9' }],
    } } });
    expect(screen.getByText('版本未知')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '应用默认配置 OpenCode' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText(/未确认唯一的当前候选/)).toBeInTheDocument();
    expect(screen.queryByText(/实测版本/)).not.toBeInTheDocument();
    expect(screen.queryByText(/9\.9\.9/)).not.toBeInTheDocument();
  });

  it.each([undefined, 'npm', 'unknown'] as const)('does not turn legacy version/source metadata into instance evidence (%s)', (installSource) => {
    mountAgent({ agent: { ...installedAgent, installationDetails: undefined, installSource, version: 'legacy-version', canUninstall: true } });
    expect(screen.getByText('版本未验证')).toBeInTheDocument();
    expect(screen.queryByText(/legacy-version/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('未提供安装实例证据；路径、来源与实例版本均无法确认。')).toBeInTheDocument();
    expect(screen.queryByText('npm 全局目录匹配，安装历史未知')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toBeDisabled();
  });

  it.each([null, '', '   ', undefined])('reports an unknown measured version without falling back to top-level metadata (%s)', (version) => {
    mountAgent({ agent: { ...observedAgent, version: 'unsupported-fallback', installationDetails: {
      ...observedAgent.installationDetails!, instances: [{ ...activeInstance, version } as typeof activeInstance],
    } } });
    expect(screen.getByText('版本未知')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('版本未知（未取得有效版本结果）')).toBeInTheDocument();
    expect(screen.queryByText(/unsupported-fallback/)).not.toBeInTheDocument();
  });

  it.each([
    undefined, null, 'npm', { kind: 'managed' }, { kind: 'npm' },
    { kind: 'npm', package: 'example-agent', packageRoot: '/synthetic/pkg', globalRoot: '/synthetic', prefix: '/synthetic', managerPath: 42 },
    { kind: 'unknown', reason: { text: 'not displayable' } },
  ])('does not trust malformed or unknown source variants (%j)', (source) => {
    mountAgent({ agent: { ...npmMatchedAgent, canUninstall: true, installationDetails: {
      ...npmMatchedAgent.installationDetails!,
      instances: [{ ...npmMatchedAgent.installationDetails!.instances[0], source } as unknown as typeof activeInstance],
    } } });
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('来源未知')).toBeInTheDocument();
    expect(screen.getByText('来源证据缺失或格式不完整，无法确认当前目录与包的关联。')).toBeInTheDocument();
    expect(screen.queryByText('npm 全局目录匹配，安装历史未知')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toBeDisabled();
  });

  it('requires a proven entrypoint before showing a complete npm source object as linkage evidence', () => {
    mountAgent({ agent: { ...npmMatchedAgent, installationDetails: {
      ...npmMatchedAgent.installationDetails!, instances: [{ ...npmMatchedAgent.installationDetails!.instances[0], entrypointPath: null }],
    } } });
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('未证实实际入口，不能由启动文件名推断')).toBeInTheDocument();
    expect(screen.getByText('来源未知')).toBeInTheDocument();
    expect(screen.queryByText('npm 全局目录匹配，安装历史未知')).not.toBeInTheDocument();
  });

  it.each([null, undefined, '', '   '])('requires a canonical discovered file before reporting npm linkage (%s)', (resolvedPath) => {
    mountAgent({ agent: { ...npmMatchedAgent, installationDetails: {
      ...npmMatchedAgent.installationDetails!,
      instances: [{ ...npmMatchedAgent.installationDetails!.instances[0], resolvedPath } as typeof activeInstance],
    } } });
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('未提供或无法解析')).toBeInTheDocument();
    expect(screen.getByText('来源未知')).toBeInTheDocument();
    expect(screen.queryByText('npm 全局目录匹配，安装历史未知')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toBeDisabled();
  });

  it.each([undefined, null, {}, { status: 'error' }, { status: 'not_detected' }, { status: 'unsupported' }, { status: 'forged' }, { status: 42 }])('blocks installed configuration with missing, malformed or inconsistent probe (%j)', (probe) => {
    const { props } = mountAgent({ agent: { ...observedAgent, installationDetails: {
      ...observedAgent.installationDetails!, probe,
    } } as unknown as AgentInfo });
    expect(screen.getByText('安装状态未知')).toBeInTheDocument();
    expect(screen.getByText('版本未知')).toBeInTheDocument();
    const configure = screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    expect(configure).toBeDisabled();
    expect(configure).toHaveAttribute('title', expect.stringContaining('安装检测证据不完整或状态不一致'));
    fireEvent.click(configure);
    expect(props.onConfigure).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
  });

  it.each([undefined, null, {}, { status: 'error' }, { status: 'installed' }, { status: 'unsupported' }, { status: 'forged' }])('blocks installation with missing, malformed or inconsistent probe despite install permission (%j)', (probe) => {
    const { props } = mountAgent({ agent: { ...agent, installationDetails: {
      ...installedAgent.installationDetails!, probe,
    } } as unknown as AgentInfo });
    const install = screen.getByRole('button', { name: '安装 OpenCode' });
    expect(install).toBeDisabled();
    expect(install).toHaveTextContent('安装不可用');
    expect(install).toHaveAttribute('title', expect.stringContaining('安装检测证据不完整或状态不一致'));
    fireEvent.click(install);
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
  });

  it.each([
    { installed: false, installationStatus: 'installed', probeStatus: 'installed' },
    { installed: true, installationStatus: 'not_detected', probeStatus: 'not_detected' },
  ] as const)('rejects a matching status string when the installed boolean contradicts it (%j)', ({ installed, installationStatus, probeStatus }) => {
    mountAgent({ agent: { ...installedAgent, installed, installationStatus, installationDetails: {
      ...installedAgent.installationDetails!, probe: { status: probeStatus, message: null },
    } } });
    expect(screen.getByText('安装状态未知')).toBeInTheDocument();
    expect(screen.getByText(/安装检测证据不完整或状态不一致/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '应用默认配置 OpenCode' })).not.toBeInTheDocument();
  });

  it('does not authorize configuration for nonempty observations with no active candidate', () => {
    const { props } = mountAgent({ agent: { ...observedAgent, installationDetails: {
      ...observedAgent.installationDetails!, instances: [{ ...activeInstance, active: false }],
    } } });
    const configure = screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    expect(configure).toBeDisabled();
    expect(configure).toHaveAttribute('title', expect.stringContaining('当前实例未确认'));
    fireEvent.click(configure);
    expect(props.onConfigure).not.toHaveBeenCalled();
    expect(screen.getByText('版本未知')).toBeInTheDocument();
  });

  it('preserves valid empty custom-verification lists without inventing a required instance', () => {
    const { rerender, props } = mountAgent({ agent: installedAgent });
    expect(screen.getByRole('button', { name: '应用默认配置 OpenCode' })).toBeEnabled();
    rerender(<AgentCard {...props} agent={{ ...agent, installationDetails: {
      probe: { status: 'not_detected', message: null }, instances: [], ambiguous: false, message: null,
    } }} />);
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled();
  });

  it.each([null, {}, { instances: null }, { instances: [null], ambiguous: false }])('fails closed on malformed new evidence without crashing (%j)', (installationDetails) => {
    mountAgent({ agent: { ...installedAgent, installationDetails } as unknown as AgentInfo });
    expect(screen.getByRole('button', { name: '应用默认配置 OpenCode' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('未提供安装实例证据；路径、来源与实例版本均无法确认。')).toBeInTheDocument();
  });

  it('shows unknown-source reasons, long paths and raw evidence as text, not executable links or markup', () => {
    const path = '/synthetic/' + 'long-path-segment/'.repeat(30) + '<script>example</script>';
    mountAgent({ agent: { ...observedAgent, installationDetails: {
      ...observedAgent.installationDetails!,
      instances: [{ ...activeInstance, path, resolvedPath: null, source: { kind: 'unknown', reason: '<b>synthetic reason</b>' } }],
    } } });
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    const value = screen.getByText(path);
    expect(value).toHaveClass('agent-path');
    expect(value.querySelector('script')).toBeNull();
    expect(screen.getByText('<b>synthetic reason</b>')).toBeInTheDocument();
    expect(screen.getByText('未提供或无法解析')).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: path })).not.toBeInTheDocument();
  });

  it('never offers uninstall even with complete npm evidence and forged capability', () => {
    const { props } = mountAgent({ agent: { ...npmMatchedAgent, canUninstall: true } });
    const uninstall = screen.getByRole('button', { name: '卸载 OpenCode' });
    expect(uninstall).toBeDisabled();
    expect(uninstall).toHaveAttribute('title', expect.stringContaining('自动卸载功能尚未提供'));
    fireEvent.click(uninstall);
    expect(props.onUninstall).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
    expect(screen.queryByRole('button', { name: /升级/ })).not.toBeInTheDocument();
  });

  it.each([
    { mode: 'native' as const, busy: true, dataStale: false },
    { mode: 'native' as const, busy: false, dataStale: true },
    { mode: 'demo' as const, busy: false, dataStale: false },
    { mode: 'unavailable' as const, busy: false, dataStale: false },
  ])('does not let known source evidence bypass runtime/freshness/busy guards (%j)', ({ mode, busy, dataStale }) => {
    vi.mocked(runtimeMode).mockReturnValue(mode);
    const { props } = mountAgent({ agent: npmMatchedAgent, busy, dataStale });
    const configure = screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    expect(configure).toBeDisabled();
    fireEvent.click(configure);
    expect(props.onConfigure).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText('npm 全局目录匹配，安装历史未知')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toBeDisabled();
  });

  it('shows the actual bounded plan and limitations before any installation', async () => {
    const { props } = mountAgent();
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    const dialog = await screen.findByRole('dialog', { name: 'OpenCode 安装计划' });
    for (const value of [installPlan.package, installPlan.executable, installPlan.prefix,
      installPlan.globalRoot, installPlan.packageRoot, installPlan.launcherDirectory,
      installPlan.managerPath, installPlan.nodePath, installPlan.installerNodeVersion, installPlan.installerNodeMinimum]) {
      expect(within(dialog).getByText(value)).toBeInTheDocument();
    }
    expect(dialog).toHaveTextContent('版本由当前 npm 配置选择，未固定或解析版本');
    expect(dialog).toHaveTextContent('安装器 Node 仅用于此次安装');
    expect(dialog).toHaveTextContent('AgentDeck 安装器 Node 最低策略');
    expect(dialog).toHaveTextContent('不代表包或依赖兼容');
    expect(dialog).toHaveTextContent('生命周期脚本：遵循当前 npm 配置');
    expect(dialog).toHaveTextContent('不创建配置备份');
    expect(dialog).toHaveTextContent('不自动回滚或卸载');
    expect(dialog).toHaveTextContent('npm 可能读取配置或更新缓存');
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
  });

  it.each([['pi', 'Pi', '@earendil-works/pi-coding-agent'], ['codex', 'Codex', '@openai/codex']])('shows disabled lifecycle scripts for the reviewed %s plan', async (id, name, pkg) => {
    const disabledPlan = { ...installPlan, versionPolicy: id === 'codex' ? 'exact_root' as const : 'npm_configured' as const, packageVersion: id === 'codex' ? '1.2.3' : null, installerNodeMinimum: id === 'pi' ? '22.19.0' : '18.0.0', agentId: id, package: pkg, executable: id, scriptPolicy: 'disabled' as const };
    mountAgent({ agent: { ...agent, id, name }, onPrepareInstall: vi.fn().mockResolvedValue(disabledPlan) });
    fireEvent.click(screen.getByRole('button', { name: `安装 ${name}` }));
    const dialog = await screen.findByRole('dialog', { name: `${name} 安装计划` });
    expect(dialog).toHaveTextContent('生命周期脚本：禁用（ignore-scripts）');
    expect(dialog).not.toHaveTextContent('生命周期脚本：遵循当前 npm 配置');
    if (id === 'codex') {
      expect(dialog).toHaveTextContent('Codex 根包固定版本');
      expect(dialog).toHaveTextContent('1.2.3');
      expect(dialog).toHaveTextContent('未锁定依赖闭包、registry 或下载内容完整性');
      expect(dialog).not.toHaveTextContent('版本由当前 npm 配置选择');
    }
  });

  it('confirms software-only installation without configuring or selecting the row', async () => {
    const { props } = mountAgent();
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(props.onInstall).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole('button', { name: '确认安装软件' }));
    expect(window.confirm).not.toHaveBeenCalled();
    expect(props.onInstall).toHaveBeenCalledWith('opencode', installPlan.revision);
    expect(props.onSelect).not.toHaveBeenCalled();
    expect(props.onConfigure).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled());
  });

  it.each(['unknown', undefined] as const)('shows observed versions but never uninstalls unverified origins (%s)', (installSource) => {
    const { props } = mountAgent({ agent: { ...observedAgent, installSource, canUninstall: true } });
    expect(screen.getByText('已安装')).toBeInTheDocument();
    expect(screen.getByText('v2.4.0')).toBeInTheDocument();
    const action = screen.getByRole('button', { name: '卸载 OpenCode' });
    expect(action).toBeDisabled();
    fireEvent.click(action);
    expect(window.confirm).not.toHaveBeenCalled();
    expect(props.onUninstall).not.toHaveBeenCalled();
    expect(screen.getByText(/安装来源未验证/)).toBeInTheDocument();
  });

  it('requires a separate explicit confirmation to apply the default profile', async () => {
    const { props } = mountAgent({ agent: installedAgent });
    const configure = screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    expect(props.onConfigure).not.toHaveBeenCalled();
    vi.mocked(window.confirm).mockReturnValue(false);
    fireEvent.click(configure);
    expect(window.confirm).toHaveBeenCalledWith('确定为 OpenCode 应用默认 profile 吗？将更新个性化指令与支持的模型配置，写入前会备份相关文件。');
    expect(props.onConfigure).not.toHaveBeenCalled();
    vi.mocked(window.confirm).mockReturnValue(true);
    fireEvent.click(configure);
    expect(props.onConfigure).toHaveBeenCalledTimes(1);
    expect(props.onConfigure).toHaveBeenCalledWith('opencode');
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(props.onSelect).not.toHaveBeenCalled();
    await waitFor(() => expect(configure).toBeEnabled());
  });

  it('canceling installation never performs an operation and permits a later confirmation', async () => {
    const { props } = mountAgent();
    const action = screen.getByRole('button', { name: '安装 OpenCode' });
    fireEvent.click(action);
    await screen.findByRole('button', { name: '确认安装软件' });
    fireEvent.click(screen.getByTestId('install-review-cancel'));
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(props.onConfigure).not.toHaveBeenCalled();
    expect(props.onUninstall).not.toHaveBeenCalled();
    expect(action).toBeEnabled();
    fireEvent.click(action);
    fireEvent.click(await screen.findByRole('button', { name: '确认安装软件' }));
    expect(props.onInstall).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(action).toBeEnabled());
  });

  it('locks repeated preview and install clicks until the operation settles', async () => {
    let finish!: () => void;
    const onInstall = vi.fn(() => new Promise<void>((resolve) => { finish = resolve; }));
    const { props } = mountAgent({ onInstall });
    const action = screen.getByRole('button', { name: '安装 OpenCode' });
    fireEvent.click(action);
    fireEvent.click(action);
    expect(props.onPrepareInstall).toHaveBeenCalledTimes(1);
    const confirm = await screen.findByRole('button', { name: '确认安装软件' });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    expect(action).toBeDisabled();
    expect(onInstall).toHaveBeenCalledTimes(1);
    expect(window.confirm).not.toHaveBeenCalled();
    await act(async () => finish());
    expect(action).toBeEnabled();
  });

  it('canceling a pending preview discards its result but retains the request lock until settlement', async () => {
    let finish!: (value: typeof installPlan) => void;
    const onPrepareInstall = vi.fn(() => new Promise<typeof installPlan>((resolve) => { finish = resolve; }));
    const { props } = mountAgent({ onPrepareInstall });
    const action = screen.getByRole('button', { name: '安装 OpenCode' });
    fireEvent.click(action);
    fireEvent.click(screen.getByRole('button', { name: '取消预览展示' }));
    expect(action).toBeDisabled();
    fireEvent.click(action);
    expect(onPrepareInstall).toHaveBeenCalledTimes(1);
    await act(async () => finish(installPlan));
    expect(screen.queryByRole('button', { name: '确认安装软件' })).not.toBeInTheDocument();
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(action).toBeEnabled();
  });

  it('discards preview results after unmount without installing', async () => {
    let finish!: (value: typeof installPlan) => void;
    const onPrepareInstall = vi.fn(() => new Promise<typeof installPlan>((resolve) => { finish = resolve; }));
    const { props, unmount } = mountAgent({ onPrepareInstall });
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(onPrepareInstall).toHaveBeenCalledTimes(1);
    unmount();
    await act(async () => finish(installPlan));
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(screen.queryByRole('button', { name: '确认安装软件' })).not.toBeInTheDocument();
  });

  it.each(['agent', 'selection', 'freshness'] as const)('discards a reviewed plan when %s changes', async (change) => {
    const { props, rerender } = mountAgent();
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    await screen.findByRole('button', { name: '确认安装软件' });
    rerender(<AgentCard {...props} {...(change === 'agent' ? { agent: { ...agent } } : change === 'selection' ? { selected: true } : { dataStale: true })} />);
    expect(screen.queryByRole('button', { name: '确认安装软件' })).not.toBeInTheDocument();
    expect(props.onInstall).not.toHaveBeenCalled();
  });

  it('disables operations while another workspace operation is busy', () => {
    const { props } = mountAgent({ busy: true });
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '查看 OpenCode 详情' })).toBeEnabled();
  });

  it('keeps manual install help visible and does not offer automatic installation', () => {
    const { props } = mountAgent({ agent: { ...agent, canInstall: false, installHelp: '请在 VS Code 安装扩展' } });
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeDisabled();
    expect(screen.getByText('手动安装')).toBeInTheDocument();
    expect(screen.getByText('请在 VS Code 安装扩展')).toBeInTheDocument();
    const link = screen.getByRole('link', { name: '官方说明' });
    expect(link).toHaveAttribute('href', 'https://opencode.ai/');
    expect(link).toHaveAttribute('rel', 'noopener noreferrer');
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(props.onInstall).not.toHaveBeenCalled();
  });

  it('retains official manual-install guidance alongside an unsupported probe explanation', () => {
    mountAgent({ agent: { ...agent, canInstall: false, installationStatus: 'unsupported', installationMessage: '不支持自动检测扩展', installHelp: '请在 VS Code 安装扩展' } });
    expect(screen.getByText(/不支持自动检测扩展.*请在 VS Code 安装扩展/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeDisabled();
  });

  it('keeps manual uninstall disabled and provides visible guidance', () => {
    mountAgent({ agent: { ...installedAgent, canUninstall: false } });
    expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toBeDisabled();
    expect(screen.getByText('手动卸载')).toBeInTheDocument();
    expect(screen.getByText(/官方卸载说明/)).toBeInTheDocument();
    expect(screen.getByText('版本未知')).toBeInTheDocument();
  });

  it.each(['error', 'unsupported', undefined] as const)('fails closed rather than claiming absence when detection is %s', (installationStatus) => {
    const { props } = mountAgent({ agent: { ...agent, installationStatus, installationMessage: '探测未能确认状态' } });
    expect(screen.getByText(installationStatus === 'error' ? '检测出错' : '安装状态未知')).toBeInTheDocument();
    expect(screen.queryByText('未安装')).not.toBeInTheDocument();
    expect(screen.queryByText('PATH 未检测到')).not.toBeInTheDocument();
    expect(screen.getByText('探测未能确认状态')).toBeInTheDocument();
    const install = screen.getByRole('button', { name: '安装 OpenCode' });
    expect(install).toBeDisabled();
    fireEvent.click(install);
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
  });

  it('does not confuse PATH absence with a claim that no copy exists on the device', () => {
    mountAgent();
    expect(screen.getByText('PATH 未检测到')).toBeInTheDocument();
    expect(screen.queryByText('未安装')).not.toBeInTheDocument();
  });

  it.each([false, undefined])('does not offer configuration without declared support (%s)', (canConfigure) => {
    mountAgent({ agent: { ...installedAgent, canConfigure } });
    expect(screen.queryByRole('button', { name: '应用默认配置 OpenCode' })).not.toBeInTheDocument();
  });

  it('does not offer configuration for a legacy installed boolean without detection status', () => {
    mountAgent({ agent: { ...installedAgent, installationStatus: undefined } });
    expect(screen.getByText('安装状态未知')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '应用默认配置 OpenCode' })).not.toBeInTheDocument();
  });

  it.each([true, undefined])('blocks installation when data freshness is not explicitly established (%s)', (dataStale) => {
    const { props } = mountAgent({ dataStale });
    const action = screen.getByRole('button', { name: '安装 OpenCode' });
    expect(action).toBeDisabled();
    expect(action).toHaveAttribute('title', expect.stringContaining('状态待刷新'));
    fireEvent.click(action);
    expect(props.onInstall).not.toHaveBeenCalled();
  });

  it('blocks configuration while the last known installed data is stale', () => {
    const { props } = mountAgent({ agent: installedAgent, dataStale: true });
    const action = screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    expect(action).toBeDisabled();
    fireEvent.click(action);
    expect(props.onConfigure).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
  });

  it('locks repeated configuration clicks until the operation settles', async () => {
    let finish!: () => void;
    const onConfigure = vi.fn(() => new Promise<void>((resolve) => { finish = resolve; }));
    mountAgent({ agent: installedAgent, onConfigure });
    const action = screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    fireEvent.click(action);
    fireEvent.click(action);
    expect(action).toBeDisabled();
    expect(onConfigure).toHaveBeenCalledTimes(1);
    expect(action).toHaveTextContent('应用中…');
    expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toHaveTextContent('手动卸载');
    expect(window.confirm).toHaveBeenCalledTimes(1);
    await act(async () => finish());
    expect(action).toBeEnabled();
  });

  it('uses an accessible native disclosure with Escape close and focus return', () => {
    mountAgent({ selected: true });
    expect(screen.getByRole('button', { name: '选择 OpenCode' })).toHaveAttribute('aria-pressed', 'true');
    const trigger = screen.getByRole('button', { name: '查看 OpenCode 详情' });
    expect(trigger.tagName).toBe('BUTTON');
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    trigger.focus();
    fireEvent.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    const details = screen.getByRole('region', { name: 'OpenCode 详情' });
    expect(details.id).toBe(trigger.getAttribute('aria-controls'));
    const close = within(details).getByRole('button', { name: '关闭 OpenCode 详情' });
    close.focus();
    fireEvent.keyDown(close, { key: 'Escape' });
    expect(screen.queryByRole('region')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
  });

  it('also closes details with Escape while the disclosure button retains focus', () => {
    mountAgent();
    const trigger = screen.getByRole('button', { name: '查看 OpenCode 详情' });
    fireEvent.click(trigger);
    fireEvent.keyDown(trigger, { key: 'Escape' });
    expect(screen.queryByRole('region')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it.each([false, true])('describes auth as local evidence, never online verification (%s)', (authVerified) => {
    mountAgent({ agent: { ...agent, authVerified } });
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText(authVerified ? '检测到本地认证配置' : '未验证')).toBeInTheDocument();
    expect(screen.getByText(/不代表远端凭据仍有效/)).toBeInTheDocument();
    expect(screen.queryByText('认证成功')).not.toBeInTheDocument();
  });

  it.each(['http://example.com', 'javascript:alert(1)', 'not-a-url', 'https://user:secret@example.com'])('does not expose an unsafe official link: %s', (homepage) => {
    mountAgent({ agent: { ...agent, canInstall: false, homepage } });
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.queryByRole('link')).not.toBeInTheDocument();
    expect(screen.getByText('未提供 HTTPS 官方链接')).toBeInTheDocument();
  });

  it.each(['demo', 'unavailable'] as const)('forbids writes in %s runtime even with install capability', (mode) => {
    vi.mocked(runtimeMode).mockReturnValue(mode);
    const { props } = mountAgent();
    const action = screen.getByRole('button', { name: '安装 OpenCode' });
    expect(action).toBeDisabled();
    fireEvent.click(action);
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(props.onPrepareInstall).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '查看 OpenCode 详情' }));
    expect(screen.getByText(mode === 'demo' ? /只读演示：/ : /未连接桌面后端/)).toBeInTheDocument();
  });

  it.each(['demo', 'unavailable'] as const)('forbids configuring installed agents in %s runtime', (mode) => {
    vi.mocked(runtimeMode).mockReturnValue(mode);
    const { props } = mountAgent({ agent: installedAgent });
    const action = screen.getByRole('button', { name: '应用默认配置 OpenCode' });
    expect(action).toBeDisabled();
    fireEvent.click(action);
    expect(props.onConfigure).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
  });

  it('reports unexpected operation failure without inventing an installed state', async () => {
    mountAgent({ onInstall: vi.fn().mockRejectedValue(new Error('连接中断')) });
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    fireEvent.click(await screen.findByRole('button', { name: '确认安装软件' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('安装未确认完成，软件可能已安装');
    expect(screen.getByText('PATH 未检测到')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled();
  });
});
