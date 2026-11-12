import { requireQoderRelease, qoderPayloads, type QoderRelease } from './qoderReleases';
export type DeploymentInspectionState = 'payload_verified' | 'missing' | 'incomplete' | 'unsupported_or_changed';
export interface DeploymentInspection { state: DeploymentInspectionState }
export function validateDeploymentInspection(value: unknown, release: QoderRelease): DeploymentInspection {
  requireQoderRelease(release);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('QODER_INSPECTION_INVALID_RESPONSE');
  const p = value as Record<string, unknown>;
  const keys = ['version', 'target', 'state', 'payload_bytes', 'payload_sha256', 'execution_verified', 'activated', 'ownership_proven', 'deletion_authorized', 'repair_authorized', 'manual_review_required', 'limitations'];
  if (Object.keys(p).length !== keys.length || Object.keys(p).some(k => !keys.includes(k)) ||
      typeof p.limitations !== 'string' || !p.limitations.trim() || p.limitations.length > 4096 ||
      p.version !== release || p.target !== 'linux-amd64-baseline' || p.payload_bytes !== qoderPayloads[release].bytes || p.payload_sha256 !== qoderPayloads[release].sha256 ||
      !['payload_verified', 'missing', 'incomplete', 'unsupported_or_changed'].includes(p.state as string) ||
      ['execution_verified', 'activated', 'ownership_proven', 'deletion_authorized', 'repair_authorized'].some(k => p[k] !== false) || p.manual_review_required !== true)
    throw new Error('QODER_INSPECTION_INVALID_RESPONSE');
  return { state: p.state as DeploymentInspectionState };
}
