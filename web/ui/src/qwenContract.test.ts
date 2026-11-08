import { describe, it, expect } from 'vitest';
import { validateQwenReport, sectionNames } from './qwenContract';
function report() { return {
  mode: 'explicit_settings_summary', authentication: 'unknown', effective_configuration: 'unknown',
  mutation_capabilities: { configure: false, authenticate: false, install: false, restore: false, execute: false },
  version_recognition: 'reviewed_manifest_version', runtime_manifest: { status: 'read' },
  settings: { status: 'read', sections: Object.fromEntries(sectionNames.map(key => [key, true])) },
}; }
describe('Qwen IPC projection', () => {
  it('retains only fixed metadata and no backend-provided text or dynamic keys', () => {
    const raw = report();
    Object.assign(raw.settings.sections, { SECRET: 'KEY' });
    const summary = validateQwenReport({ ...raw, limitations: ['SECRET'], error: 'SECRET' });
    expect(JSON.stringify(summary)).not.toContain('SECRET');
    expect(Object.keys(summary.sections!)).toEqual([...sectionNames]);
  });
  it('rejects promoted capabilities, malformed shapes and contradictory observations', () => {
    for (const bad of [null, [], {}, { ...report(), authentication: 'authenticated' },
      { ...report(), mutation_capabilities: { ...report().mutation_capabilities, configure: true } },
      { ...report(), version_recognition: 'unknown' },
      { ...report(), settings: { status: 'read', sections: { ...report().settings.sections, model: 'SECRET' } } },
      { ...report(), settings: { status: 'unsupported', sections: {} } }]) {
      expect(() => validateQwenReport(bad)).toThrow('Qwen 响应不兼容');
    }
  });
  it('handles unsupported settings without inferring installation or authentication', () => {
    expect(validateQwenReport({ ...report(), settings: { status: 'unsupported', sections: null } }).sections).toBeNull();
  });
});
