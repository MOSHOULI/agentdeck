import manifest from '../../../docs/upstream/qoder/manifest-1.1.85.json';
const version = '1.1.85';
const manifestHash = 'a78234316fe2065cdae6fc17ac425bbc2ae11006d04b6ab5216fd6093597e4de';
const maxBytes = 256 * 1024 * 1024;
const files = manifest.files.map(file => ({ ...file, id: `${file.os}-${file.arch}${'variant' in file ? `-${file.variant}` : ''}` }));
export const qoderTargets = files.map(file => file.id);
export interface QoderPlan { target: string; version: '1.1.85'; archiveName: string; url: string; sha256: string; manifestHash: string; runtime: string; variant: string; requirements: string }
export interface QoderVerification { plan: QoderPlan; bytes: number }
const invalid = () => new Error('Qoder 响应不兼容，请重试。');
function object(value: unknown): Record<string, unknown> { if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalid(); return value as Record<string, unknown>; }
export function requireQoderTarget(target: string) { if (!qoderTargets.includes(target)) throw invalid(); }
function conditions(target: string): string {
  if (target === 'windows-arm64-standard') return 'Bun standard，Windows build ≥ 17763。官方文档仍称 ARM64 不支持，但清单列有原生包，存在矛盾；未验证实际兼容性。';
  if (target === 'windows-amd64-standard') return 'Bun standard，Windows build ≥ 17763；未检查本机版本或兼容性。';
  if (target === 'windows-amd64-legacy') return 'Node SEA legacy，官方脚本用于已知 build < 17763 的 x64 Windows；清单无最低版本，不保证全部旧系统可用。';
  if (target === 'linux-amd64') return '优化版要求 sse4_2、popcnt、avx、avx2、bmi1、bmi2、fma；未检查 CPU 或 libc 兼容性。';
  if (target === 'linux-amd64-baseline') return 'baseline 供优化版 CPU 条件不满足或未知时选择；未检查本机 CPU、libc 或运行兼容性。';
  if (target.endsWith('-musl')) return '独立 musl 变体；CPU、musl 版本和本机兼容性未检查，不可仅凭架构替换。';
  if (target === 'linux-arm64') return 'Linux ARM64；未检查 libc 或运行兼容性。';
  if (target === 'openharmony-arm64') return '仅分发清单列出 OpenHarmony ARM64，最低系统与运行兼容性未验收。';
  return 'macOS 明确架构变体；最低系统、代码签名、公证和运行兼容性未验证。';
}
export function validateQoderPlan(value: unknown, target: string): QoderPlan {
  const file = files.find(file => file.id === target); if (!file) throw invalid();
  const p = object(value);
  const archiveName = file.url.split('/').pop()!;
  const runtime = file.runtime ?? 'not_declared';
  const variant = file.variant ?? (target === 'linux-amd64' ? 'optimized' : target.endsWith('-baseline') ? 'baseline' : target.endsWith('-musl') ? 'musl' : 'not_declared');
  if (p.target !== target || p.version !== version || p.manifest_sha256 !== manifestHash || p.url !== file.url || p.sha256 !== file.sha256 || p.archive_name !== archiveName
    || p.runtime !== runtime || p.variant !== variant || p.min_windows_build !== (file.min_windows_build ?? null)
    || p.host_eligibility !== 'not_evaluated' || p.install_supported !== false || p.execution_verified !== false || p.signature_verified !== false || p.max_archive_bytes !== maxBytes) throw invalid();
  // Render local fixed labels, not arbitrary backend requirements/limitations/paths.
  return { target, version, archiveName, url: file.url, sha256: file.sha256, manifestHash, runtime, variant, requirements: conditions(target) };
}
export function validateQoderVerification(value: unknown, target: string): QoderVerification {
  const report = object(value);
  if (report.state !== 'matches_pinned_archive' || typeof report.bytes !== 'number' || !Number.isSafeInteger(report.bytes) || report.bytes <= 0 || report.bytes > maxBytes) throw invalid();
  return { plan: validateQoderPlan(report.plan, target), bytes: report.bytes };
}

export const qoderCacheStates = ['staged_archive_verified', 'unpacked_payload_verified', 'incomplete', 'conflict', 'unknown_ownership'] as const;
export type QoderCacheState = typeof qoderCacheStates[number];
export interface QoderCacheObservation { state: QoderCacheState; recoveryPlan: 'retain_cache_no_action' | 'preserve_and_review'; deletionAuthorized: false; manualReviewRequired: true; installed: false }
export function validateQoderCacheObservation(value: unknown): QoderCacheObservation {
  const report = object(value);
  if (!qoderCacheStates.includes(report.state as QoderCacheState)) throw invalid();
  const state = report.state as QoderCacheState;
  const recoveryPlan = state === 'staged_archive_verified' || state === 'unpacked_payload_verified' ? 'retain_cache_no_action' : 'preserve_and_review';
  if (report.recovery_plan !== recoveryPlan || report.deletion_authorized !== false || report.manual_review_required !== true || report.installed !== false) throw invalid();
  return { state, recoveryPlan, deletionAuthorized: false, manualReviewRequired: true, installed: false };
}
export const qoderCacheLabels: Record<QoderCacheState, string> = {
  staged_archive_verified: '暂存归档与固定契约一致；未安装、未运行。',
  unpacked_payload_verified: '解包缓存的归档与载荷符合固定契约；未安装、未运行。',
  incomplete: '缓存未完成；部分字节未获得完整性结论，请保留现场。',
  conflict: '缓存存在冲突或不安全状态；请保留现场，不覆盖或强制修复。',
  unknown_ownership: '无法确认缓存归属；请保留现场，不据此删除。',
};
