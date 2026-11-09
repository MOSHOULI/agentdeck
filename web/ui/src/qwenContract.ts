/** Project untrusted IPC into fixed labels only; never retain arbitrary values/keys/errors. */
export const sectionNames = ['general', 'ui', 'model', 'model_providers', 'context', 'tools', 'mcp_servers', 'security', 'experimental'] as const;
export type QwenSection = typeof sectionNames[number];
const statuses = ['not_selected', 'not_read', 'read', 'missing', 'unsupported', 'unreadable'] as const;
export type QwenStatus = typeof statuses[number];
export interface QwenSummary {
  manifest: QwenStatus;
  settings: QwenStatus;
  reviewed: boolean;
  sections: Record<QwenSection, boolean> | null;
}
const invalid = () => new Error('Qwen 响应不兼容，请重试。');
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalid();
  return value as Record<string, unknown>;
}
function status(value: unknown): QwenStatus {
  if (!statuses.includes(value as QwenStatus)) throw invalid();
  return value as QwenStatus;
}
export function validateQwenReport(value: unknown): QwenSummary {
  const report = object(value);
  if (report.mode !== 'explicit_settings_summary' || report.authentication !== 'unknown' || report.effective_configuration !== 'unknown') throw invalid();
  const capabilities = object(report.mutation_capabilities);
  if (Object.values(capabilities).some(value => value !== false)) throw invalid();
  for (const key of ['configure', 'authenticate', 'install', 'restore', 'execute']) {
    if (capabilities[key] !== false) throw invalid();
  }
  if (!['unknown', 'reviewed_manifest_version', 'unrecognized_manifest_version'].includes(report.version_recognition as string)) throw invalid();
  const manifest = status(object(report.runtime_manifest).status);
  const settingsObject = object(report.settings);
  const settings = status(settingsObject.status);
  const reviewed = report.version_recognition === 'reviewed_manifest_version';
  if (reviewed !== (manifest === 'read')) throw invalid();
  let sections: QwenSummary['sections'] = null;
  if (settings === 'read') {
    if (!reviewed) throw invalid();
    const source = object(settingsObject.sections);
    sections = {} as Record<QwenSection, boolean>;
    for (const key of sectionNames) {
      if (typeof source[key] !== 'boolean') throw invalid();
      sections[key] = source[key];
    }
  } else if (settingsObject.sections != null) throw invalid();
  return { manifest, settings, reviewed, sections };
}
export function qwenDemoSummary(): QwenSummary {
  return { manifest: 'read', settings: 'read', reviewed: true, sections: Object.fromEntries(sectionNames.map(key => [key, key === 'general' || key === 'model'])) as Record<QwenSection, boolean> };
}

export interface QwenVimPreview {
  field: 'general.vimMode'; before: boolean | null; after: boolean; changes: boolean; token: string;
}
/** Only fixed labels and the one non-sensitive boolean cross into rendered state. */
export function validateQwenVimPreview(value: unknown, enabled: boolean): QwenVimPreview {
  const preview = object(value);
  if (preview.field !== 'general.vimMode' || !(preview.before === null || typeof preview.before === 'boolean')
    || preview.after !== enabled || typeof preview.changes !== 'boolean'
    || preview.changes !== (preview.before !== enabled)
    || typeof preview.token !== 'string' || !/^[a-f0-9]{64}$/.test(preview.token)) throw invalid();
  return { field: 'general.vimMode', before: preview.before, after: enabled, changes: preview.changes, token: preview.token };
}
export function validateQwenMutation(value: unknown): boolean {
  if (typeof value !== 'boolean') throw invalid();
  return value;
}

export const recoveryStates = ['no_receipt', 'restore_field', 'cleanup_receipt', 'conflict'] as const;
export type QwenRecoveryState = typeof recoveryStates[number];
export interface QwenRecovery { field: 'general.vimMode'; state: QwenRecoveryState; changesSettings: boolean }
/** Do not retain backend limitations, errors, paths, hashes or arbitrary properties. */
export function validateQwenRecovery(value: unknown): QwenRecovery {
  const report = object(value);
  if (report.field !== 'general.vimMode' || !recoveryStates.includes(report.state as QwenRecoveryState)
    || typeof report.changes_settings !== 'boolean' || report.changes_settings !== (report.state === 'restore_field')) throw invalid();
  return { field: 'general.vimMode', state: report.state as QwenRecoveryState, changesSettings: report.changes_settings };
}
