import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { AgentCard } from './AgentCard';
import { runtimeMode, type AgentInfo } from '../bridge';

vi.mock('../bridge', async (importOriginal) => ({
  ...await importOriginal<typeof import('../bridge')>(),
  runtimeMode: vi.fn(() => 'native'),
}));

const agent: AgentInfo = {
  id: 'opencode', name: 'OpenCode', installed: false,
  canInstall: true, canUninstall: true, canVerifyInstall: true,
  authVerified: false, source: 'open', uiType: 'all', stars: 194000,
  popular: true, description: '开源 AI 编程代理', icon: 'opencode', category: 'ai-coding',
  homepage: 'https://opencode.ai',
};

function mountAgent(overrides: Partial<React.ComponentProps<typeof AgentCard>> = {}) {
  const props = { agent, selected: false, onInstall: vi.fn(), onUninstall: vi.fn(), onSelect: vi.fn(), ...overrides };
  return { ...render(<AgentCard {...props} />), props };
}

describe('AgentCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(runtimeMode).mockReturnValue('native');
    vi.spyOn(window, 'confirm').mockReturnValue(true);
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

  it('confirms installation, including configuration changes, without selecting the row', async () => {
    const { props } = mountAgent();
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('写前备份'));
    expect(props.onInstall).toHaveBeenCalledWith('opencode');
    expect(props.onSelect).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled());
  });

  it('shows the observed version and confirms uninstall', async () => {
    const { props } = mountAgent({ agent: { ...agent, installed: true, version: 'v1.0.0' } });
    expect(screen.getByText('已安装')).toBeInTheDocument();
    expect(screen.getByText('v1.0.0')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '卸载 OpenCode' }));
    expect(window.confirm).toHaveBeenCalledWith('确定卸载 OpenCode 吗？');
    expect(props.onUninstall).toHaveBeenCalledWith('opencode');
    await waitFor(() => expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toBeEnabled());
  });

  it.each([false, true])('canceling confirmation never performs an operation (installed=%s)', async (installed) => {
    vi.mocked(window.confirm).mockReturnValue(false);
    const { props } = mountAgent({ agent: { ...agent, installed } });
    const action = screen.getByRole('button', { name: `${installed ? '卸载' : '安装'} OpenCode` });
    fireEvent.click(action);
    expect(props.onInstall).not.toHaveBeenCalled();
    expect(props.onUninstall).not.toHaveBeenCalled();
    expect(action).toBeEnabled();
    vi.mocked(window.confirm).mockReturnValue(true);
    fireEvent.click(action);
    expect(installed ? props.onUninstall : props.onInstall).toHaveBeenCalledTimes(1);
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

  it('keeps manual uninstall disabled and provides visible guidance', () => {
    mountAgent({ agent: { ...agent, installed: true, canUninstall: false } });
    expect(screen.getByRole('button', { name: '卸载 OpenCode' })).toBeDisabled();
    expect(screen.getByText('手动卸载')).toBeInTheDocument();
    expect(screen.getByText(/请使用官方卸载方式/)).toBeInTheDocument();
    expect(screen.getByText('版本未知')).toBeInTheDocument();
  });

  it.each([false, undefined])('does not claim an agent is uninstalled without a supported probe (%s)', (canVerifyInstall) => {
    mountAgent({ agent: { ...agent, canVerifyInstall } });
    expect(screen.getByText('安装状态未知')).toBeInTheDocument();
    expect(screen.queryByText('未安装')).not.toBeInTheDocument();
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

  it('reports unexpected operation failure without inventing an installed state', async () => {
    mountAgent({ onInstall: vi.fn().mockRejectedValue(new Error('连接中断')) });
    fireEvent.click(screen.getByRole('button', { name: '安装 OpenCode' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('连接中断');
    expect(screen.getByText('未安装')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '安装 OpenCode' })).toBeEnabled();
  });
});
