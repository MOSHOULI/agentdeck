import { expect, it } from 'vitest';
import { parseResidual, residualSlots, validSessionRef } from './codexResidualContract';
const report = () => ({ schemaVersion: 1, agentId: 'codex', runtimeNotRun: true, ownership: 'unknown', cleanupSupported: false, limitations: 'private', slots: residualSlots.map(slot => ({ slot, state: 'missing', secret: 'private' })), path: 'private' });
it('projects only fixed slot labels and enum states', () => {
  const result = parseResidual(report());
  expect(JSON.stringify(result)).not.toContain('private');
  expect(result).toEqual(residualSlots.map(slot => ({ slot, state: 'missing' })));
});
it.each(['schemaVersion', 'agentId', 'runtimeNotRun', 'ownership', 'cleanupSupported', 'slots'])('rejects invalid %s', key => {
  expect(() => parseResidual({ ...report(), [key]: null })).toThrow();
});
it('rejects unknown, duplicate or content-bearing states', () => {
  for (const change of [{ slot: 'C:\\private' }, { state: 'private' }, { slot: 'cmd_launcher' }]) {
    const raw = report(); Object.assign(raw.slots[0], change); expect(() => parseResidual(raw)).toThrow();
  }
});
it('accepts only opaque UUID v4 session references', () => {
  expect(validSessionRef('12345678-1234-4123-8123-123456789abc')).toBe(true);
  for (const value of ['', 'C:\\prefix', 'a'.repeat(64), null]) expect(validSessionRef(value)).toBe(false);
});

it('rejects string-coercible state objects', () => {
  for (const state of [['file'], { toString: () => 'file' }]) {
    const raw = report(); (raw.slots[0] as { state: unknown }).state = state;
    expect(() => parseResidual(raw)).toThrow();
  }
});
