import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { VaultPanel } from './VaultPanel';
import type { ApiKeyInfo, BackupInfo } from '../bridge';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';

vi.mock('../bridge', () => ({
  runtimeMode: vi.fn(),
  listApiKeys: vi.fn(),
  saveApiKey: vi.fn(),
  getApiKey: vi.fn(),
  deleteApiKey: vi.fn(),
  importV2Keys: vi.fn(),
  listBackups: vi.fn(),
  restoreBackup: vi.fn(),
}));

import * as bridge from '../bridge';

const keys: ApiKeyInfo[] = [{ agentId: 'codex', provider: 'manual', maskedKey: 'sk-a***5678' }];
const backups: BackupInfo[] = [{ hash: 'abc1234', timestamp: '1700000000000000000', message: 'inject codex' }];

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

async function ready() {
  await screen.findByText('sk-a***5678');
  await waitFor(() => expect(screen.getByRole('button', { name: '刷新列表' })).toBeEnabled());
}

function fillKey() {
  fireEvent.change(screen.getByLabelText(/^名称/), { target: { value: 'claude-code' } });
  fireEvent.change(screen.getByLabelText(/^API Key/), { target: { value: 'unit-test-secret' } });
}

describe('VaultPanel', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(bridge.runtimeMode).mockReturnValue('native');
    vi.mocked(bridge.listApiKeys).mockResolvedValue(keys);
    vi.mocked(bridge.listBackups).mockResolvedValue(backups);
    vi.mocked(bridge.saveApiKey).mockResolvedValue(undefined);
    vi.mocked(bridge.deleteApiKey).mockResolvedValue(true);
    vi.mocked(bridge.importV2Keys).mockResolvedValue(2);
    vi.mocked(bridge.restoreBackup).mockResolvedValue(['vault/vault.json']);
    vi.spyOn(window, 'confirm').mockReturnValue(true);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('展示脱敏密钥与独立的备份历史，不隐式读取明文', async () => {
    render(<VaultPanel />);
    await ready();
    expect(screen.getByRole('heading', { name: '密钥与备份', level: 1 })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: '密钥保管库' })).toBeInTheDocument();
    expect(within(screen.getByRole('region', { name: '自动备份' })).getByText('inject codex')).toBeInTheDocument();
    expect(bridge.getApiKey).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await waitFor(() => expect(bridge.listApiKeys).toHaveBeenCalledTimes(2));
    expect(bridge.getApiKey).not.toHaveBeenCalled();
  });

  it('持续说明恢复范围仅限管理中的 vault 与配置备份副本', async () => {
    render(<VaultPanel />);
    await ready();
    const recovery = screen.getByRole('region', { name: '自动备份' });
    expect(within(recovery).getByText(/仅恢复 AgentDeck 管理的密钥 vault 与配置备份副本/)).toHaveTextContent('不会卸载或重装软件，也不会自动覆盖外部 Agent 配置');
    expect(bridge.restoreBackup).not.toHaveBeenCalled();
  });

  it('提供可访问的字段标签并始终使用密码输入', async () => {
    render(<VaultPanel />);
    await ready();
    expect(screen.getByLabelText(/^名称/)).toHaveAttribute('required');
    expect(screen.getByLabelText(/^来源/)).toBeInTheDocument();
    expect(screen.getByLabelText(/^API Key/)).toHaveAttribute('type', 'password');
    expect(screen.getByLabelText(/^API Key/)).toHaveAttribute('autocomplete', 'new-password');
    expect(screen.getByRole('button', { name: '复制 codex 的密钥' })).toBeInTheDocument();
  });

  it('保存密钥调用后端、清空输入并刷新，不把密钥写入浏览器存储', async () => {
    const persist = vi.spyOn(Storage.prototype, 'setItem');
    render(<VaultPanel />);
    await ready();
    fillKey();
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    await waitFor(() => expect(bridge.saveApiKey).toHaveBeenCalledWith('claude-code', 'manual', 'unit-test-secret'));
    expect(await screen.findByText('已保存（加密存储）')).toBeInTheDocument();
    expect(screen.getByLabelText(/^API Key/)).toHaveValue('');
    expect(screen.getByLabelText(/^名称/)).toHaveValue('');
    expect(persist).not.toHaveBeenCalled();
    expect(bridge.getApiKey).not.toHaveBeenCalled();
  });

  it('保存中的同步锁会阻止重复提交及其他操作，隐藏工作区不会解除锁', async () => {
    const pending = deferred<void>();
    vi.mocked(bridge.saveApiKey).mockReturnValue(pending.promise);
    const { rerender } = render(<div hidden={false}><VaultPanel /></div>);
    await ready();
    fillKey();
    const form = screen.getByRole('form', { name: '添加密钥' });
    fireEvent.submit(form);
    fireEvent.submit(form);
    expect(bridge.saveApiKey).toHaveBeenCalledTimes(1);
    expect(useWorkspaceActivityStore.getState().busy.vault).toBe(true);
    expect(screen.getByRole('button', { name: '复制 codex 的密钥' })).toBeDisabled();
    rerender(<div hidden><VaultPanel /></div>);
    fireEvent.submit(form);
    expect(bridge.saveApiKey).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve());
    expect(useWorkspaceActivityStore.getState().busy.vault).toBe(false);
  });

  it('空字段或保存错误使用错误语义，错误中不回显提交的密钥', async () => {
    render(<VaultPanel />);
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    expect(screen.getByRole('alert')).toHaveTextContent('名称和密钥不能为空');
    expect(bridge.saveApiKey).not.toHaveBeenCalled();
    fillKey();
    vi.mocked(bridge.saveApiKey).mockRejectedValue(new Error('cannot save unit-test-secret'));
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('保存失败'));
    expect(screen.getByRole('alert')).toHaveClass('notice-error');
    expect(screen.getByRole('alert')).not.toHaveClass('notice-success');
    expect(screen.getByRole('alert')).not.toHaveTextContent('unit-test-secret');
  });

  it('同名替换需要确认，取消后不写入', async () => {
    vi.mocked(window.confirm).mockReturnValue(false);
    render(<VaultPanel />);
    await ready();
    fillKey();
    fireEvent.change(screen.getByLabelText(/^名称/), { target: { value: 'codex' } });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('确认替换'));
    expect(bridge.saveApiKey).not.toHaveBeenCalled();
  });

  it('只在显式复制时读取明文，并阻止重复复制', async () => {
    const pending = deferred<string | null>();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    vi.mocked(bridge.getApiKey).mockReturnValue(pending.promise);
    render(<VaultPanel />);
    await ready();
    expect(bridge.getApiKey).not.toHaveBeenCalled();
    const copy = screen.getByRole('button', { name: '复制 codex 的密钥' });
    fireEvent.click(copy);
    fireEvent.click(copy);
    expect(bridge.getApiKey).toHaveBeenCalledTimes(1);
    expect(bridge.getApiKey).toHaveBeenCalledWith('codex');
    await act(async () => pending.resolve('unit-test-copy-value'));
    expect(writeText).toHaveBeenCalledWith('unit-test-copy-value');
    expect(screen.queryByText('unit-test-copy-value')).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('已复制 codex');
  });

  it('剪贴板失败不把已读取的明文带入错误提示', async () => {
    vi.mocked(bridge.getApiKey).mockResolvedValue('unit-test-copy-value');
    const writeText = vi.fn().mockRejectedValue(new Error('denied unit-test-copy-value'));
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    render(<VaultPanel />);
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '复制 codex 的密钥' }));
    const error = await screen.findByRole('alert');
    expect(error).toHaveTextContent('复制失败');
    expect(error).not.toHaveTextContent('unit-test-copy-value');
    expect(screen.getByRole('button', { name: '复制 codex 的密钥' })).toBeEnabled();
  });

  it('复制失败或找不到密钥时，不显示成功状态', async () => {
    vi.mocked(bridge.getApiKey).mockResolvedValue(null);
    render(<VaultPanel />);
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '复制 codex 的密钥' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('未找到该密钥');
    vi.mocked(bridge.getApiKey).mockRejectedValue(new Error('vault locked'));
    fireEvent.click(screen.getByRole('button', { name: '复制 codex 的密钥' }));
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('复制失败'));
  });

  it('删除密钥带确认并刷新，操作中不可重复删除', async () => {
    const pending = deferred<boolean>();
    vi.mocked(bridge.deleteApiKey).mockReturnValue(pending.promise);
    render(<VaultPanel />);
    await ready();
    const remove = screen.getByRole('button', { name: '删除 codex 的密钥' });
    fireEvent.click(remove);
    fireEvent.click(remove);
    expect(window.confirm).toHaveBeenCalledTimes(1);
    expect(bridge.deleteApiKey).toHaveBeenCalledTimes(1);
    expect(bridge.deleteApiKey).toHaveBeenCalledWith('codex');
    await act(async () => pending.resolve(true));
    expect(screen.getByRole('status')).toHaveTextContent('已删除 codex');
    expect(bridge.listApiKeys).toHaveBeenCalledTimes(2);
  });

  it('取消删除和恢复不会调用后端', async () => {
    vi.mocked(window.confirm).mockReturnValue(false);
    render(<VaultPanel />);
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '删除 codex 的密钥' }));
    fireEvent.click(screen.getByRole('button', { name: '按操作回滚 inject codex' }));
    fireEvent.click(screen.getByRole('button', { name: '整体恢复 inject codex' }));
    expect(window.confirm).toHaveBeenCalledTimes(3);
    expect(bridge.deleteApiKey).not.toHaveBeenCalled();
    expect(bridge.restoreBackup).not.toHaveBeenCalled();
  });

  it('整体恢复明确说明删除后续文件的影响，确认后调用 full=true', async () => {
    const pending = deferred<string[]>();
    vi.mocked(bridge.restoreBackup).mockReturnValue(pending.promise);
    render(<VaultPanel />);
    await ready();
    const restore = screen.getByRole('button', { name: '整体恢复 inject codex' });
    fireEvent.click(restore);
    fireEvent.click(restore);
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('移除之后新增、已纳入备份的文件'));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('不会卸载或重装软件，也不会自动覆盖外部 Agent 配置'));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('仅恢复 AgentDeck 管理的密钥 vault 与配置备份副本'));
    expect(bridge.restoreBackup).toHaveBeenCalledTimes(1);
    expect(bridge.restoreBackup).toHaveBeenCalledWith('1700000000000000000', true);
    await act(async () => pending.resolve(['vault/vault.json']));
    expect(screen.getByRole('status')).toHaveTextContent('已整体恢复 1 个文件');
    expect(bridge.getApiKey).not.toHaveBeenCalled();
  });

  it('按操作回滚说明较小范围并调用 full=false，失败可重试', async () => {
    vi.mocked(bridge.restoreBackup).mockRejectedValueOnce(new Error('missing master key'));
    render(<VaultPanel />);
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '按操作回滚 inject codex' }));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('只还原该次备份变更的文件'));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('不会自动覆盖外部 Agent 配置'));
    expect(bridge.restoreBackup).toHaveBeenCalledWith('1700000000000000000', false);
    expect(await screen.findByRole('alert')).toHaveTextContent('missing master key');
    expect(screen.getByRole('button', { name: '按操作回滚 inject codex' })).toBeEnabled();
  });

  it('V2 导入调用真实接口，不重复导入且报告失败', async () => {
    const pending = deferred<number>();
    vi.mocked(bridge.importV2Keys).mockReturnValue(pending.promise);
    render(<VaultPanel />);
    await ready();
    const importButton = screen.getByRole('button', { name: '导入 V2 vault 密钥' });
    fireEvent.click(importButton);
    fireEvent.click(importButton);
    expect(bridge.importV2Keys).toHaveBeenCalledTimes(1);
    await act(async () => pending.reject(new Error('V2 vault unavailable')));
    expect(screen.getByRole('alert')).toHaveTextContent('V2 导入失败');
    expect(screen.queryByRole('button', { name: /创建备份/ })).not.toBeInTheDocument();
  });

  it('加载失败不伪装成空列表，另一资源仍能显示', async () => {
    vi.mocked(bridge.listApiKeys).mockRejectedValue(new Error('key service unavailable'));
    render(<VaultPanel />);
    expect(await screen.findByRole('alert')).toHaveTextContent('密钥加载失败');
    expect(screen.getByText('inject codex')).toBeInTheDocument();
    expect(screen.queryByText('暂无密钥')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '刷新列表' })).toBeEnabled();
    vi.mocked(bridge.listApiKeys).mockResolvedValue(keys);
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await ready();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it.each(['initial', 'refresh'] as const)('%s 密钥读取失败时阻止写入，成功重试后仍需确认同名替换', async (failure) => {
    if (failure === 'initial') {
      vi.mocked(bridge.listApiKeys).mockRejectedValueOnce(new Error('list unavailable'));
    }
    render(<VaultPanel />);
    if (failure === 'refresh') {
      await ready();
      vi.mocked(bridge.listApiKeys).mockRejectedValueOnce(new Error('list unavailable'));
      fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    }
    expect(await screen.findByText(/密钥加载失败：Error: list unavailable/)).toBeInTheDocument();
    fillKey();
    fireEvent.change(screen.getByLabelText(/^名称/), { target: { value: 'codex' } });
    expect(screen.getByRole('button', { name: '保存' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '导入 V2 vault 密钥' })).toBeDisabled();
    if (failure === 'refresh') {
      expect(screen.getByRole('button', { name: '删除 codex 的密钥' })).toBeDisabled();
      expect(screen.getByRole('button', { name: '复制 codex 的密钥' })).toBeDisabled();
    }
    // A form submission also fails closed, even without relying on disabled buttons.
    fireEvent.submit(screen.getByRole('form', { name: '添加密钥' }));
    expect(bridge.saveApiKey).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();
    expect(screen.getByText(/请先刷新并成功读取密钥列表/)).toBeInTheDocument();
    expect(screen.getByLabelText(/^API Key/)).toHaveValue('unit-test-secret');
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await ready();
    expect(screen.getByRole('button', { name: '保存' })).toBeEnabled();
    expect(screen.getByLabelText(/^名称/)).toHaveValue('codex');
    expect(screen.getByLabelText(/^API Key/)).toHaveValue('unit-test-secret');
    expect(screen.queryByText(/请先刷新并成功读取密钥列表/)).not.toBeInTheDocument();
    vi.mocked(window.confirm).mockReturnValueOnce(false);
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('已存在 codex'));
    expect(bridge.saveApiKey).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    await waitFor(() => expect(bridge.saveApiKey).toHaveBeenCalledWith('codex', 'manual', 'unit-test-secret'));
    await screen.findByText('已保存（加密存储）');
  });

  it('备份加载失败不会隐藏已读取的密钥，也不报告零条备份', async () => {
    vi.mocked(bridge.listBackups).mockRejectedValue(new Error('git unavailable'));
    render(<VaultPanel />);
    await ready();
    expect(screen.getByRole('alert')).toHaveTextContent('备份加载失败');
    expect(screen.queryByText('暂无备份记录')).not.toBeInTheDocument();
    expect(screen.queryByText('0 条记录')).not.toBeInTheDocument();
  });

  it('保存成功后刷新失败，不把成功写入误报成保存失败', async () => {
    render(<VaultPanel />);
    await ready();
    vi.mocked(bridge.listApiKeys).mockRejectedValue(new Error('refresh unavailable'));
    fillKey();
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    expect(await screen.findByText('已保存（加密存储）')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('密钥加载失败');
    expect(screen.getByRole('alert')).toHaveTextContent('上次读取的列表');
    expect(screen.queryByText(/保存失败/)).not.toBeInTheDocument();
    expect(screen.getByLabelText(/^API Key/)).toHaveValue('');
  });

  it('初始加载期间禁止修改并报告工作区活动', async () => {
    const pending = deferred<ApiKeyInfo[]>();
    vi.mocked(bridge.listApiKeys).mockReturnValue(pending.promise);
    render(<VaultPanel />);
    expect(screen.getByRole('button', { name: '保存' })).toBeDisabled();
    expect(useWorkspaceActivityStore.getState().busy.vault).toBe(true);
    await act(async () => pending.resolve(keys));
    expect(useWorkspaceActivityStore.getState().busy.vault).toBe(false);
  });

  it('演示模式禁用敏感操作，不允许读取密钥或恢复备份', async () => {
    vi.mocked(bridge.runtimeMode).mockReturnValue('demo');
    render(<VaultPanel />);
    await ready();
    expect(screen.getByRole('status')).toHaveTextContent('密钥与备份均为示例');
    expect(screen.getByLabelText(/^API Key/)).toBeDisabled();
    for (const name of ['保存', '导入 V2 vault 密钥', '复制 codex 的密钥', '删除 codex 的密钥', '按操作回滚 inject codex', '整体恢复 inject codex']) {
      const button = screen.getByRole('button', { name });
      expect(button).toBeDisabled();
      fireEvent.click(button);
    }
    expect(bridge.getApiKey).not.toHaveBeenCalled();
    expect(bridge.restoreBackup).not.toHaveBeenCalled();
    expect(bridge.saveApiKey).not.toHaveBeenCalled();
  });

  it('可展开二十条以后的备份，异常时间不破坏渲染', async () => {
    vi.mocked(bridge.listBackups).mockResolvedValue(Array.from({ length: 21 }, (_, index) => ({
      hash: `test-${index}`, timestamp: 'not-a-timestamp', message: `backup ${index}`,
    })));
    render(<VaultPanel />);
    await ready();
    expect(screen.queryByText('backup 20')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '显示更多备份（剩余 1 条）' }));
    expect(screen.getByText('backup 20')).toBeInTheDocument();
    expect(screen.getAllByText('时间未知')).toHaveLength(21);
  });
});
