// Compact reviewed evidence only. Tests pin this projection to artifacts.json;
// member lists and source records never enter the application bundle.
export const dshNativePackages = ['node-pty', 'koffi', 'koffi-linux-x64', 'koffi-win32-x64'] as const;
export type DshNativePackage = typeof dshNativePackages[number];
export const dshNativeEvidence = {
  "node-pty": {
    "package": "node-pty",
    "version": "1.2.0-beta.15",
    "archive_name": "node-pty-1.2.0-beta.15.tgz",
    "bytes": 7150506,
    "sha256": "114ac80c3fe075eff76217a4122d135576582695f49c03a5da3835cdfc2f89c5",
    "integrity": "sha512-vORSzHXi4Ofl7HemVWpuudLqCPdaQb4LfpRCUpE5HPxhp4JYscl8zZwxh11p26v2wvW24WMwnMfLjhRLixrfxA==",
    "regular_member_count": 49
  },
  "koffi": {
    "package": "koffi",
    "version": "3.1.1",
    "archive_name": "koffi-3.1.1.tgz",
    "bytes": 302173,
    "sha256": "d6e3cb95f5d155d16cfe40f39ce09be18568c7f84dfb0b0722820a0da9e03b17",
    "integrity": "sha512-mRX6AMeeKCxSOeOopqAcLAl5jcNvge7NAG8l7rF/8gGJATI0tdHFYjteIdE0mGOtWdsrJOij+PjnP8Q9c1gwgA==",
    "regular_member_count": 81
  },
  "koffi-linux-x64": {
    "package": "@koromix/koffi-linux-x64",
    "version": "3.1.1",
    "archive_name": "koffi-linux-x64-3.1.1.tgz",
    "bytes": 614185,
    "sha256": "0b4038a46282a354aab77d6d05059ac8a8fccb0dfd231b4c18acb8623953d1fe",
    "integrity": "sha512-pyTcX5fePeYbt7TZAwRby69wdlRx3PT+g15ra5IYdat/Pgh3qAKEYeZ+uu7WpPGOy43p/oSRqqZoa2kORzozlA==",
    "regular_member_count": 5
  },
  "koffi-win32-x64": {
    "package": "@koromix/koffi-win32-x64",
    "version": "3.1.1",
    "archive_name": "koffi-win32-x64-3.1.1.tgz",
    "bytes": 399006,
    "sha256": "b0be9b6a916b9f5de5b97fb5de844efe1121f7ed44cafb481a1479c7c90aec9b",
    "integrity": "sha512-KqHPmvj6QILhNyI/To8QSihHsijeVGIYYPBOUnXEpcnH2LuLbargY4Hd6dDeTN3Z90uUUxN+1FWz1UnhVzFOiA==",
    "regular_member_count": 5
  }
} as const;

const staticClaims = {
  schema_version: 1,
  state: 'native_archive_static_verified',
  archive_verified_during_this_command: true,
  reviewed_manifest_and_sources_matched: true,
  metadata_archive_agreement: 'reviewed_declaration_fields_match',
  lifecycle_executed: false,
  native_code_loaded: false,
  closure_complete: false,
  install_ready: false,
  dependencies_resolved: false,
  runtime_verified: false,
  signature_verified: false,
  platform_compatibility: 'unverified',
} as const;
export type DshNativeVerification = typeof staticClaims & {
  package_id: DshNativePackage;
  package: string; version: string; archive_name: string;
  bytes: number; sha256: string; integrity: string; regular_member_count: number;
};
export function requireDshNativePackage(value: unknown): asserts value is DshNativePackage {
  if (!dshNativePackages.some(id => id === value)) throw new Error('DSH_NATIVE_INVALID_PACKAGE');
}
export function validateDshNativeVerification(value: unknown, selected: DshNativePackage): DshNativeVerification {
  requireDshNativePackage(selected);
  const expected = {...staticClaims, package_id: selected, ...dshNativeEvidence[selected]};
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('DSH_NATIVE_INVALID_RESPONSE');
  const record = value as Record<string, unknown>;
  if (Object.keys(record).length !== Object.keys(expected).length ||
      Object.entries(expected).some(([key, fixed]) => !Object.prototype.hasOwnProperty.call(record, key) || record[key] !== fixed)) {
    throw new Error('DSH_NATIVE_INVALID_RESPONSE');
  }
  return expected;
}
