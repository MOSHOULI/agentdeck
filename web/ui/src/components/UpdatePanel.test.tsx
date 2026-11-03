import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { UpdatePanel } from './UpdatePanel';
import type { UpdateSummary } from '../bridge';

vi.mock('../bridge', () => ({
  checkUpdates: vi.fn(),
  applyUpdates: vi.fn(),
  getAppVersion: vi.fn(),
}));

import * as bridge from '../bridge';

const summary: UpdateSummary = {
  version: '1.1.0',
  released: '2026-08-11',
  files: ['registry-v3.json'],
  pubkeyFp: 'dc83b35677939e36',
};

describe('UpdatePanel', () => {
  beforeEach(() => {
    vi.mocked(bridge.checkUpdates).mockReset();
    vi.mocked(bridge.applyUpdates).mockReset();
    vi.mocked(bridge.getAppVersion).mockReset();
    vi.mocked(bridge.getAppVersion).mockResolvedValue('v1.0.0');
  });

  it('无更新时显示已是最新', async () => {
    vi.mocked(bridge.checkUpdates).mockResolvedValue(null);
    render(<UpdatePanel />);
    await waitFor(() => expect(screen.getByText('已是最新注册表。')).toBeInTheDocument());
  });

  it('有更新时显示新版并支持一键应用', async () => {
    vi.mocked(bridge.checkUpdates).mockResolvedValue(summary);
    vi.mocked(bridge.applyUpdates).mockResolvedValue(['registry-v3.json']);
    render(<UpdatePanel />);
    await waitFor(() =>
      expect(screen.getByText(/发现新版本 1.1.0/)).toBeInTheDocument()
    );

    fireEvent.click(screen.getByText('应用更新'));
    await waitFor(() => expect(bridge.applyUpdates).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByText(/已更新：registry-v3.json/)).toBeInTheDocument());
  });

  it('检查失败显示错误', async () => {
    vi.mocked(bridge.checkUpdates).mockRejectedValue(new Error('签名校验失败'));
    render(<UpdatePanel />);
    await waitFor(() =>
      expect(screen.getByText(/更新失败/)).toBeInTheDocument()
    );
  });
});

it('cannot check again or apply twice while applying', async () => {
  vi.mocked(bridge.checkUpdates).mockReset().mockResolvedValue(summary);
  vi.mocked(bridge.getAppVersion).mockResolvedValue('v1.0.0');
  let resolve!: (files: string[]) => void;
  vi.mocked(bridge.applyUpdates).mockReset().mockImplementation(() => new Promise((done) => { resolve = done; }));
  render(<UpdatePanel />);
  const apply = await screen.findByText('应用更新');
  fireEvent.click(apply);
  fireEvent.click(apply);
  expect(bridge.applyUpdates).toHaveBeenCalledTimes(1);
  expect(screen.getByText('检查更新')).toBeDisabled();
  resolve(['registry-v3.json']);
  await screen.findByText(/已更新：registry-v3.json/);
});
