import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { AgentCard } from './AgentCard';
import type { AgentInfo } from '../bridge';

const agent: AgentInfo = {
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
};

describe('AgentCard', () => {
  it('渲染名称、描述与安装按钮', () => {
    render(
      <AgentCard
        agent={agent}
        selected={false}
        onInstall={vi.fn()}
        onUninstall={vi.fn()}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByText('OpenCode')).toBeInTheDocument();
    expect(screen.getByText('开源 AI 编程代理')).toBeInTheDocument();
    expect(screen.getByText('一键安装')).toBeInTheDocument();
  });

  it('点击安装触发 onInstall', () => {
    const onInstall = vi.fn();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(
      <AgentCard
        agent={agent}
        selected={false}
        onInstall={onInstall}
        onUninstall={vi.fn()}
        onSelect={vi.fn()}
      />
    );
    fireEvent.click(screen.getByText('一键安装'));
    expect(onInstall).toHaveBeenCalledWith('opencode');
  });

  it('已安装态显示卸载按钮并触发 onUninstall', () => {
    const onUninstall = vi.fn();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(
      <AgentCard
        agent={{ ...agent, installed: true, version: '1.0.0' }}
        selected={false}
        onInstall={vi.fn()}
        onUninstall={onUninstall}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByText(/已安装/)).toBeInTheDocument();
    fireEvent.click(screen.getByText('卸载'));
    expect(onUninstall).toHaveBeenCalledWith('opencode');
  });
});

it('unsupported capability never offers an automatic install', () => {
  const onInstall = vi.fn();
  render(<AgentCard agent={{ ...agent, canInstall: false, installHelp: '请在 VS Code 安装扩展' }} selected={false} onInstall={onInstall} onUninstall={vi.fn()} onSelect={vi.fn()} />);
  expect(screen.getByText('手动安装')).toBeDisabled();
  expect(screen.getByText('请在 VS Code 安装扩展')).toBeInTheDocument();
  fireEvent.click(screen.getByText('手动安装'));
  expect(onInstall).not.toHaveBeenCalled();
});

it('canceling confirmation and busy state never start an operation', () => {
  const onInstall = vi.fn();
  vi.spyOn(window, 'confirm').mockReturnValue(false);
  const { rerender } = render(<AgentCard agent={agent} selected={false} onInstall={onInstall} onUninstall={vi.fn()} onSelect={vi.fn()} />);
  fireEvent.click(screen.getByText('一键安装'));
  expect(onInstall).not.toHaveBeenCalled();
  rerender(<AgentCard agent={agent} busy selected={false} onInstall={onInstall} onUninstall={vi.fn()} onSelect={vi.fn()} />);
  expect(screen.getByText('处理中...')).toBeDisabled();
});

it('does not label manually managed agents as uninstalled without a probe', () => {
  render(<AgentCard agent={{ ...agent, canInstall: false, canVerifyInstall: false }} selected={false} onInstall={vi.fn()} onUninstall={vi.fn()} onSelect={vi.fn()} />);
  expect(screen.getByText('安装状态未知')).toBeInTheDocument();
  expect(screen.queryByText('未安装')).not.toBeInTheDocument();
});
