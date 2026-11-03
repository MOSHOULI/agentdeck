import { afterEach, expect, it } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import App from './App';
import { useAgentStore } from './stores/agentStore';
afterEach(() => {
  cleanup();
  window.history.replaceState({}, '', '/');
  act(() => useAgentStore.setState({ agents: [], loading: false, error: null }));
});
it('visibly labels read-only demo data', async () => {
  window.history.replaceState({}, '', '/?demo=1');
  render(<App />);
  expect(screen.getByText(/只读演示模式 · 所有数据均为示例/)).toBeInTheDocument();
  await screen.findByText('OpenCode');
});
it('never renders installed sample agents without explicit demo', async () => {
  render(<App />);
  expect(screen.getByText(/未连接桌面后端 · 请在/)).toBeInTheDocument();
  await screen.findByRole('alert');
  expect(screen.queryByText('OpenCode')).not.toBeInTheDocument();
});
