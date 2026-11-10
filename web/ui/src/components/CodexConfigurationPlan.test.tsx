import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import { demoCodexConfigurationPlan } from '../codexConfigurationContract';
import { CodexConfigurationPlan } from './CodexConfigurationPlan';
import { AgentCard } from './AgentCard';
import { notDetectedAgent } from '../test-fixtures/agents';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
const result = { plan: demoCodexConfigurationPlan(), synthetic: false };
beforeEach(() => { vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native'); vi.spyOn(bridge, 'previewCodexConfiguration').mockResolvedValue(result); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); for (const token of useWorkspaceActivityStore.getState().owners.agents) useWorkspaceActivityStore.getState().setActivity('agents', token, false); });
function deferred() { let resolve!: (r: typeof result) => void; return { promise: new Promise<typeof result>(r => { resolve = r; }), resolve: () => resolve(result) }; }
it('does not read until action; displays fixed result without Apply', async () => {
  render(<CodexConfigurationPlan busy={false} context="one" />);
  expect(bridge.previewCodexConfiguration).not.toHaveBeenCalled();
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  expect(await screen.findByTestId('codex-config-result')).toHaveTextContent('应用功能尚未开放');
  expect(screen.queryByRole('button', { name: /应用|确认/ })).not.toBeInTheDocument();
  fireEvent.click(screen.getByText('清除 Codex 配置计划')); expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
});
it('keeps operation owner through clear/unmount and rejects duplicate reads until actual settlement', async () => {
  const work = deferred(); vi.mocked(bridge.previewCodexConfiguration).mockReturnValue(work.promise);
  const view = render(<CodexConfigurationPlan busy={false} context="one" />);
  const button = screen.getByTestId('codex-config-preview'); fireEvent.click(button); fireEvent.click(button);
  fireEvent.click(screen.getByText('清除 Codex 配置计划')); view.unmount();
  render(<CodexConfigurationPlan busy={false} context="two" />);
  expect(screen.getByTestId('codex-config-preview')).toBeDisabled();
  expect(bridge.previewCodexConfiguration).toHaveBeenCalledTimes(1);
  await act(async () => work.resolve());
  expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
  expect(screen.getByTestId('codex-config-preview')).toBeEnabled();
});
it('discards pending or displayed result when review context changes', async () => {
  const work = deferred(); vi.mocked(bridge.previewCodexConfiguration).mockReturnValue(work.promise);
  const view = render(<CodexConfigurationPlan busy={false} context="one" />); fireEvent.click(screen.getByTestId('codex-config-preview'));
  view.rerender(<CodexConfigurationPlan busy={false} context="two" />); await act(async () => work.resolve());
  expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
});
it('hides raw errors and permits a fresh explicit retry', async () => {
  vi.mocked(bridge.previewCodexConfiguration).mockRejectedValue(new Error('SECRET /private'));
  render(<CodexConfigurationPlan busy={false} context="one" />); fireEvent.click(screen.getByTestId('codex-config-preview'));
  expect(await screen.findByRole('alert')).not.toHaveTextContent('SECRET'); expect(screen.getByTestId('codex-config-preview')).toBeEnabled();
});
it('offers Codex plan with missing installation and stale catalog; never legacy apply', () => {
  render(<AgentCard agent={{ ...notDetectedAgent, id: 'codex', name: 'Codex', canConfigure: true }} selected={false} dataStale onPrepareInstall={vi.fn()} onInstall={vi.fn()} onUninstall={vi.fn()} onConfigure={vi.fn()} onSelect={vi.fn()} />);
  expect(screen.getByTestId('codex-config-preview')).toBeEnabled(); expect(screen.queryByRole('button', { name: '应用默认配置 Codex' })).not.toBeInTheDocument();
});
it('labels demo and blocks unavailable mode', () => {
  vi.mocked(bridge.runtimeMode).mockReturnValue('demo'); const view=render(<CodexConfigurationPlan busy={false} context="one" />);
  expect(screen.getByRole('button', { name: '查看 Codex 合成配置计划' })).toBeEnabled();
  vi.mocked(bridge.runtimeMode).mockReturnValue('unavailable'); view.rerender(<CodexConfigurationPlan busy={false} context="two" />); expect(screen.getByTestId('codex-config-preview')).toBeDisabled();
});
