import { requireQoderRelease, qoderPayloads, type QoderRelease } from './qoderReleases';
/** Fixed private deployment contract. Backend prose is never rendered as authority. */
export const qoderDeploymentVersion = '1.1.85';
export const qoderDeploymentTarget = 'linux-amd64-baseline';
export const qoderDeploymentBytes = 182543840;
export const qoderDeploymentHash = '8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b';
export interface QoderDeploymentPreview { token: string }
export interface QoderDeploymentOutcome { state: 'private_executable_deployed_not_activated' }
export const validQoderDeploymentToken = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
export const validQoderDeploymentPath = (value: unknown): value is string => typeof value === 'string' && value.startsWith('/') && new TextEncoder().encode(value).length <= 4096 && !/[\x00-\x1f\x7f]/.test(value);
function report(value: unknown, version: QoderRelease): Record<string, unknown> {
  requireQoderRelease(version);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Qoder 部署响应不兼容。');
  const p = value as Record<string, unknown>;
  if (p.version !== version || p.target !== qoderDeploymentTarget || p.executable_mode !== '0700') throw new Error('Qoder 部署响应不兼容。');
  return p;
}
export function validateQoderDeploymentPreview(value: unknown, version: QoderRelease = '1.1.85'): QoderDeploymentPreview {
  const p = report(value, version);
  if (p.payload_bytes !== qoderPayloads[version].bytes || p.payload_sha256 !== qoderPayloads[version].sha256 || !validQoderDeploymentToken(p.token)) throw new Error('Qoder 部署响应不兼容。');
  return { token: p.token };
}
export function validateQoderDeploymentOutcome(value: unknown, version: QoderRelease = '1.1.85'): QoderDeploymentOutcome {
  const p = report(value, version);
  if (p.state !== 'private_executable_deployed_not_activated' || p.execution_verified !== false || p.activated !== false) throw new Error('Qoder 部署响应不兼容。');
  return { state: 'private_executable_deployed_not_activated' };
}
