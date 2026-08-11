import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryPanel } from './MemoryPanel';
import type { MemoryEntry } from '../bridge';

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
  });

  it('点击采集按钮触发 collectMemory', async () => {
    vi.mocked(bridge.searchMemory).mockResolvedValue([]);
    vi.mocked(bridge.collectMemory).mockResolvedValue([entry]);
    render(<MemoryPanel />);

    fireEvent.click(screen.getByText('采集 OpenCode'));
    await waitFor(() => expect(bridge.collectMemory).toHaveBeenCalledWith('opencode'));
  });
});
