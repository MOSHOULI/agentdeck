import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryPanel } from './MemoryPanel';
import type { MemoryEntry } from '../bridge';

vi.mock('../bridge', () => ({
  runtimeMode: vi.fn(() => 'native'),
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

afterEach(() => { cleanup(); vi.restoreAllMocks(); });
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(bridge.runtimeMode).mockReturnValue('native');
  vi.mocked(bridge.getMemoryCollectionStatus).mockResolvedValue({ enabled: false, lastRunAt: null, lastError: null, nextRunAt: null });
  vi.mocked(bridge.setMemoryCollectionEnabled).mockResolvedValue();
  vi.mocked(bridge.searchMemory).mockResolvedValue([entry]);
  vi.mocked(bridge.collectMemory).mockResolvedValue([entry]);
  vi.mocked(bridge.injectMemory).mockResolvedValue();
  vi.mocked(bridge.toggleAutoInject).mockResolvedValue();
});

describe('MemoryPanel', () => {


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

    fireEvent.click(screen.getByRole('button', { name: '导出 Codex 记忆 42' }));
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
  vi.mocked(bridge.getMemoryCollectionStatus)
    .mockResolvedValueOnce({ enabled: false, lastRunAt: null, lastError: null, nextRunAt: null })
    .mockResolvedValueOnce({ enabled: true, lastRunAt: null, lastError: null, nextRunAt: null });
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

async function searchForEntry() {
  fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  await screen.findByText(entry.content);
}

it('labels collection and export separately and distinguishes initial from searched empty states', async () => {
  vi.mocked(bridge.searchMemory).mockResolvedValue([]);
  render(<MemoryPanel />);
  expect(screen.getByRole('heading', { name: '会话采集' })).toBeInTheDocument();
  expect(screen.getByRole('heading', { name: '文件导出' })).toBeInTheDocument();
  expect(screen.getByText('从本地记忆开始')).toBeInTheDocument();
  expect(screen.queryByText('记忆库暂无内容')).not.toBeInTheDocument();
  expect(bridge.searchMemory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '最近记忆' }));
  await screen.findByText('记忆库暂无内容');
  fireEvent.change(screen.getByRole('textbox', { name: '搜索本地记忆' }), { target: { value: 'missing' } });
  fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  await screen.findByText('没有匹配的记忆');
  expect(screen.getByText(/未找到与“missing”匹配的内容/)).toBeInTheDocument();
  expect(screen.queryByText('记忆库暂无内容')).not.toBeInTheDocument();
});

it('shows failed search as an error and retries the exact failed query', async () => {
  vi.mocked(bridge.searchMemory).mockRejectedValueOnce(new Error('FTS unavailable'));
  render(<MemoryPanel />);
  fireEvent.change(screen.getByRole('textbox', { name: '搜索本地记忆' }), { target: { value: 'rust' } });
  fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  expect(await screen.findByRole('alert')).toHaveClass('notice-error');
  fireEvent.change(screen.getByRole('textbox', { name: '搜索本地记忆' }), { target: { value: 'changed' } });
  fireEvent.click(screen.getByRole('button', { name: '重试查询' }));
  await screen.findByText(entry.content);
  expect(bridge.searchMemory).toHaveBeenLastCalledWith('rust');
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

it('does not submit Enter while composing Chinese input', async () => {
  render(<MemoryPanel />);
  fireEvent.keyDown(screen.getByRole('textbox', { name: '搜索本地记忆' }), { key: 'Enter', isComposing: true });
  expect(bridge.searchMemory).not.toHaveBeenCalled();
  await waitFor(() => expect(screen.getByRole('checkbox', { name: /允许每小时自动采集/ })).toBeEnabled());
});

it('retries failed collection settings without pretending an unread setting is disabled', async () => {
  vi.mocked(bridge.getMemoryCollectionStatus).mockRejectedValueOnce(new Error('settings unavailable'));
  render(<MemoryPanel />);
  expect(await screen.findByRole('alert')).toHaveTextContent('读取自动采集设置失败');
  expect(screen.getByText('设置未读取')).toBeInTheDocument();
  expect(screen.getByRole('checkbox', { name: /允许每小时自动采集/ })).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '重试读取设置' }));
  await waitFor(() => expect(screen.getByRole('checkbox', { name: /允许每小时自动采集/ })).toBeEnabled());
  expect(bridge.getMemoryCollectionStatus).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

it('rolls back automatic export on failure and blocks repeated mutation while pending', async () => {
  const pending = deferred<void>();
  vi.mocked(bridge.toggleAutoInject).mockReturnValue(pending.promise);
  render(<MemoryPanel />);
  await searchForEntry();
  const toggle = screen.getByRole('checkbox', { name: 'Codex 记忆 42 的自动导出' });
  fireEvent.click(toggle);
  fireEvent.click(toggle);
  expect(toggle).toBeChecked();
  expect(toggle).toBeDisabled();
  expect(screen.getByRole('button', { name: '导出 Codex 记忆 42' })).toBeDisabled();
  expect(bridge.toggleAutoInject).toHaveBeenCalledTimes(1);
  expect(bridge.toggleAutoInject).toHaveBeenCalledWith('42', true);
  await act(async () => pending.reject(new Error('export write denied')));
  expect(toggle).not.toBeChecked();
  expect(toggle).toBeEnabled();
  expect(screen.getByRole('alert')).toHaveClass('notice-error');
  expect(screen.getByRole('alert')).toHaveTextContent('已恢复原设置');
});

it('explains that automatic revocation preserves manually selected memory', async () => {
  vi.mocked(bridge.searchMemory).mockResolvedValue([{ ...entry, autoInject: true }]);
  render(<MemoryPanel />);
  await searchForEntry();
  fireEvent.click(screen.getByRole('checkbox', { name: 'Codex 记忆 42 的自动导出' }));
  const notice = await screen.findByRole('status');
  expect(bridge.toggleAutoInject).toHaveBeenCalledWith('42', false);
  expect(notice).toHaveClass('notice-warning');
  expect(notice).toHaveTextContent('若此前已手动选择导出，此条仍会保留');
});

it('prevents duplicate exports, reports errors as errors, and unlocks retry', async () => {
  const pending = deferred<void>();
  vi.mocked(bridge.injectMemory).mockReturnValue(pending.promise);
  render(<MemoryPanel />);
  await searchForEntry();
  const button = screen.getByRole('button', { name: '导出 Codex 记忆 42' });
  fireEvent.click(button);
  fireEvent.click(button);
  expect(bridge.injectMemory).toHaveBeenCalledTimes(1);
  expect(button).toBeDisabled();
  await act(async () => pending.reject(new Error('file locked')));
  expect(screen.getByRole('alert')).toHaveTextContent('导出失败');
  expect(screen.getByRole('alert')).toHaveClass('notice-error');
  expect(button).toBeEnabled();
});

it('distinguishes collection failure from successful collection followed by refresh failure', async () => {
  vi.mocked(bridge.collectMemory).mockRejectedValueOnce(new Error('source unreadable'));
  vi.mocked(bridge.searchMemory).mockRejectedValueOnce(new Error('query unavailable'));
  render(<MemoryPanel />);
  fireEvent.click(screen.getByRole('button', { name: '采集 Codex' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('采集失败');
  expect(bridge.searchMemory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '采集 Codex' }));
  expect(await screen.findByRole('status')).toHaveTextContent('已采集 Codex，但记忆列表刷新失败');
  expect(screen.getByRole('status')).toHaveClass('notice-warning');
  expect(screen.getByRole('button', { name: '重试查询' })).toBeEnabled();
});

it('keeps demo searches available while disabling all collection and export writes', async () => {
  vi.mocked(bridge.runtimeMode).mockReturnValue('demo');
  render(<MemoryPanel />);
  await searchForEntry();
  expect(screen.getByRole('button', { name: '采集 Codex' })).toBeDisabled();
  expect(screen.getByRole('checkbox', { name: /允许每小时自动采集/ })).toBeDisabled();
  expect(screen.getByRole('checkbox', { name: 'Codex 记忆 42 的自动导出' })).toBeDisabled();
  expect(screen.getByRole('button', { name: '导出 Codex 记忆 42' })).toBeDisabled();
  expect(bridge.collectMemory).not.toHaveBeenCalled();
  expect(bridge.injectMemory).not.toHaveBeenCalled();
  expect(bridge.toggleAutoInject).not.toHaveBeenCalled();
});

it('shows all returned results rather than silently truncating the last entries', async () => {
  vi.mocked(bridge.searchMemory).mockResolvedValue(Array.from({ length: 50 }, (_, index) => ({ ...entry, id: String(index), content: `Result ${index}` })));
  render(<MemoryPanel />);
  fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  await screen.findByText('Result 49');
  expect(screen.getAllByRole('article')).toHaveLength(50);
  expect(screen.getByText('本次查询 50 条')).toBeInTheDocument();
});

it('keeps a newer search draft when an earlier pending query completes', async () => {
  const pending = deferred<MemoryEntry[]>();
  vi.mocked(bridge.searchMemory).mockReturnValue(pending.promise);
  render(<MemoryPanel />);
  const input = screen.getByRole('textbox', { name: '搜索本地记忆' });
  fireEvent.change(input, { target: { value: 'first query' } });
  fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  fireEvent.change(input, { target: { value: 'next query' } });
  await act(async () => pending.resolve([entry]));
  expect(input).toHaveValue('next query');
  expect(screen.getByText('“first query”的搜索结果')).toBeInTheDocument();
});

it('does not offer working search or mutation controls without a backend', async () => {
  vi.mocked(bridge.runtimeMode).mockReturnValue('unavailable');
  vi.mocked(bridge.getMemoryCollectionStatus).mockRejectedValue(new Error('未连接桌面后端'));
  render(<MemoryPanel />);
  await screen.findByRole('alert');
  expect(screen.getByRole('textbox', { name: '搜索本地记忆' })).toBeDisabled();
  expect(screen.getByRole('button', { name: '搜索' })).toBeDisabled();
  expect(screen.getByRole('button', { name: '采集 Codex' })).toBeDisabled();
  expect(screen.getByRole('button', { name: '重试读取设置' })).toBeDisabled();
  expect(bridge.searchMemory).not.toHaveBeenCalled();
});

it('refreshes collection status explicitly and replaces outdated run times and errors', async () => {
  vi.mocked(bridge.getMemoryCollectionStatus)
    .mockResolvedValueOnce({ enabled: true, lastRunAt: 1700000000, lastError: 'old worker failure', nextRunAt: 1700003600 })
    .mockResolvedValueOnce({ enabled: false, lastRunAt: 1700086400, lastError: null, nextRunAt: null });
  render(<MemoryPanel />);
  await screen.findByText(/最近自动采集失败：old worker failure/);
  const previousRun = screen.getByText(/上次自动采集：/).textContent;
  expect(screen.getByText(/下次计划采集：/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '刷新采集状态' }));
  await waitFor(() => expect(screen.getByRole('checkbox', { name: /允许每小时自动采集/ })).not.toBeChecked());
  expect(screen.getByText(/上次自动采集：/).textContent).not.toBe(previousRun);
  expect(screen.queryByText(/old worker failure/)).not.toBeInTheDocument();
  expect(screen.queryByText(/下次计划采集：/)).not.toBeInTheDocument();
  expect(bridge.getMemoryCollectionStatus).toHaveBeenCalledTimes(2);
  expect(bridge.collectMemory).not.toHaveBeenCalled();
});

it('rereads authoritative collection status after enabling instead of fabricating its schedule', async () => {
  vi.mocked(bridge.getMemoryCollectionStatus)
    .mockResolvedValueOnce({ enabled: false, lastRunAt: 1700000000, lastError: 'old failure', nextRunAt: null })
    .mockResolvedValueOnce({ enabled: true, lastRunAt: 1700086400, lastError: null, nextRunAt: 1700090000 });
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  render(<MemoryPanel />);
  const toggle = screen.getByRole('checkbox', { name: /允许每小时自动采集/ });
  await waitFor(() => expect(toggle).toBeEnabled());
  fireEvent.click(toggle);
  await screen.findByText(/下次计划采集：/);
  expect(toggle).toBeChecked();
  expect(screen.queryByText(/old failure/)).not.toBeInTheDocument();
  expect(bridge.getMemoryCollectionStatus).toHaveBeenCalledTimes(2);
  expect(bridge.setMemoryCollectionEnabled).toHaveBeenCalledTimes(1);
});

it('marks old collection data stale after a failed refresh and hides outdated timing and error fields', async () => {
  vi.mocked(bridge.getMemoryCollectionStatus)
    .mockResolvedValueOnce({ enabled: true, lastRunAt: 1700000000, lastError: 'old failure', nextRunAt: 1700003600 })
    .mockRejectedValueOnce(new Error('status unavailable'));
  render(<MemoryPanel />);
  await screen.findByText(/下次计划采集：/);
  fireEvent.click(screen.getByRole('button', { name: '刷新采集状态' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('status unavailable');
  expect(screen.getByText('状态待刷新')).toBeInTheDocument();
  expect(screen.getByRole('checkbox', { name: /允许每小时自动采集/ })).toBeDisabled();
  expect(screen.queryByText(/下次计划采集：/)).not.toBeInTheDocument();
  expect(screen.queryByText(/上次自动采集：/)).not.toBeInTheDocument();
  expect(screen.queryByText(/old failure/)).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '重试读取设置' })).toBeEnabled();
});

it('does not claim enabling failed or current status is confirmed when only the post-write read fails', async () => {
  vi.mocked(bridge.getMemoryCollectionStatus)
    .mockResolvedValueOnce({ enabled: false, lastRunAt: null, lastError: null, nextRunAt: null })
    .mockRejectedValueOnce(new Error('read unavailable'))
    .mockResolvedValueOnce({ enabled: true, lastRunAt: 1700000000, lastError: null, nextRunAt: 1700003600 });
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  render(<MemoryPanel />);
  const toggle = screen.getByRole('checkbox', { name: /允许每小时自动采集/ });
  await waitFor(() => expect(toggle).toBeEnabled());
  fireEvent.click(toggle);
  expect(await screen.findByRole('alert')).toHaveTextContent('设置已保存，但读取最新状态失败');
  expect(screen.getByRole('status')).toHaveClass('notice-warning');
  expect(screen.getByText('状态待刷新')).toBeInTheDocument();
  expect(toggle).toBeDisabled();
  expect(screen.queryByText('自动采集已开启')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '重试读取设置' }));
  await waitFor(() => expect(toggle).toBeEnabled());
  expect(toggle).toBeChecked();
  expect(screen.getByText('自动采集已开启')).toBeInTheDocument();
  expect(bridge.setMemoryCollectionEnabled).toHaveBeenCalledTimes(1);
});
