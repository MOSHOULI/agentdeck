export const residualSlots = ['package_directory', 'posix_launcher', 'cmd_launcher', 'powershell_launcher'] as const;
export type ResidualState = 'missing' | 'file' | 'directory' | 'blocked';
export type ResidualSlots = { slot: typeof residualSlots[number]; state: ResidualState }[];
export const validSessionRef = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(value);
export function parseResidual(value: unknown): ResidualSlots {
  const fail = () => { throw new Error('只读检查结果无效。'); };
  if (!value || typeof value !== 'object' || Array.isArray(value)) return fail();
  const raw = value as Record<string, unknown>;
  if (raw.schemaVersion !== 1 || raw.agentId !== 'codex' || raw.runtimeNotRun !== true || raw.ownership !== 'unknown' || raw.cleanupSupported !== false || !Array.isArray(raw.slots) || raw.slots.length !== 4) return fail();
  return residualSlots.map((slot, index) => {
    const entry = (raw.slots as unknown[])[index];
    if (!entry || typeof entry !== 'object' || Array.isArray(entry)) return fail();
    const row = entry as Record<string, unknown>;
    if (row.slot !== slot || typeof row.state !== 'string' || !['missing', 'file', 'directory', 'blocked'].includes(row.state)) return fail();
    return { slot, state: row.state as ResidualState };
  });
}
