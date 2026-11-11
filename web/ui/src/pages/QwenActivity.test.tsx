import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { StrictMode } from 'react';
import * as bridge from '../bridge';
import { qwenDemoSummary } from '../qwenContract';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
import { QwenSettings } from './QwenSettings';

const inspection = { summary: qwenDemoSummary(), synthetic: false };
const preview = { field: 'general.vimMode' as const, before: null, after: true, changes: true, token: 'a'.repeat(64) };
const restorePreview = { field: 'general.vimMode' as const, state: 'restore_field' as const, current: true, initial: null, changesSettings: true, token: 'c'.repeat(64) };
const recovery = { field: 'general.vimMode' as const, state: 'restore_field' as const, changesSettings: true };
const actions = ['inspect', 'preview', 'update-preview', 'restore-preview', 'recovery', 'apply', 'update', 'restore'] as const;
type Action = typeof actions[number];
const pending: (() => void)[] = [];
const activity = () => useWorkspaceActivityStore.getState();
const inspect = () => fireEvent.submit(screen.getByRole('form', { name: 'Qwen 检查路径' }));
const buttonNames = {
  'restore-preview': '撤销受管 Vim 设置', inspect: '检查所选文件', preview: '预览 Vim 变更', recovery: '只读检查 Vim 恢复状态',
  'update-preview': '预览更新受管 Vim 值', update: '确认更新受管 Vim 值', apply: '确认应用 Vim 变更', restore: '确认撤销 Vim 设置',
} satisfies Record<Action, string>;
function fill() {
  fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/selected/settings.json' } });
  fireEvent.change(screen.getByLabelText('运行时 package.json 绝对路径'), { target: { value: '/runtime/package.json' } });
}
function defer<T>(fallback: T) {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  pending.push(() => resolve(fallback));
  return { promise, resolve: () => resolve(fallback), reject: () => reject(new Error('SECRET stale failure')) };
}
function hold(action: Action) {
  if (action === 'inspect') {
    const work = defer(inspection); vi.mocked(bridge.inspectQwenSettings).mockReturnValueOnce(work.promise); return work;
  }
  if (action === 'preview') {
    const work = defer(preview); vi.mocked(bridge.previewQwenVim).mockReturnValueOnce(work.promise); return work;
  }
  if (action === 'update-preview') {
    const work = defer(preview); vi.mocked(bridge.previewQwenVimUpdate).mockReturnValueOnce(work.promise); return work;
  }
  if (action === 'restore-preview') {
    const work = defer(restorePreview); vi.mocked(bridge.previewQwenVimRestore).mockReturnValueOnce(work.promise); return work;
  }
  if (action === 'recovery') {
    const work = defer(recovery); vi.mocked(bridge.inspectQwenVimRecovery).mockReturnValueOnce(work.promise); return work;
  }
  const work = defer(true);
  vi.mocked(action === 'apply' ? bridge.applyQwenVim : action === 'update' ? bridge.updateQwenVim : bridge.restoreQwenVim).mockReturnValueOnce(work.promise);
  return work;
}
async function prepare(action: Action) {
  if (action === 'apply') {
    fireEvent.click(screen.getByRole('button', { name: buttonNames.preview }));
    await screen.findByRole('dialog', { name: '确认 Vim 变更' });
  } else if (action === 'update') {
    fireEvent.click(screen.getByRole('button', { name: buttonNames['update-preview'] }));
    await screen.findByRole('dialog', { name: '确认更新受管 Vim 值' });
  } else if (action === 'restore') {
    fireEvent.click(screen.getByRole('button', { name: '撤销受管 Vim 设置' }));
    await screen.findByRole('dialog', { name: '确认撤销 Vim 设置' });
  }
}
function calls() {
  return [bridge.previewQwenVimRestore, bridge.inspectQwenSettings, bridge.previewQwenVimUpdate, bridge.updateQwenVim, bridge.previewQwenVim, bridge.inspectQwenVimRecovery, bridge.applyQwenVim, bridge.restoreQwenVim]
    .map(fn => vi.mocked(fn).mock.calls.length);
}
function expectNoResults() {
  expect(screen.queryByRole('region', { name: 'Qwen 检查结果' })).not.toBeInTheDocument();
  expect(screen.queryByRole('region', { name: 'Vim 恢复状态观察' })).not.toBeInTheDocument();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(screen.queryByText(/受管 Vim 值已更新|字段已应用|受管字段已撤销|操作完成，未改变/)).not.toBeInTheDocument();
}
function expectBlocked() {
  for (const name of [buttonNames.inspect, buttonNames.preview, buttonNames['update-preview'], buttonNames.recovery, '撤销受管 Vim 设置']) {
    expect(screen.getByRole('button', { name })).toBeDisabled();
  }
}
beforeEach(() => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'inspectQwenSettings').mockResolvedValue(inspection);
  vi.spyOn(bridge, 'previewQwenVim').mockResolvedValue(preview);
  vi.spyOn(bridge, 'inspectQwenVimRecovery').mockResolvedValue(recovery);
  vi.spyOn(bridge, 'applyQwenVim').mockResolvedValue(true);
  vi.spyOn(bridge, 'previewQwenVimUpdate').mockResolvedValue(preview);
  vi.spyOn(bridge, 'updateQwenVim').mockResolvedValue(true);
  vi.spyOn(bridge, 'previewQwenVimRestore').mockResolvedValue(restorePreview);
  vi.spyOn(bridge, 'restoreQwenVim').mockResolvedValue(true);
});
afterEach(async () => {
  cleanup();
  await act(async () => { pending.splice(0).forEach(settle => settle()); });
  for (const owner of activity().owners.qwen) activity().setActivity('qwen', owner, false);
  vi.restoreAllMocks();
});

