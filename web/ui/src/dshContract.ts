/** Passive DSH IPC schema. Do not translate declarations into runtime facts. */
export const dshLimits = {
  max_files: 2, max_manifest_bytes: 65536, max_path_bytes: 4096,
  max_path_components: 64, max_json_depth: 16, max_json_entries: 4096,
  max_json_string_bytes: 8192, max_dependencies: 128, max_saved_bundles: 64,
  max_name_bytes: 214, max_version_bytes: 64, max_report_bytes: 131072,
} as const;

export const dshIssues = [
  'path_must_be_absolute', 'parent_traversal', 'unsupported_path', 'other_selection_invalid',
  'path_limit', 'expected_package_manifest', 'missing', 'permission_denied', 'io_failure',
  'link_or_reparse_point', 'not_directory', 'not_regular_file', 'multiple_hard_links',
  'file_changed', 'file_too_large', 'invalid_json', 'duplicate_json_key', 'json_limit',
  'unsupported_shape', 'unsupported_manifest_version', 'unsupported_metadata',
  'dependency_limit', 'bundle_limit', 'platform_unsupported',
] as const;
export type DshIssue = typeof dshIssues[number];
export type DshManifestStatus = 'read' | 'missing' | 'corrupt' | 'unreadable' | 'unsupported';
export interface DshDependency {
  name: string;
  section: 'dependencies' | 'optional_dependencies' | 'peer_dependencies';
  specification_kind: 'registry_version_or_range' | 'other_redacted';
  installed: 'unknown';
  actual_enabled: 'unknown';
}
interface DshManifest<T> {
  status: DshManifestStatus;
  issue: DshIssue | null;
  metadata: T | null;
}
export interface DshInventoryReport {
  mode: 'passive_manifest_only';
  profile_manifest: DshManifest<{
    name: string | null;
    declared_dependencies: DshDependency[];
    saved_bundle_selections: string[];
  }>;
  runtime_manifest: DshManifest<{
    name: string | null;
    reported_version: string | null;
    declared_node_engine: string | null;
  }> | null;
  version_recognition: 'reviewed_manifest_version' | 'unrecognized_manifest_version' | 'unknown';
  runtime_status: 'unknown';
  authentication: 'unknown';
  actual_enabled: 'unknown';
  mutation_capabilities: {
    configure_profile: false; manage_plugins: false; authenticate: false;
    launch_runtime: false; upgrade_runtime: false; uninstall_runtime: false;
  };
  limits: { [K in keyof typeof dshLimits]: number };
  limitations: string[];
}
export interface DshInspectionResult { report: DshInventoryReport; synthetic: boolean }

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function boundedText(value: unknown, max: number): value is string {
  return typeof value === 'string' && value.length <= max && /^[\x20-\x7e]*$/.test(value);
}
function packageName(value: unknown): value is string {
  return boundedText(value, 214) && /^(?:@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*$/.test(value);
}
function nullable(value: unknown, check: (v: unknown) => boolean): boolean {
  return value === null || check(value);
}
function manifest(value: unknown, metadata: (v: unknown) => boolean): boolean {
  if (!record(value)) return false;
  if (value.status === 'read') return value.issue === null && metadata(value.metadata);
  if (!dshIssues.includes(value.issue as DshIssue) || value.metadata !== null) return false;
  const expected = value.issue === 'missing' ? 'missing'
    : ['invalid_json', 'duplicate_json_key'].includes(String(value.issue)) ? 'corrupt'
    : ['permission_denied', 'io_failure', 'file_changed'].includes(String(value.issue)) ? 'unreadable' : 'unsupported';
  return value.status === expected;
}
function profileMetadata(value: unknown): boolean {
  return record(value) && nullable(value.name, packageName)
    && Array.isArray(value.declared_dependencies) && value.declared_dependencies.length <= 128
    && value.declared_dependencies.every((row: unknown) => record(row) && packageName(row.name)
      && ['dependencies', 'optional_dependencies', 'peer_dependencies'].includes(String(row.section))
      && ['registry_version_or_range', 'other_redacted'].includes(String(row.specification_kind))
      && row.installed === 'unknown' && row.actual_enabled === 'unknown')
    && Array.isArray(value.saved_bundle_selections) && value.saved_bundle_selections.length <= 64
    && value.saved_bundle_selections.every(packageName);
}
function runtimeMetadata(value: unknown): boolean {
  return record(value) && nullable(value.name, packageName)
    && nullable(value.reported_version, (v) => boundedText(v, 64)
      && /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/.test(v))
    && nullable(value.declared_node_engine, (v) => boundedText(v, 64)
      && /^(\*|[~^<>=]*[0-9]+(\.[0-9xX*]+){0,2}(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?)( (\|\| )?(\*|[~^<>=]*[0-9]+(\.[0-9xX*]+){0,2}(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?))*$/.test(v));
}

/** Reject an incompatible/unsafe response instead of showing partial success. */
export function validateDshReport(value: unknown): DshInventoryReport {
  const capabilities = ['configure_profile', 'manage_plugins', 'authenticate', 'launch_runtime', 'upgrade_runtime', 'uninstall_runtime'];
  if (!record(value) || value.mode !== 'passive_manifest_only'
    || !['runtime_status', 'authentication', 'actual_enabled'].every((key) => value[key] === 'unknown')
    || !['reviewed_manifest_version', 'unrecognized_manifest_version', 'unknown'].includes(String(value.version_recognition))
    || !record(value.mutation_capabilities) || capabilities.some((key) => (value.mutation_capabilities as Record<string, unknown>)[key] !== false)
    || Object.values(value.mutation_capabilities).some((enabled) => enabled !== false)
    || !record(value.limits) || Object.entries(dshLimits).some(([key, limit]) => (value.limits as Record<string, unknown>)[key] !== limit)
    || !manifest(value.profile_manifest, profileMetadata)
    || !(value.runtime_manifest === null || manifest(value.runtime_manifest, runtimeMetadata))
    || !Array.isArray(value.limitations) || value.limitations.length > 16
    || !value.limitations.every((line: unknown) => boundedText(line, 2048))
    || JSON.stringify(value).length > dshLimits.max_report_bytes) {
    throw new Error('DSH 盘点响应格式不兼容，未显示未经验证的数据。');
  }
  const report = value as unknown as DshInventoryReport;
  const runtime = report.runtime_manifest?.metadata;
  const recognition = runtime?.reported_version
    ? (['@deepseek-ai/dsh', '@deepseek-ai/dsh-app-boot'].includes(runtime.name ?? '') && runtime.reported_version === '0.2.2-alpha.3'
      ? 'reviewed_manifest_version' : 'unrecognized_manifest_version')
    : 'unknown';
  if (report.version_recognition !== recognition) {
    throw new Error('DSH 盘点响应格式不兼容，未显示未经验证的数据。');
  }
  return report;
}

/** Synthetic declarations only; no input path is read or echoed. */
export function dshDemoReport(withRuntime: boolean): DshInventoryReport {
  return {
    mode: 'passive_manifest_only',
    profile_manifest: { status: 'read', issue: null, metadata: {
      name: 'synthetic-demo-profile',
      declared_dependencies: [{ name: 'synthetic-demo-bundle', section: 'dependencies', specification_kind: 'registry_version_or_range', installed: 'unknown', actual_enabled: 'unknown' }],
      saved_bundle_selections: ['synthetic-demo-bundle'],
    } },
    runtime_manifest: withRuntime ? { status: 'read', issue: null, metadata: {
      name: '@deepseek-ai/dsh', reported_version: '999.0.0-demo', declared_node_engine: '>=24.9.0',
    } } : null,
    version_recognition: withRuntime ? 'unrecognized_manifest_version' : 'unknown',
    runtime_status: 'unknown', authentication: 'unknown', actual_enabled: 'unknown',
    mutation_capabilities: { configure_profile: false, manage_plugins: false, authenticate: false, launch_runtime: false, upgrade_runtime: false, uninstall_runtime: false },
    limits: { ...dshLimits },
    limitations: ['Synthetic demo only. No local files were inspected.'],
  };
}
