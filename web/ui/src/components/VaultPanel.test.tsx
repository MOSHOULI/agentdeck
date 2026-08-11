import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { VaultPanel } from './VaultPanel';
import type { ApiKeyInfo, BackupInfo } from '../bridge';

vi.mock('../bridge', () => ({
  listApiKeys: vi.fn(),
  saveApiKey: vi.fn(),
  getApiKey: vi.fn(),
  deleteApiKey: vi.fn(),
  importV2Keys: vi.fn(),
  listBackups: vi.fn(),
  restoreBackup: vi.fn(),
}));

import * as bridge from '../bridge';

const keys: ApiKeyInfo[] = [
  { agentId: 'codex', provider: 'manual', maskedKey: 'sk-a***5678' },
];

const backups: BackupInfo[] = [
  {
    hash: 'abc1234',
    timestamp: '1700000000000000000',
    message: 'inject codex',
  },
];

describe('VaultPanel', () => {
  beforeEach(() => {
    vi.mocked(bridge.listApiKeys).mockReset();
    vi.mocked(bridge.saveApiKey).mockReset();
    vi.mocked(bridge.getApiKey).mockReset();
    vi.mocked(bridge.deleteApiKey).mockReset();
    vi.mocked(bridge.importV2Keys).mockReset();
    vi.mocked(bridge.listBackups).mockReset();
    vi.mocked(bridge.restoreBackup).mockReset();
    vi.mocked(bridge.listApiKeys).mockResolvedValue(keys);
    vi.mocked(bridge.listBackups).mockResolvedValue(backups);
    vi.spyOn(window, 'confirm').mockReturnValue(true);
  });

  it('展示密钥列表与备份历史', async () => {
    render(<VaultPanel />);
    await waitFor(() => expect(screen.getByText('sk-a***5678')).toBeInTheDocument());
    expect(screen.getByText('inject codex')).toBeInTheDocument();
  });

  it('保存密钥调用后端并刷新', async () => {
    vi.mocked(bridge.saveApiKey).mockResolvedValue(undefined);
    render(<VaultPanel />);
    await waitFor(() => expect(screen.getByText('sk-a***5678')).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText('名称（如 codex）'), {
      target: { value: 'claude-code' },
    });
    fireEvent.change(screen.getByPlaceholderText('API Key（加密存储，绝不落盘明文）'), {
      target: { value: 'sk-claude-123456' },
    });
    fireEvent.click(screen.getByText('保存'));

    await waitFor(() =>
      expect(bridge.saveApiKey).toHaveBeenCalledWith('claude-code', 'manual', 'sk-claude-123456')
    );
  });

  it('删除密钥带确认并刷新', async () => {
    vi.mocked(bridge.deleteApiKey).mockResolvedValue(true);
    render(<VaultPanel />);
    await waitFor(() => expect(screen.getByText('sk-a***5678')).toBeInTheDocument());

    fireEvent.click(screen.getByText('删除'));
    await waitFor(() => expect(bridge.deleteApiKey).toHaveBeenCalledWith('codex'));
  });

  it('整体恢复走确认并调用 restoreBackup(full=true)', async () => {
    vi.mocked(bridge.restoreBackup).mockResolvedValue(['vault/vault.json']);
    render(<VaultPanel />);
    await waitFor(() => expect(screen.getByText('inject codex')).toBeInTheDocument());

    fireEvent.click(screen.getByText('整体恢复'));
    await waitFor(() =>
      expect(bridge.restoreBackup).toHaveBeenCalledWith('1700000000000000000', true)
    );
  });
});
