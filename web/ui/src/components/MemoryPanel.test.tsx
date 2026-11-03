import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryPanel } from './MemoryPanel';
import type { MemoryEntry } from '../bridge';

vi.mock('../bridge', () => ({
  getMemoryCollectionStatus: vi.fn().mockResolvedValue({ enabled: false, lastRunAt: null, lastError: null, nextRunAt: null }),
  setMemoryCollectionEnabled: vi.fn(),
  getAgents: vi.fn(),
  installAgent: vi.fn(),
  uninstallAgent: vi.fn(),
  verifyAgentAuth: vi.fn(),
  collectMemory: vi.fn(),
  searchMemory: vi.fn(),
  injectMemory: vi.fn(),
  toggleAutoInject: vi.fn(),
}));

import * as bridge from '../bridge';

const entry: MemoryEntry = {
  id: '42',
  agentId: 'codex',
  content: 'rust memory engine FTS5 测试记忆',
  collectedAt: 1700000000,
  autoInject: false,
  tags: ['codex'],
};

describe('MemoryPanel', () => {
  beforeEach(() => {
    vi.mocked(bridge.searchMemory).mockReset();
    vi.mocked(bridge.collectMemory).mockReset();
    vi.mocked(bridge.injectMemory).mockReset();
    vi.mocked(bridge.toggleAutoInject).mockReset();
  });

  it('搜索后展示结果并支持注入', async () => {
    vi.mocked(bridge.searchMemory).mockResolvedValue([entry]);
    render(<MemoryPanel />);

    fireEvent.change(screen.getByPlaceholderText('全文搜索记忆（FTS5）'), {
      target: { value: 'rust' },
    });
    fireEvent.click(screen.getByText('搜索'));

    await waitFor(() =>
      expect(screen.getByText('rust memory engine FTS5 测试记忆')).toBeInTheDocument()
    );
    expect(bridge.searchMemory).toHaveBeenCalledWith('rust');

    fireEvent.click(screen.getByText('注入'));
    await waitFor(() =>
      expect(bridge.injectMemory).toHaveBeenCalledWith('codex', ['42'])
    );
    await screen.findByText(/不代表 Agent 已读取/);
  });

  it('点击采集按钮触发 collectMemory', async () => {
    vi.mocked(bridge.searchMemory).mockResolvedValue([]);
    vi.mocked(bridge.collectMemory).mockResolvedValue([entry]);
    render(<MemoryPanel />);

    fireEvent.click(screen.getByText('采集 OpenCode'));
    await waitFor(() => expect(bridge.collectMemory).toHaveBeenCalledWith('opencode'));
  });
});

it('reports search failure and unlocks retry', async () => {
  vi.mocked(bridge.searchMemory).mockRejectedValueOnce(new Error('FTS unavailable'));
  render(<MemoryPanel />);
  fireEvent.click(screen.getByText('搜索'));
  await screen.findByText(/搜索失败:.*FTS unavailable/);
  expect(screen.getByText('搜索')).toBeEnabled();
});

it('blocks repeated Enter and collecting while a search is pending', async () => {
  let resolve!: (entries: MemoryEntry[]) => void;
  vi.mocked(bridge.searchMemory).mockReset().mockImplementation(() => new Promise((done) => { resolve = done; }));
  render(<MemoryPanel />);
  const input = screen.getByPlaceholderText('全文搜索记忆（FTS5）');
  fireEvent.keyDown(input, { key: 'Enter' });
  fireEvent.keyDown(input, { key: 'Enter' });
  expect(bridge.searchMemory).toHaveBeenCalledTimes(1);
  expect(screen.getByText('采集 Codex')).toBeDisabled();
  resolve([]);
  await waitFor(() => expect(screen.getByText('搜索')).toBeEnabled());
});

it('automatic collection defaults off and enabling requires confirmation', async () => {
  vi.mocked(bridge.setMemoryCollectionEnabled).mockReset().mockResolvedValue();
  vi.spyOn(window, 'confirm').mockReturnValue(false);
  render(<MemoryPanel />);
  const toggle = screen.getByRole('checkbox', { name: /允许每小时自动采集/ });
  await waitFor(() => expect(toggle).toBeEnabled());
  expect(toggle).not.toBeChecked();
  fireEvent.click(toggle);
  expect(bridge.setMemoryCollectionEnabled).not.toHaveBeenCalled();
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle).toBeChecked());
  expect(bridge.setMemoryCollectionEnabled).toHaveBeenCalledWith(true);
});

it('failed automatic collection setting preserves disabled state', async () => {
  vi.mocked(bridge.setMemoryCollectionEnabled).mockRejectedValueOnce(new Error('settings write denied'));
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  render(<MemoryPanel />);
  const toggle = screen.getByRole('checkbox', { name: /允许每小时自动采集/ });
  await waitFor(() => expect(toggle).toBeEnabled());
  fireEvent.click(toggle);
  await screen.findByText(/更新自动采集设置失败:.*settings write denied/);
  expect(toggle).not.toBeChecked();
  expect(toggle).toBeEnabled();
});

it('can disable persisted automatic collection without another confirmation', async () => {
  vi.mocked(bridge.getMemoryCollectionStatus).mockResolvedValueOnce({ enabled: true, lastRunAt: null, lastError: null, nextRunAt: null });
  vi.mocked(bridge.setMemoryCollectionEnabled).mockReset().mockResolvedValue();
  const confirm = vi.spyOn(window, 'confirm').mockClear();
  render(<MemoryPanel />);
  const toggle = screen.getByRole('checkbox', { name: /允许每小时自动采集/ });
  await waitFor(() => expect(toggle).toBeChecked());
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle).not.toBeChecked());
  expect(bridge.setMemoryCollectionEnabled).toHaveBeenCalledWith(false);
  expect(confirm).not.toHaveBeenCalled();
});
