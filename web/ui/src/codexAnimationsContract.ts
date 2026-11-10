export type AnimationsPreview = { field: 'tui.animations'; before: boolean; after: boolean; changes: boolean; token: string };
export type AnimationsRecovery = { state: 'no_receipt' | 'restore_original' | 'cleanup_receipt'; token: string | null };
const fail = (): never => { throw new Error('Codex 动画操作返回无效，请重新检查。'); };
export const validAnimationsToken = (v: unknown): v is string => typeof v === 'string' && /^[a-f0-9]{64}$/.test(v);
export function validateAnimationsPreview(value: unknown, after: boolean): AnimationsPreview {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return fail();
  const r = value as Record<string, unknown>;
  if (r.field !== 'tui.animations' || typeof r.before !== 'boolean' || r.after !== after || r.changes !== (r.before !== after) || !validAnimationsToken(r.token)) return fail();
  return { field: 'tui.animations', before: r.before, after, changes: r.before !== after, token: r.token };
}
export function validateAnimationsRecovery(value: unknown): AnimationsRecovery {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return fail();
  const r = value as Record<string, unknown>;
  if (r.state !== 'no_receipt' && r.state !== 'restore_original' && r.state !== 'cleanup_receipt') return fail();
  if (r.state === 'no_receipt' ? r.token !== null : !validAnimationsToken(r.token)) return fail();
  return { state: r.state, token: r.token as string | null };
}
export function validateAnimationsMutation(value: unknown): boolean { if (typeof value !== 'boolean') return fail(); return value; }
export type AnimationsCapability = 'linux_managed' | 'windows_read_only' | 'unsupported';
export function validateAnimationsCapability(value: unknown): AnimationsCapability {
  if (value !== 'linux_managed' && value !== 'windows_read_only' && value !== 'unsupported') return fail();
  return value;
}
export type AnimationsObservation = { schemaVersion: 1; readOnly: true; state: 'ready' | 'missing' | 'unsupported' | 'receipt_present'; current: boolean | null; applySupported: false; restoreSupported: false };
export function validateAnimationsObservation(value: unknown): AnimationsObservation {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return fail();
  const r = value as Record<string, unknown>;
  if (r.schemaVersion !== 1 || r.readOnly !== true || !['ready','missing','unsupported','receipt_present'].includes(r.state as string) || r.applySupported !== false || r.restoreSupported !== false || (r.state === 'ready' ? typeof r.current !== 'boolean' : r.current !== null)) return fail();
  return { schemaVersion: 1, readOnly: true, state: r.state as AnimationsObservation['state'], current: r.current as boolean | null, applySupported: false, restoreSupported: false };
}
