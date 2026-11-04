import { StrictMode } from 'react';
import { afterEach, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { useWorkspaceActivity, useWorkspaceActivityStore } from './workspaceActivity';
import type { WorkspaceId } from './workspaceActivity';

function Publisher({ id = 'memory', active }: { id?: WorkspaceId; active: boolean }) {
  useWorkspaceActivity(id, active);
  return null;
}

afterEach(() => {
  cleanup();
  expect(Object.values(useWorkspaceActivityStore.getState().busy).every((busy) => !busy)).toBe(true);
});

it('publishes activity and removes only its own owner on completion', () => {
  const first = render(<Publisher active />);
  const second = render(<Publisher active />);
  expect(useWorkspaceActivityStore.getState().owners.memory).toHaveLength(2);
  first.rerender(<Publisher active={false} />);
  expect(useWorkspaceActivityStore.getState().busy.memory).toBe(true);
  second.unmount();
  expect(useWorkspaceActivityStore.getState().busy.memory).toBe(false);
});

it('does not leak owners across StrictMode effect setup and cleanup', () => {
  const view = render(<StrictMode><Publisher active /></StrictMode>);
  expect(useWorkspaceActivityStore.getState().owners.memory).toHaveLength(1);
  view.rerender(<StrictMode><Publisher active={false} /></StrictMode>);
  expect(useWorkspaceActivityStore.getState().owners.memory).toHaveLength(0);
  view.rerender(<StrictMode><Publisher active /></StrictMode>);
  expect(useWorkspaceActivityStore.getState().owners.memory).toHaveLength(1);
  view.unmount();
  expect(useWorkspaceActivityStore.getState().busy.memory).toBe(false);
});

it('moves activity between workspaces without leaving stale status', () => {
  const view = render(<Publisher active />);
  view.rerender(<Publisher id="mcp" active />);
  expect(useWorkspaceActivityStore.getState().busy.memory).toBe(false);
  expect(useWorkspaceActivityStore.getState().busy.mcp).toBe(true);
});

it('does not publish redundant updates for an already registered owner', () => {
  const owner = Symbol('test-owner');
  const store = useWorkspaceActivityStore;
  store.getState().setActivity('vault', owner, true);
  const registered = store.getState();
  store.getState().setActivity('vault', owner, true);
  expect(store.getState()).toBe(registered);
  store.getState().setActivity('vault', owner, false);
  const cleared = store.getState();
  store.getState().setActivity('vault', owner, false);
  expect(store.getState()).toBe(cleared);
});
