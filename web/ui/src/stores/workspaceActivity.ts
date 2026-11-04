import { useEffect, useRef } from 'react';
import { create } from 'zustand';

export type WorkspaceId = 'agents' | 'memory' | 'mcp' | 'vault' | 'updates';

type ActivityOwners = Record<WorkspaceId, readonly symbol[]>;

interface WorkspaceActivityState {
  owners: ActivityOwners;
  busy: Record<WorkspaceId, boolean>;
  setActivity: (id: WorkspaceId, owner: symbol, active: boolean) => void;
}

/** Each mounted publisher owns a token, so one finishing operation cannot clear another. */
export const useWorkspaceActivityStore = create<WorkspaceActivityState>((set) => ({
  owners: { agents: [], memory: [], mcp: [], vault: [], updates: [] },
  busy: { agents: false, memory: false, mcp: false, vault: false, updates: false },
  setActivity: (id, owner, active) => set((state) => {
    const current = state.owners[id];
    if (current.includes(owner) === active) return state;
    const next = active ? [...current, owner] : current.filter((entry) => entry !== owner);
    return {
      owners: { ...state.owners, [id]: next },
      busy: { ...state.busy, [id]: next.length > 0 },
    };
  }),
}));

/** Report activity independently of workspace visibility. Safe for StrictMode and cleanup. */
export function useWorkspaceActivity(id: WorkspaceId, active: boolean): void {
  const owner = useRef(Symbol('workspace-activity'));
  const setActivity = useWorkspaceActivityStore((state) => state.setActivity);

  useEffect(() => {
    const token = owner.current;
    setActivity(id, token, active);
    return () => setActivity(id, token, false);
  }, [id, active, setActivity]);
}
