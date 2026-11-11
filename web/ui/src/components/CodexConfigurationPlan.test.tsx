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

it('invalidates a displayed plan on another Agent activity and never automatically rereads', async () => {
  render(<CodexConfigurationPlan busy={false} context="one" />);
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  await screen.findByTestId('codex-config-result');
  const other = Symbol('other-agent-operation');
  act(() => useWorkspaceActivityStore.getState().setActivity('agents', other, true));
  expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
  expect(screen.getByTestId('codex-config-preview')).toBeDisabled();
  act(() => useWorkspaceActivityStore.getState().setActivity('agents', other, false));
  expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
  expect(bridge.previewCodexConfiguration).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  await screen.findByTestId('codex-config-result');
  expect(bridge.previewCodexConfiguration).toHaveBeenCalledTimes(2);
});

it('discards a pending plan after another activity while retaining its own owner until settlement', async () => {
  const work = deferred(); vi.mocked(bridge.previewCodexConfiguration).mockReturnValue(work.promise);
  render(<CodexConfigurationPlan busy={false} context="one" />);
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  const own = useWorkspaceActivityStore.getState().owners.agents[0];
  const other = Symbol('other-agent-operation');
  act(() => useWorkspaceActivityStore.getState().setActivity('agents', other, true));
  act(() => useWorkspaceActivityStore.getState().setActivity('agents', other, false));
  expect(useWorkspaceActivityStore.getState().owners.agents).toEqual([own]);
  expect(screen.getByTestId('codex-config-preview')).toBeDisabled();
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  await act(async () => work.resolve());
  expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
  expect(useWorkspaceActivityStore.getState().busy.agents).toBe(false);
  expect(bridge.previewCodexConfiguration).toHaveBeenCalledTimes(1);
});

it('does not miss another activity that starts and finishes within one render batch', async () => {
  render(<CodexConfigurationPlan busy={false} context="one" />);
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  await screen.findByTestId('codex-config-result');
  const other = Symbol('short-agent-operation');
  act(() => {
    useWorkspaceActivityStore.getState().setActivity('agents', other, true);
    useWorkspaceActivityStore.getState().setActivity('agents', other, false);
  });
  expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
  expect(bridge.previewCodexConfiguration).toHaveBeenCalledTimes(1);
});

it('does not invalidate a plan for activity in an unrelated workspace', async () => {
  render(<CodexConfigurationPlan busy={false} context="one" />);
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  await screen.findByTestId('codex-config-result');
  const other = Symbol('memory-operation');
  try {
    act(() => useWorkspaceActivityStore.getState().setActivity('memory', other, true));
    expect(screen.getByTestId('codex-config-result')).toBeInTheDocument();
    expect(screen.getByTestId('codex-config-preview')).toBeEnabled();
  } finally {
    act(() => useWorkspaceActivityStore.getState().setActivity('memory', other, false));
  }
});

it('suppresses late errors after invalidation and never releases another operation owner', async () => {
  let reject!: (reason: Error) => void;
  vi.mocked(bridge.previewCodexConfiguration).mockReturnValue(new Promise((_, fail) => { reject = fail; }));
  render(<CodexConfigurationPlan busy={false} context="one" />);
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  const other = Symbol('other-agent-operation');
  act(() => useWorkspaceActivityStore.getState().setActivity('agents', other, true));
  await act(async () => reject(new Error('old request failed')));
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(useWorkspaceActivityStore.getState().owners.agents).toEqual([other]);
  expect(screen.getByTestId('codex-config-preview')).toBeDisabled();
  act(() => useWorkspaceActivityStore.getState().setActivity('agents', other, false));
  expect(screen.getByTestId('codex-config-preview')).toBeEnabled();
  expect(bridge.previewCodexConfiguration).toHaveBeenCalledTimes(1);
});

it.each(['apply', 'restore'] as const)('invalidates the same-card plan before animation %s and leaves refresh explicit', async operation => {
  vi.spyOn(bridge, 'getCodexAnimationsCapability').mockResolvedValue('linux_managed');
  vi.spyOn(bridge, 'previewCodexAnimations').mockResolvedValue({ field: 'tui.animations', before: true, after: false, changes: true, token: 'a'.repeat(64) });
  vi.spyOn(bridge, 'inspectCodexAnimationsRecovery').mockResolvedValue({ state: 'restore_original', token: 'b'.repeat(64) });
  let finish!: (value: boolean) => void;
  const work = new Promise<boolean>(resolve => { finish = resolve; });
  const apply = vi.spyOn(bridge, 'applyCodexAnimations').mockReturnValue(work);
  const restore = vi.spyOn(bridge, 'restoreCodexAnimations').mockReturnValue(work);
  render(<AgentCard agent={{ ...notDetectedAgent, id: 'codex', name: 'Codex' }} selected={false} dataStale={false} onPrepareInstall={vi.fn()} onInstall={vi.fn()} onUninstall={vi.fn()} onConfigure={vi.fn()} onSelect={vi.fn()} />);
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  await screen.findByTestId('codex-config-result');
  fireEvent.click(screen.getByTestId(`codex-animations-${operation === 'apply' ? 'preview' : 'inspect'}`));
  await screen.findByTestId(`codex-animations-confirm-${operation}`);
  expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
  expect(apply).not.toHaveBeenCalled(); expect(restore).not.toHaveBeenCalled();
  fireEvent.click(screen.getByTestId(`codex-animations-confirm-${operation}`));
  expect(operation === 'apply' ? apply : restore).toHaveBeenCalledTimes(1);
  expect(operation === 'apply' ? restore : apply).not.toHaveBeenCalled();
  expect(screen.getByTestId('codex-config-preview')).toBeDisabled();
  await act(async () => finish(true));
  expect(screen.queryByTestId('codex-config-result')).not.toBeInTheDocument();
  expect(bridge.previewCodexConfiguration).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByTestId('codex-config-preview'));
  await screen.findByTestId('codex-config-result');
  expect(bridge.previewCodexConfiguration).toHaveBeenCalledTimes(2);
});
