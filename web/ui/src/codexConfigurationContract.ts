export const codexConflictCodes = ['managed_block_replacement', 'ambiguous_markers', 'invalid_target_toml', 'toml_type_conflict', 'differing_toml_value'] as const;
export type CodexConflict = typeof codexConflictCodes[number];
export type CodexTarget = { kind: 'instructions' | 'settings'; path: string; state: 'absent' | 'present'; action: 'create' | 'unchanged' | 'merge' | 'replace_managed_block' | 'conflict'; conflictCodes: CodexConflict[]; conflictCount: number };
export type CodexConfigurationPlan = { schemaVersion: 1; agentId: 'codex'; mode: 'read_only_configure_plan'; targets: CodexTarget[]; conflictCount: number; revision: string; applySupported: false; usesBundledDefaults: boolean };
const invalid = () => new Error('Codex 配置计划响应不兼容。');
const object = (value: unknown): Record<string, unknown> => { if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalid(); return value as Record<string, unknown>; };
export function validateCodexConfigurationPlan(value: unknown): CodexConfigurationPlan {
  const plan = object(value);
  if (plan.schemaVersion !== 1 || plan.agentId !== 'codex' || plan.mode !== 'read_only_configure_plan' || plan.applySupported !== false || typeof plan.usesBundledDefaults !== 'boolean' || typeof plan.revision !== 'string' || !/^[a-f0-9]{64}$/.test(plan.revision) || !Array.isArray(plan.targets) || plan.targets.length !== 2) throw invalid();
  const targets = plan.targets.map((raw, index): CodexTarget => {
    const t = object(raw); const kind = index === 0 ? 'instructions' : 'settings';
    if (t.kind !== kind || typeof t.path !== 'string' || !t.path.trim() || new TextEncoder().encode(t.path).length > 4096 || /[\x00-\x1f\x7f]/.test(t.path) || !['absent', 'present'].includes(t.state as string) || !['create', 'unchanged', 'merge', 'replace_managed_block', 'conflict'].includes(t.action as string) || !Array.isArray(t.conflictCodes) || t.conflictCodes.some(c => !codexConflictCodes.includes(c as CodexConflict)) || new Set(t.conflictCodes).size !== t.conflictCodes.length || !Number.isSafeInteger(t.conflictCount) || (t.conflictCount as number) < t.conflictCodes.length || (t.conflictCount === 0) !== (t.conflictCodes.length === 0)) throw invalid();
    if ((t.state === 'absent' && !['create', 'unchanged'].includes(t.action as string)) || (t.state === 'present' && t.action === 'create') || (kind === 'settings' && t.action === 'replace_managed_block') || (t.action === 'conflict' && t.conflictCount === 0) || (t.action === 'replace_managed_block' && !t.conflictCodes.includes('managed_block_replacement'))) throw invalid();
    if ((['create', 'unchanged', 'merge'].includes(t.action as string) && t.conflictCount !== 0) || t.conflictCodes.some(code => kind === 'instructions' ? !['managed_block_replacement', 'ambiguous_markers'].includes(code as string) : !['invalid_target_toml', 'toml_type_conflict', 'differing_toml_value'].includes(code as string))) throw invalid();
    return { kind, path: t.path, state: t.state as CodexTarget['state'], action: t.action as CodexTarget['action'], conflictCodes: [...t.conflictCodes] as CodexConflict[], conflictCount: t.conflictCount as number };
  });
  if (targets[0].path === targets[1].path || plan.conflictCount !== targets.reduce((n,t) => n+t.conflictCount,0)) throw invalid();
  return { schemaVersion: 1, agentId: 'codex', mode: 'read_only_configure_plan', targets, conflictCount: plan.conflictCount as number, revision: plan.revision, applySupported: false, usesBundledDefaults: plan.usesBundledDefaults };
}
export function demoCodexConfigurationPlan(): CodexConfigurationPlan {
  return { schemaVersion: 1, agentId: 'codex', mode: 'read_only_configure_plan', targets: [ { kind: 'instructions', path: '/synthetic/.codex/AGENTS.md', state: 'absent', action: 'create', conflictCodes: [], conflictCount: 0 }, { kind: 'settings', path: '/synthetic/.codex/config.toml', state: 'present', action: 'conflict', conflictCodes: ['differing_toml_value'], conflictCount: 1 } ], conflictCount: 1, revision: '0'.repeat(64), applySupported: false, usesBundledDefaults: true };
}
