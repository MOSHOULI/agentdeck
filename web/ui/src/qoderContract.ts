import manifest from '../../../docs/upstream/qoder/manifest-1.1.85.json';
import manifest86 from '../../../docs/upstream/qoder/manifest-1.1.86.json';
import { requireQoderRelease, type QoderRelease } from './qoderReleases';
const manifestHashes = { '1.1.85': 'a78234316fe2065cdae6fc17ac425bbc2ae11006d04b6ab5216fd6093597e4de', '1.1.86': 'f323905da3f75088b1e9ae4cf9b0fef6f117879f4f6942b609e92228a533fa78' };
const maxBytes = 256 * 1024 * 1024;
const filesFor = (version: QoderRelease) => (version === '1.1.85' ? manifest : manifest86).files.map(file => ({ ...file, id: `${file.os}-${file.arch}${'variant' in file ? `-${file.variant}` : ''}` }));
export const qoderTargets = filesFor('1.1.85').map(file => file.id);
export interface QoderPlan { target: string; version: QoderRelease; archiveName: string; url: string; sha256: string; manifestHash: string; runtime: string; variant: string; requirements: string }
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
export function validateQoderPlan(value: unknown, target: string, version: QoderRelease = '1.1.85'): QoderPlan {
  requireQoderRelease(version);
  const manifestHash = manifestHashes[version];
  const file = filesFor(version).find(file => file.id === target); if (!file) throw invalid();
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
export function validateQoderVerification(value: unknown, target: string, version: QoderRelease = '1.1.85'): QoderVerification {
  const report = object(value);
  if (report.state !== 'matches_pinned_archive' || typeof report.bytes !== 'number' || !Number.isSafeInteger(report.bytes) || report.bytes <= 0 || report.bytes > maxBytes) throw invalid();
  return { plan: validateQoderPlan(report.plan, target, version), bytes: report.bytes };
}

export const qoderCacheStates = ['staged_archive_verified', 'unpacked_payload_verified', 'incomplete', 'conflict', 'unknown_ownership'] as const;
export type QoderCacheState = typeof qoderCacheStates[number];
export interface QoderCacheObservation { state: QoderCacheState; recoveryPlan: 'retain_cache_no_action' | 'preserve_and_review'; deletionAuthorized: false; manualReviewRequired: true; installed: false }
export function validateQoderCacheObservation(value: unknown, version: QoderRelease = '1.1.85'): QoderCacheObservation {
  const report = object(value);
  requireQoderRelease(version);
  if (report.version !== version) throw invalid();
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

export type QoderWriteOperation = 'stage' | 'unpack';
export interface QoderWriteResult { operation: QoderWriteOperation; state: 'staged_archive_verified' | 'unpacked_payload_verified' }
export function validateQoderWrite(value: unknown, operation: QoderWriteOperation, version: QoderRelease = '1.1.85'): QoderWriteResult {
  requireQoderRelease(version);
  const report = object(value);
  const state = operation === 'stage' ? 'staged_archive_verified' : 'unpacked_payload_verified';
  if (report.operation !== operation || report.state !== state || report.target !== 'linux-amd64-baseline' || report.version !== version || report.installed !== false || report.executable !== false) throw invalid();
  return { operation, state };
}

/** Guidance is local fixed text, never backend instructions or cleanup authority. */
export const qoderCacheRecoveryGuidance: Record<QoderCacheState, string> = {
  staged_archive_verified: '可将这个暂存目录作为后续解包来源，并选择另一个全新目标目录；每次解包仍须重新确认和由核心重新校验，不复用本次诊断作为授权。',
  unpacked_payload_verified: '仅保留此解包缓存，不激活或执行；它不能作为暂存归档或受控暂存目录来源再次解包。',
  incomplete: '保留未完成现场，人工排查；不要复用此目录作为来源或目标，不原地重试，不自动删除。',
  conflict: '保留冲突现场，人工排查；不要覆盖、复用或强制修复，不自动删除。',
  unknown_ownership: '归属未知，保留现场并人工排查；不复用、不据此清理，也不能推断目录为空或安全。',
};
