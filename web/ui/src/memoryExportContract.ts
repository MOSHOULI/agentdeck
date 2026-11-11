/** Fail closed on malformed, oversized or mismatched export responses. */
export type ExportTarget = 'codex' | 'claude-code' | 'opencode';
export interface ExportRow { id: string; agentId: string; content: string; contentHash: string; collectedAt: number; autoInject: boolean; tags: string[] }
export interface ExportSnapshot { agentId: ExportTarget; databaseExists: boolean; stateToken: string; manualEntries: ExportRow[]; automaticEntries: ExportRow[] }
export interface ExportPreview { agentId: ExportTarget; stateToken: string; previewToken: string; manualEntries: ExportRow[]; automaticEntries: ExportRow[]; outputEntries: ExportRow[] }
export interface ExportReceipt { agentId: ExportTarget; manualIds: string[]; exportedCount: number }
const fail = (): never => { throw new Error('导出响应无效，请重新读取并预览。'); };
function object(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return fail();
  const o = value as Record<string, unknown>;
  if (Object.keys(o).length !== keys.length || keys.some(k => !(k in o))) return fail();
  return o;
}
export function exportTarget(value: string): asserts value is ExportTarget { if (!['codex', 'claude-code', 'opencode'].includes(value)) fail(); }
export function exportToken(value: unknown): asserts value is string { if (typeof value !== 'string' || !/^[a-f0-9]{64}$/.test(value)) fail(); }
export function exportIds(value: unknown): asserts value is string[] {
  if (!Array.isArray(value) || value.length > 200 || new Set(value).size !== value.length || value.some(id => typeof id !== 'string' || !/^[1-9][0-9]{0,18}$/.test(id) || BigInt(id) > 9223372036854775807n)) fail();
}
const sameIds = (a: string[], b: string[]) => a.length === b.length && a.every(id => b.includes(id));
function rows(value: unknown, limit: number): ExportRow[] {
  if (!Array.isArray(value) || value.length > limit) return fail();
  for (const item of value) {
    const r = object(item, ['id','agentId','content','contentHash','collectedAt','autoInject','tags']);
    exportIds([r.id]); exportToken(r.contentHash);
    if (typeof r.agentId !== 'string' || !/^[a-z0-9-]{1,64}$/.test(r.agentId) || typeof r.content !== 'string' || new TextEncoder().encode(r.content).length > 65536 || !Number.isSafeInteger(r.collectedAt) || typeof r.autoInject !== 'boolean' || !Array.isArray(r.tags) || r.tags.some(t => typeof t !== 'string')) fail();
  }
  if (new Set(value.map(r => r.id)).size !== value.length) fail();
  return value as ExportRow[];
}
function totalRows(groups: ExportRow[][]) {
  const unique = new Map<string, ExportRow>();
  for (const row of groups.flat()) {
    const previous = unique.get(row.id);
    if (previous && JSON.stringify(previous) !== JSON.stringify(row)) fail();
    unique.set(row.id, row);
  }
  if (unique.size > 500) fail();
  if ([...unique.values()].reduce((n, r) => n + new TextEncoder().encode(JSON.stringify(r)).length, 0) > 2097152) fail();
}
export function validateExportSnapshot(value: unknown, target: ExportTarget): ExportSnapshot {
  const o = object(value, ['agentId','databaseExists','stateToken','manualEntries','automaticEntries']);
  if (o.agentId !== target || typeof o.databaseExists !== 'boolean') fail();
  exportToken(o.stateToken);
  const manual = rows(o.manualEntries, 200), automatic = rows(o.automaticEntries, 500);
  if (automatic.some(r => r.agentId !== target || !r.autoInject)) fail();
  if (!o.databaseExists && (manual.length || automatic.length)) fail();
  totalRows([manual, automatic]);
  return o as unknown as ExportSnapshot;
}
export function validateExportPreview(value: unknown, target: ExportTarget, token: string, ids: string[]): ExportPreview {
  const o = object(value, ['agentId','stateToken','previewToken','manualEntries','automaticEntries','outputEntries']);
  if (o.agentId !== target || o.stateToken !== token) fail();
  exportToken(o.previewToken);
  const manual = rows(o.manualEntries, 200), automatic = rows(o.automaticEntries, 500), output = rows(o.outputEntries, 500);
  if (!sameIds(manual.map(r => r.id), ids) || automatic.some(r => r.agentId !== target || !r.autoInject) || !sameIds(output.map(r => r.id), [...new Set([...ids, ...automatic.map(r => r.id)])])) fail();
  totalRows([manual, automatic, output]);
  return o as unknown as ExportPreview;
}
export function validateExportReceipt(value: unknown, target: ExportTarget, ids: string[]): ExportReceipt {
  const o = object(value, ['agentId','manualIds','exportedCount']); exportIds(o.manualIds);
  if (o.agentId !== target || !sameIds(o.manualIds, ids) || !Number.isInteger(o.exportedCount) || (o.exportedCount as number) < ids.length || (o.exportedCount as number) > 500) fail();
  return o as unknown as ExportReceipt;
}
