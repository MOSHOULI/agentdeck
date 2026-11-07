import { describe, expect, it } from 'vitest';
import type { AgentInfo } from '../bridge';
import { notDetectedAgent, installedAgent, ambiguousAgent, activeInstance } from '../test-fixtures/agents';
import { matchesInstalledFilter, matchesSupportFilter, type CatalogContext } from './catalogFilters';

const fresh: CatalogContext = { native: true, loading: false, dataStale: false };
const passive: AgentInfo = {
  ...notDetectedAgent, id: 'qwen-code', name: 'Qwen Code', canInstall: false, installationStatus: 'unsupported',
  installationDetails: {
    probe: { status: 'unsupported', message: 'Passive only' }, ambiguous: false, message: null,
    instances: [{ ...activeInstance, version: null }],
  },
};

describe('catalog display filters', () => {
  it('only advertises current automatic installation with fresh, consistent and supported observations', () => {
    expect(matchesSupportFilter(notDetectedAgent, 'automatic', fresh)).toBe(true);
    for (const candidate of [installedAgent, ambiguousAgent, passive,
      { ...notDetectedAgent, canInstall: undefined },
      { ...notDetectedAgent, installationStatus: undefined },
      { ...notDetectedAgent, installed: true },
      { ...notDetectedAgent, installationDetails: { probe: { status: 'installed' as const, message: null }, instances: [], ambiguous: false, message: null } },
    ]) expect(matchesSupportFilter(candidate, 'automatic', fresh)).toBe(false);
    for (const context of [{ ...fresh, native: false }, { ...fresh, loading: true }, { ...fresh, dataStale: true }]) {
      expect(matchesSupportFilter(notDetectedAgent, 'automatic', context)).toBe(false);
    }
  });

  it('keeps a passive PATH candidate manual and uncertain, never installed or automatically installable', () => {
    expect(matchesSupportFilter(passive, 'manual', fresh)).toBe(true);
    expect(matchesSupportFilter(passive, 'attention', fresh)).toBe(true);
    expect(matchesSupportFilter(passive, 'automatic', fresh)).toBe(false);
    expect(matchesInstalledFilter(passive)).toBe(false);
  });

  it('distinguishes explicit manual capability from missing legacy capability', () => {
    expect(matchesSupportFilter({ ...installedAgent, canInstall: false }, 'manual', fresh)).toBe(true);
    expect(matchesSupportFilter({ ...notDetectedAgent, canInstall: undefined }, 'manual', fresh)).toBe(false);
    expect(matchesSupportFilter({ ...notDetectedAgent, canInstall: undefined }, 'attention', fresh)).toBe(true);
  });

  it('groups errors, ambiguous, invalid and stale results without conflating them with damaged installations', () => {
    for (const candidate of [passive, ambiguousAgent,
      { ...notDetectedAgent, installationStatus: 'error' as const },
      { ...notDetectedAgent, installed: true },
      { ...notDetectedAgent, installationStatus: undefined },
      { ...notDetectedAgent, installationDetails: {} as AgentInfo['installationDetails'] },
    ]) expect(matchesSupportFilter(candidate, 'attention', fresh)).toBe(true);
    expect(matchesSupportFilter(installedAgent, 'attention', fresh)).toBe(false);
    expect(matchesSupportFilter(notDetectedAgent, 'attention', fresh)).toBe(false);
    expect(matchesSupportFilter(installedAgent, 'attention', { ...fresh, dataStale: true })).toBe(true);
    expect(matchesSupportFilter(notDetectedAgent, 'attention', { ...fresh, loading: true })).toBe(true);
  });

  it('excludes inconsistent installed evidence but retains verified detection with priority ambiguity', () => {
    expect(matchesInstalledFilter(installedAgent)).toBe(true);
    expect(matchesInstalledFilter(ambiguousAgent)).toBe(true);
    expect(matchesInstalledFilter({ ...installedAgent, installationDetails: { probe: { status: 'unsupported', message: null }, instances: [], ambiguous: false, message: null } })).toBe(false);
    expect(matchesInstalledFilter({ ...installedAgent, installed: false })).toBe(false);
    expect(matchesInstalledFilter({ ...installedAgent, installationStatus: undefined })).toBe(false);
  });

  it('never hides any record in the all-capabilities view', () => {
    for (const candidate of [notDetectedAgent, installedAgent, passive, ambiguousAgent]) {
      expect(matchesSupportFilter(candidate, 'all', { native: false, loading: true, dataStale: true })).toBe(true);
    }
  });
});
