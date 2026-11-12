export const qoderReleases = ['1.1.85', '1.1.86', '1.1.87'] as const;
export type QoderRelease = typeof qoderReleases[number];
export function requireQoderRelease(value: string): asserts value is QoderRelease {
  if (!(qoderReleases as readonly string[]).includes(value)) throw new Error('请选择已审阅的 Qoder 版本。');
}
export const qoderPayloads = {
  '1.1.85': { bytes: 182543840, sha256: '8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b' },
  '1.1.86': { bytes: 182412768, sha256: 'fccd0ce90d1119f6bb213146de6e45358db0c89585048c31315ddb180c925ae8' },
  '1.1.87': { bytes: 182466016, sha256: '61e318f6c4d2ebae319aecfce2f18472c51e7b413a546accddaeb82e2592af2e' },
} as const;
