import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, expect, it } from 'vitest';
import { InstallSessionNote } from './InstallSessionNote';
import { useAgentStore, type InstallTargetSnapshot } from '../stores/agentStore';
import { installPlan } from '../test-fixtures/installPlan';

beforeEach(() => useAgentStore.setState({ installSessions: {} }));
it('shows only historical reviewed targets with non-authority and session boundaries', () => {
  const { revision: _revision, ...target } = installPlan;
  useAgentStore.setState({ installSessions: { opencode: { state: 'unconfirmed', target: target as InstallTargetSnapshot } } });
  const view = render(<InstallSessionNote agentId="opencode" />);
  const record = screen.getByTestId('install-session-record');
  expect(record).toHaveAttribute('data-state', 'unconfirmed');
  fireEvent.click(screen.getByText('本会话安装记录：安装完成状态未确认'));
  expect(record).toHaveTextContent('可能在内部预检阶段被拒绝');
  expect(record).toHaveTextContent('不是当前磁盘检查、安装所有权或恢复凭据');
  expect(record).toHaveTextContent('关闭或重启应用后记录丢失');
  expect(record).toHaveTextContent(installPlan.packageRoot);
  expect(record).not.toHaveTextContent(installPlan.revision);
  expect(screen.queryAllByRole('button')).toHaveLength(0);
  view.unmount();
  render(<InstallSessionNote agentId="opencode" />);
  expect(screen.getByTestId('install-session-record')).toHaveAttribute('data-state', 'unconfirmed');
  act(() => useAgentStore.setState({ installSessions: {} }));
  expect(screen.queryByTestId('install-session-record')).not.toBeInTheDocument();
});
it('does not show another agent history or invent paths without a matched review', () => {
  useAgentStore.setState({ installSessions: { opencode: { state: 'unconfirmed', target: null } } });
  const view = render(<InstallSessionNote agentId="codex" />);
  expect(screen.queryByTestId('install-session-record')).not.toBeInTheDocument();
  view.rerender(<InstallSessionNote agentId="opencode" />);
  expect(screen.getByTestId('install-session-record')).toHaveTextContent('没有可关联的已审核路径快照');
});
