/** Fixed private deployment contract. Backend prose is never rendered as authority. */
export const qoderDeploymentVersion = '1.1.85';
export const qoderDeploymentTarget = 'linux-amd64-baseline';
export const qoderDeploymentBytes = 182543840;
export const qoderDeploymentHash = '8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b';
export interface QoderDeploymentPreview { token: string }
export interface QoderDeploymentOutcome { state: 'private_executable_deployed_not_activated' }
export const validQoderDeploymentToken = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
export const validQoderDeploymentPath = (value: unknown): value is string => typeof value === 'string' && value.startsWith('/') && new TextEncoder().encode(value).length <= 4096 && !/[\x00-\x1f\x7f]/.test(value);
function report(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Qoder 部署响应不兼容。');
  const p = value as Record<string, unknown>;
  if (p.version !== qoderDeploymentVersion || p.target !== qoderDeploymentTarget || p.executable_mode !== '0700') throw new Error('Qoder 部署响应不兼容。');
  return p;
}
export function validateQoderDeploymentPreview(value: unknown): QoderDeploymentPreview {
  const p = report(value);
  if (p.payload_bytes !== qoderDeploymentBytes || p.payload_sha256 !== qoderDeploymentHash || !validQoderDeploymentToken(p.token)) throw new Error('Qoder 部署响应不兼容。');
  return { token: p.token };
}
export function validateQoderDeploymentOutcome(value: unknown): QoderDeploymentOutcome {
  const p = report(value);
  if (p.state !== 'private_executable_deployed_not_activated' || p.execution_verified !== false || p.activated !== false) throw new Error('Qoder 部署响应不兼容。');
  return { state: 'private_executable_deployed_not_activated' };
}
