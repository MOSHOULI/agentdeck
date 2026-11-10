import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import { CodexAnimations } from './CodexAnimations';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
const preview = { field: 'tui.animations' as const, before: true, after: false, changes: true, token: 'a'.repeat(64) };
beforeEach(() => {
  vi.spyOn(bridge, 'getCodexAnimationsCapability').mockResolvedValue('linux_managed');
  vi.spyOn(bridge, 'observeCodexAnimations').mockResolvedValue({schemaVersion:1,readOnly:true,state:'ready',current:true,applySupported:false,restoreSupported:false});
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'previewCodexAnimations').mockResolvedValue(preview);
  vi.spyOn(bridge, 'applyCodexAnimations').mockResolvedValue(true);
  vi.spyOn(bridge, 'inspectCodexAnimationsRecovery').mockResolvedValue({ state: 'restore_original', token: 'b'.repeat(64) });
  vi.spyOn(bridge, 'restoreCodexAnimations').mockResolvedValue(true);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); for (const t of useWorkspaceActivityStore.getState().owners.agents) useWorkspaceActivityStore.getState().setActivity('agents', t, false); });
async function mount(node: React.ReactElement) { let view!: ReturnType<typeof render>; await act(async () => { view = render(node); }); return view; }
const click = (id: string) => fireEvent.click(screen.getByTestId(`codex-animations-${id}`));
it('requires separate preview/confirmation and cancel never writes', async () => {
  await mount(<CodexAnimations busy={false} context="a" />); click('preview'); await screen.findByTestId('codex-animations-confirm-apply');
  expect(bridge.applyCodexAnimations).not.toHaveBeenCalled(); click('cancel'); expect(screen.queryByTestId('codex-animations-confirm-apply')).toBeNull();
  click('preview'); await screen.findByTestId('codex-animations-confirm-apply'); click('confirm-apply');
  expect(await screen.findByTestId('codex-animations-result')).toHaveAttribute('data-status', 'applied');
  expect(bridge.applyCodexAnimations).toHaveBeenCalledWith(false, preview.token);
});
it('recovery inspection does not restore; explicit confirmation passes independent token', async () => {
  await mount(<CodexAnimations busy={false} context="a" />); click('inspect'); await screen.findByTestId('codex-animations-confirm-restore');
  expect(bridge.restoreCodexAnimations).not.toHaveBeenCalled(); click('cancel'); click('inspect'); await screen.findByTestId('codex-animations-confirm-restore'); click('confirm-restore');
  expect(await screen.findByTestId('codex-animations-result')).toHaveAttribute('data-status', 'restored'); expect(bridge.restoreCodexAnimations).toHaveBeenCalledWith('b'.repeat(64));
});
it('stale errors are redacted, remove token and never auto retry', async () => {
  vi.mocked(bridge.applyCodexAnimations).mockRejectedValue(new Error('SECRET'));
  await mount(<CodexAnimations busy={false} context="a" />); click('preview'); await screen.findByTestId('codex-animations-confirm-apply'); click('confirm-apply');
  expect(await screen.findByRole('alert')).not.toHaveTextContent('SECRET'); expect(screen.queryByTestId('codex-animations-confirm-apply')).toBeNull(); expect(bridge.applyCodexAnimations).toHaveBeenCalledTimes(1);
});
it('retains busy ownership through cancellation and unmount until settlement', async () => {
  let resolve!: (v: typeof preview) => void; vi.mocked(bridge.previewCodexAnimations).mockReturnValue(new Promise(r => { resolve = r; }));
  const view=await mount(<CodexAnimations busy={false} context="a" />); click('preview'); click('preview'); click('cancel'); view.unmount(); await mount(<CodexAnimations busy={false} context="b" />);
  expect(screen.getByTestId('codex-animations-preview')).toBeDisabled(); await act(async () => resolve(preview)); expect(screen.queryByTestId('codex-animations-result')).toBeNull(); expect(bridge.previewCodexAnimations).toHaveBeenCalledTimes(1);
});
it('invalidates confirmation on selected value or context changes', async () => {
  const view=await mount(<CodexAnimations busy={false} context="a" />); click('preview'); await screen.findByTestId('codex-animations-confirm-apply'); click('enabled'); expect(screen.queryByTestId('codex-animations-result')).toBeNull();
  click('inspect'); await screen.findByTestId('codex-animations-confirm-restore'); view.rerender(<CodexAnimations busy={false} context="b" />); expect(screen.queryByTestId('codex-animations-result')).toBeNull();
});
it('browser demo does not simulate or perform file writes', async () => { vi.mocked(bridge.runtimeMode).mockReturnValue('demo'); await mount(<CodexAnimations busy={false} context="a" />); expect(screen.getByTestId('codex-animations-preview')).toBeDisabled(); expect(screen.getByTestId('codex-animations-inspect')).toBeDisabled(); });
it.each(['apply', 'restore'] as const)('keeps %s mutation busy through cancel/unmount and discards late success', async operation => {
  let resolve!: (value: boolean) => void;
  const work = new Promise<boolean>(r => { resolve = r; });
  const mutate = operation === 'apply' ? vi.mocked(bridge.applyCodexAnimations) : vi.mocked(bridge.restoreCodexAnimations);
  mutate.mockReturnValue(work);
  const view = await mount(<CodexAnimations busy={false} context="first" />);
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
  await mount(<CodexAnimations busy={false} context="second" />);
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
it.each(['ready', 'missing', 'unsupported', 'receipt_present'] as const)('Windows shows only read-only observation for %s', async state => {
  vi.mocked(bridge.getCodexAnimationsCapability).mockResolvedValue('windows_read_only');
  vi.mocked(bridge.observeCodexAnimations).mockResolvedValue({schemaVersion:1,readOnly:true,state,current:state==='ready'?true:null,applySupported:false,restoreSupported:false});
  await mount(<CodexAnimations busy={false} context="win" />);
  for (const id of ['enabled','preview','inspect','confirm-apply','confirm-restore']) expect(screen.queryByTestId(`codex-animations-${id}`)).toBeNull();
  click('observe'); expect(await screen.findByTestId('codex-animations-observation')).toHaveAttribute('data-state',state);
  expect(bridge.applyCodexAnimations).not.toHaveBeenCalled(); expect(bridge.restoreCodexAnimations).not.toHaveBeenCalled();
  click('cancel'); expect(screen.queryByTestId('codex-animations-result')).toBeNull();
});
it('fails closed while capability is loading or unavailable', async () => {
  let reject!: (error: Error) => void;
  vi.mocked(bridge.getCodexAnimationsCapability).mockReturnValue(new Promise((_,r) => {reject=r;}));
  await mount(<CodexAnimations busy={false} context="a" />);
  expect(screen.queryByTestId('codex-animations-preview')).toBeNull(); expect(screen.queryByTestId('codex-animations-observe')).toBeNull();
  await act(async () => reject(new Error('SECRET')));
  expect(screen.getByText('当前平台不支持动画配置管理，未读取或修改配置。')).toBeInTheDocument(); expect(document.body).not.toHaveTextContent('SECRET');
});
it('Windows observation busy owner survives cancel and remount; late data is discarded', async () => {
  vi.mocked(bridge.getCodexAnimationsCapability).mockResolvedValue('windows_read_only');
  let resolve!: (r: Awaited<ReturnType<typeof bridge.observeCodexAnimations>>) => void;
  vi.mocked(bridge.observeCodexAnimations).mockReturnValue(new Promise(r=>{resolve=r;}));
  const view=await mount(<CodexAnimations busy={false} context="a" />); click('observe'); click('observe'); click('cancel'); view.unmount();
  await mount(<CodexAnimations busy={false} context="b" />); expect(screen.getByTestId('codex-animations-observe')).toBeDisabled();
  await act(async()=>resolve({schemaVersion:1,readOnly:true,state:'ready',current:false,applySupported:false,restoreSupported:false}));
  expect(screen.queryByTestId('codex-animations-result')).toBeNull(); expect(screen.getByTestId('codex-animations-observe')).toBeEnabled(); expect(bridge.observeCodexAnimations).toHaveBeenCalledTimes(1);
});