describe('Qwen pending operation ownership', () => {
  it.each(actions.flatMap(action => (['resolve', 'reject'] as const).map(outcome => ({ action, outcome }))))(
    'retains pending $action across unmount/remount until actual $outcome', async ({ action, outcome }) => {
      const work = hold(action);
      const view = render(<StrictMode><QwenSettings /></StrictMode>); fill(); await prepare(action);
      if (action === 'inspect') inspect(); else fireEvent.click(screen.getByRole('button', { name: buttonNames[action] }));
      const originalOwners = activity().owners.qwen;
      expect(originalOwners).toHaveLength(1);
      const dispatched = calls();
      if (action === 'apply' || action === 'update' || action === 'restore') {
        expect(screen.getByLabelText('settings.json 绝对路径')).toBeDisabled();
        expect(screen.getByLabelText('运行时 package.json 绝对路径')).toBeDisabled();
        expect(screen.getByRole('button', { name: '清除路径与结果' })).toBeDisabled();
      }
      view.unmount();
      expect(activity().owners.qwen).toEqual(originalOwners);
      expect(activity().busy.qwen).toBe(true);
      render(<StrictMode><QwenSettings /></StrictMode>); fill();
      expect(screen.getByLabelText('settings.json 绝对路径')).toHaveValue('/selected/settings.json');
      expectBlocked();
      inspect();
      for (const name of [buttonNames.preview, buttonNames['update-preview'], buttonNames.recovery, '撤销受管 Vim 设置']) fireEvent.click(screen.getByRole('button', { name }));
      expect(calls()).toEqual(dispatched);
      expectNoResults();

      // An old finally must release only its token, never another publisher's activity.
      const otherOwner = Symbol('another-qwen-owner');
      act(() => activity().setActivity('qwen', otherOwner, true));
      await act(async () => { work[outcome](); });
      expect(activity().owners.qwen).toEqual([otherOwner]);
      expect(activity().busy.qwen).toBe(true);
      expectBlocked();
      expectNoResults();
      act(() => activity().setActivity('qwen', otherOwner, false));
      expect(activity().busy.qwen).toBe(false);
      for (const name of [buttonNames.inspect, buttonNames.preview, buttonNames['update-preview'], buttonNames.recovery, '撤销受管 Vim 设置']) {
        expect(screen.getByRole('button', { name })).toBeEnabled();
      }
      inspect();
      await screen.findByRole('region', { name: 'Qwen 检查结果' });
      expect(activity().owners.qwen).toEqual([]);
    },
  );

  it.each(actions)('guards %s synchronously before a busy store update rerenders the button', async action => {
    render(<QwenSettings />); fill(); await prepare(action);
    const button = screen.getByRole('button', { name: buttonNames[action] });
    expect(button).toBeEnabled();
    const dispatched = calls();
    const otherOwner = Symbol('same-tick-qwen-owner');
    act(() => {
      activity().setActivity('qwen', otherOwner, true);
      // React has not committed the store update yet; a disabled prop alone is insufficient.
      expect(button).toBeEnabled();
      if (action === 'inspect') inspect(); else fireEvent.click(button);
    });
    expect(calls()).toEqual(dispatched);
    const isConfirmation = action === 'apply' || action === 'update' || action === 'restore';
    if (isConfirmation) expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    else expect(button).toBeDisabled();
    expect(activity().owners.qwen).toEqual([otherOwner]);
    act(() => activity().setActivity('qwen', otherOwner, false));
    if (isConfirmation) {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      expect(calls()).toEqual(dispatched);
      return;
    }
    expect(button).toBeEnabled();
    if (action === 'inspect') inspect(); else fireEvent.click(button);
    await act(async () => {});
    expect(calls().reduce((sum, count) => sum + count, 0)).toBe(dispatched.reduce((sum, count) => sum + count, 0) + 1);
    expect(activity().owners.qwen).toEqual([]);
  });

  it.each([
    { action: 'inspect', invalidate: 'clear' },
    { action: 'inspect', invalidate: 'path' },
    { action: 'preview', invalidate: 'clear' },
    { action: 'preview', invalidate: 'path' },
    { action: 'update-preview', invalidate: 'clear' },
    { action: 'update-preview', invalidate: 'path' },
    { action: 'recovery', invalidate: 'cancel' },
    { action: 'recovery', invalidate: 'clear' },
    { action: 'recovery', invalidate: 'path' },
  ] as const)('keeps $action owned after $invalidate until rejection', async ({ action, invalidate }) => {
    const work = hold(action);
    render(<QwenSettings />); fill();
    if (action === 'inspect') inspect(); else fireEvent.click(screen.getByRole('button', { name: buttonNames[action] }));
    const owners = activity().owners.qwen;
    expect(owners).toHaveLength(1);
    if (invalidate === 'clear') { fireEvent.click(screen.getByRole('button', { name: '清除路径与结果' })); fill(); }
    else if (invalidate === 'path') fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/other/settings.json' } });
    else fireEvent.click(screen.getByRole('button', { name: '取消恢复诊断结果' }));
    expect(activity().owners.qwen).toEqual(owners);
    expect(screen.getByRole('button', { name: buttonNames.preview })).toBeDisabled();
    const dispatched = calls(); inspect();
    expect(calls()).toEqual(dispatched);
    await act(async () => { work.reject(); });
    expectNoResults();
    expect(activity().owners.qwen).toEqual([]);
    expect(screen.getByRole('button', { name: buttonNames.preview })).toBeEnabled();
  });
});
