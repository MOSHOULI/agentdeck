import manifest from '../../../../docs/upstream/qoder/manifest-1.1.85.json';
export function qoderRawPlan(target = 'linux-amd64-baseline') {
  const file = manifest.files.find(file => `${file.os}-${file.arch}${file.variant ? `-${file.variant}` : ''}` === target)!;
  return { target, version: '1.1.85', manifest_sha256: 'a78234316fe2065cdae6fc17ac425bbc2ae11006d04b6ab5216fd6093597e4de', url: file.url, sha256: file.sha256,
    archive_name: file.url.split('/').pop()!, runtime: file.runtime ?? 'not_declared',
    variant: file.variant ?? (target === 'linux-amd64' ? 'optimized' : target.endsWith('-baseline') ? 'baseline' : target.endsWith('-musl') ? 'musl' : 'not_declared'),
    min_windows_build: file.min_windows_build ?? null, host_eligibility: 'not_evaluated', install_supported: false, execution_verified: false, signature_verified: false, max_archive_bytes: 268435456,
    requirements: 'SECRET', limitations: 'SECRET', path: 'SECRET' };
}
