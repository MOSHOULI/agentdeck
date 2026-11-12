export interface CodexInstallRecovery {
  schemaVersion: 1;
  agentId: 'codex';
  state: 'clear' | 'blocked' | 'eligible';
  prefix: string | null;
  slots: { slot: string; state: 'missing' | 'present' | 'blocked' }[];
  revision: string | null;
  runtimeNotRun: true;
  cleanupSupported: false;
  limitations: string;
}
const names = ['package_directory', 'posix_launcher', 'cmd_launcher', 'powershell_launcher', 'exe_launcher', 'com_launcher', 'bat_launcher'];
const text = (v: unknown, max: number): v is string => typeof v === 'string' && v.length > 0 && v.length <= max
  && v.trim() === v && !/[\x00-\x1f\x7f\u200b-\u200f\u2028-\u202e\u2060-\u206f\ufeff]/.test(v);
export function recoveryRevision(v: unknown): v is string { return typeof v === 'string' && /^[0-9a-f]{64}$/.test(v); }
export function parseCodexInstallRecovery(value: unknown): CodexInstallRecovery {
  const fail = (): never => { throw new Error('恢复检查结果无效。'); };
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return fail();
  const r = value as Record<string, unknown>;
  if (r.schemaVersion !== 1 || r.agentId !== 'codex' || (typeof r.state !== 'string' || !['clear','blocked','eligible'].includes(r.state))
    || r.runtimeNotRun !== true || r.cleanupSupported !== false || !text(r.limitations, 8192)
    || (r.prefix !== null && (!text(r.prefix, 32768) || !/^(?:\/|[a-zA-Z]:[\\/])/.test(r.prefix))) || !Array.isArray(r.slots)) return fail();
  const slots = r.slots.map(s => {
    if (s === null || typeof s !== 'object' || Array.isArray(s) || !names.includes(s.slot)
      || !['missing','present','blocked'].includes(s.state)) return fail();
    return { slot: s.slot as string, state: s.state as 'missing' | 'present' | 'blocked' };
  });
  if (new Set(slots.map(s => s.slot)).size !== slots.length || ![0,2,7].includes(slots.length)) return fail();
  if (slots.length && !slots.every((s,i) => s.slot === names[i])) return fail();
  if (r.state === 'eligible') {
    if (!recoveryRevision(r.revision) || !text(r.prefix,32768) || ![2,7].includes(slots.length) || slots.some(s => s.state !== 'missing')) return fail();
  } else if (r.revision !== null) return fail();
  if (r.state === 'clear' && (r.prefix !== null || slots.length)) return fail();
  return { schemaVersion:1, agentId:'codex', state:r.state as CodexInstallRecovery['state'], prefix:r.prefix as string|null,
    slots, revision:r.revision as string|null, runtimeNotRun:true, cleanupSupported:false, limitations:r.limitations };
}
