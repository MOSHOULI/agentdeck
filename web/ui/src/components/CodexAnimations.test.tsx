import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import { CodexAnimations } from './CodexAnimations';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
const preview = { field: 'tui.animations' as const, before: true, after: false, changes: true, token: 'a'.repeat(64) };
beforeEach(() => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'previewCodexAnimations').mockResolvedValue(preview);
  vi.spyOn(bridge, 'applyCodexAnimations').mockResolvedValue(true);
  vi.spyOn(bridge, 'inspectCodexAnimationsRecovery').mockResolvedValue({ state: 'restore_original', token: 'b'.repeat(64) });
  vi.spyOn(bridge, 'restoreCodexAnimations').mockResolvedValue(true);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); for (const t of useWorkspaceActivityStore.getState().owners.agents) useWorkspaceActivityStore.getState().setActivity('agents', t, false); });
const click = (id: string) => fireEvent.click(screen.getByTestId(`codex-animations-${id}`));
it('requires separate preview/confirmation and cancel never writes', async () => {
  render(<CodexAnimations busy={false} context="a" />); click('preview'); await screen.findByTestId('codex-animations-confirm-apply');
  expect(bridge.applyCodexAnimations).not.toHaveBeenCalled(); click('cancel'); expect(screen.queryByTestId('codex-animations-confirm-apply')).toBeNull();
  click('preview'); await screen.findByTestId('codex-animations-confirm-apply'); click('confirm-apply');
  expect(await screen.findByTestId('codex-animations-result')).toHaveAttribute('data-status', 'applied');
  expect(bridge.applyCodexAnimations).toHaveBeenCalledWith(false, preview.token);
});
it('recovery inspection does not restore; explicit confirmation passes independent token', async () => {
  render(<CodexAnimations busy={false} context="a" />); click('inspect'); await screen.findByTestId('codex-animations-confirm-restore');
  expect(bridge.restoreCodexAnimations).not.toHaveBeenCalled(); click('cancel'); click('inspect'); await screen.findByTestId('codex-animations-confirm-restore'); click('confirm-restore');
  expect(await screen.findByTestId('codex-animations-result')).toHaveAttribute('data-status', 'restored'); expect(bridge.restoreCodexAnimations).toHaveBeenCalledWith('b'.repeat(64));
});
it('stale errors are redacted, remove token and never auto retry', async () => {
  vi.mocked(bridge.applyCodexAnimations).mockRejectedValue(new Error('SECRET'));
  render(<CodexAnimations busy={false} context="a" />); click('preview'); await screen.findByTestId('codex-animations-confirm-apply'); click('confirm-apply');
  expect(await screen.findByRole('alert')).not.toHaveTextContent('SECRET'); expect(screen.queryByTestId('codex-animations-confirm-apply')).toBeNull(); expect(bridge.applyCodexAnimations).toHaveBeenCalledTimes(1);
});
it('retains busy ownership through cancellation and unmount until settlement', async () => {
  let resolve!: (v: typeof preview) => void; vi.mocked(bridge.previewCodexAnimations).mockReturnValue(new Promise(r => { resolve = r; }));
  const view=render(<CodexAnimations busy={false} context="a" />); click('preview'); click('preview'); click('cancel'); view.unmount(); render(<CodexAnimations busy={false} context="b" />);
  expect(screen.getByTestId('codex-animations-preview')).toBeDisabled(); await act(async () => resolve(preview)); expect(screen.queryByTestId('codex-animations-result')).toBeNull(); expect(bridge.previewCodexAnimations).toHaveBeenCalledTimes(1);
});
it('invalidates confirmation on selected value or context changes', async () => {
  const view=render(<CodexAnimations busy={false} context="a" />); click('preview'); await screen.findByTestId('codex-animations-confirm-apply'); click('enabled'); expect(screen.queryByTestId('codex-animations-result')).toBeNull();
  click('inspect'); await screen.findByTestId('codex-animations-confirm-restore'); view.rerender(<CodexAnimations busy={false} context="b" />); expect(screen.queryByTestId('codex-animations-result')).toBeNull();
});
it('browser demo does not simulate or perform file writes', () => { vi.mocked(bridge.runtimeMode).mockReturnValue('demo'); render(<CodexAnimations busy={false} context="a" />); expect(screen.getByTestId('codex-animations-preview')).toBeDisabled(); expect(screen.getByTestId('codex-animations-inspect')).toBeDisabled(); });
it.each(['apply', 'restore'] as const)('keeps %s mutation busy through cancel/unmount and discards late success', async operation => {
  let resolve!: (value: boolean) => void;
  const work = new Promise<boolean>(r => { resolve = r; });
  const mutate = operation === 'apply' ? vi.mocked(bridge.applyCodexAnimations) : vi.mocked(bridge.restoreCodexAnimations);
  mutate.mockReturnValue(work);
  const view = render(<CodexAnimations busy={false} context="first" />);
  click(operation === 'apply' ? 'preview' : 'inspect');
  await screen.findByTestId(`codex-animations-confirm-${operation}`);
  click(`confirm-${operation}`);
  expect(mutate).toHaveBeenCalledTimes(1);
  expect(useWorkspaceActivityStore.getState().busy.agents).toBe(true);
  click('cancel');
  expect(screen.queryByTestId('codex-animations-result')).toBeNull();
  expect(screen.getByTestId('codex-animations-preview')).toBeDisabled();
  view.unmount();
  expect(useWorkspaceActivityStore.getState().busy.agents).toBe(true);
  render(<CodexAnimations busy={false} context="second" />);
  expect(screen.getByTestId('codex-animations-preview')).toBeDisabled();
  expect(screen.getByTestId('codex-animations-inspect')).toBeDisabled();
  click('preview'); click('inspect');
  expect(mutate).toHaveBeenCalledTimes(1);
  await act(async () => resolve(true));
  expect(useWorkspaceActivityStore.getState().busy.agents).toBe(false);
  expect(screen.queryByTestId('codex-animations-result')).toBeNull();
  expect(screen.getByTestId('codex-animations-preview')).toBeEnabled();
  expect(screen.getByTestId('codex-animations-inspect')).toBeEnabled();
  expect(mutate).toHaveBeenCalledTimes(1);
});
