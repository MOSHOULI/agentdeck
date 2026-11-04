import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { AgentCard } from './AgentCard';
import { runtimeMode, type AgentInfo } from '../bridge';
import nativeAgentContract from '../../../../tests/fixtures/agent-info-installed.json';
import { notDetectedAgent as agent, installedAgent } from '../test-fixtures/agents';

vi.mock('../bridge', async (importOriginal) => ({
  ...await importOriginal<typeof import('../bridge')>(),
  runtimeMode: vi.fn(() => 'native'),
}));

function mountAgent(overrides: Partial<React.ComponentProps<typeof AgentCard>> = {}) {
  const props = { agent, selected: false, dataStale: false, onInstall: vi.fn(), onUninstall: vi.fn(), onConfigure: vi.fn(), onSelect: vi.fn(), ...overrides };
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

  it('confirms software-only installation without configuring or selecting the row', async () => {
    const { props } = mountAgent();
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('此操作仅安装软件，不应用默认 profile 或修改模型配置'));
    expect(props.onInstall).toHaveBeenCalledWith('opencode');
    expect(props.onSelect).not.toHaveBeenCalled();
    expect(props.onConfigure).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled());
  });

  it.each(['unknown', undefined] as const)('shows observed versions but never uninstalls unverified origins (%s)', (installSource) => {
    const { props } = mountAgent({ agent: { ...installedAgent, installSource, canUninstall: true, version: 'v1.0.0' } });
    expect(screen.getByText('已安装')).toBeInTheDocument();
    expect(screen.getByText('v1.0.0')).toBeInTheDocument();
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
    vi.mocked(window.confirm).mockReturnValue(false);
    const { props } = mountAgent();
    const action = screen.getByRole('button', { name: '安装 OpenCode' });
    fireEvent.click(action);
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(props.onConfigure).not.toHaveBeenCalled();
    expect(props.onUninstall).not.toHaveBeenCalled();
    expect(action).toBeEnabled();
    vi.mocked(window.confirm).mockReturnValue(true);
    fireEvent.click(action);
    expect(props.onInstall).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(action).toBeEnabled());
  });

  it('locks repeated install clicks until the operation settles', async () => {
    let finish!: () => void;
    const onInstall = vi.fn(() => new Promise<void>((resolve) => { finish = resolve; }));
    mountAgent({ onInstall });
    const action = screen.getByRole('button', { name: '安装 OpenCode' });
    fireEvent.click(action);
    fireEvent.click(action);
    expect(action).toBeDisabled();
    expect(onInstall).toHaveBeenCalledTimes(1);
    expect(window.confirm).toHaveBeenCalledTimes(1);
    await act(async () => finish());
    expect(action).toBeEnabled();
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
    expect(screen.getByText('注册表记录，非实时数据')).toBeInTheDocument();
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
    expect(await screen.findByRole('alert')).toHaveTextContent('连接中断');
    expect(screen.getByText('PATH 未检测到')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled();
  });
});
