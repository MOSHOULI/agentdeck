import manifest87 from '../../../../docs/upstream/qoder/manifest-1.1.87.json';
import manifest86 from '../../../../docs/upstream/qoder/manifest-1.1.86.json';
import type { QoderRelease } from '../qoderReleases';
import manifest from '../../../../docs/upstream/qoder/manifest-1.1.85.json';
export function qoderRawPlan(target = 'linux-amd64-baseline', release: QoderRelease = '1.1.85') {
  const file = ({ '1.1.85': manifest, '1.1.86': manifest86, '1.1.87': manifest87 }[release]).files.find(file => `${file.os}-${file.arch}${file.variant ? `-${file.variant}` : ''}` === target)!;
  return { target, version: release, manifest_sha256: release === '1.1.87' ? '5bea07a5bb10187aac873a1b3c5f7362e0b18f464daf4612cdc804cde31cd8ba' : release === '1.1.85' ? 'a78234316fe2065cdae6fc17ac425bbc2ae11006d04b6ab5216fd6093597e4de' : 'f323905da3f75088b1e9ae4cf9b0fef6f117879f4f6942b609e92228a533fa78', url: file.url, sha256: file.sha256,
    archive_name: file.url.split('/').pop()!, runtime: file.runtime ?? 'not_declared',
    variant: file.variant ?? (target === 'linux-amd64' ? 'optimized' : target.endsWith('-baseline') ? 'baseline' : target.endsWith('-musl') ? 'musl' : 'not_declared'),
    min_windows_build: file.min_windows_build ?? null, host_eligibility: 'not_evaluated', install_supported: false, execution_verified: false, signature_verified: false, max_archive_bytes: 268435456,
    requirements: 'SECRET', limitations: 'SECRET', path: 'SECRET' };
}
