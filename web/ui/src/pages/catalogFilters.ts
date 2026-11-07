import { installationEvidenceProblem, type AgentInfo } from '../bridge';

export type SupportFilter = 'all' | 'automatic' | 'manual' | 'attention';
export interface CatalogContext { native: boolean; loading: boolean; dataStale: boolean }

function statusConsistent(agent: AgentInfo): boolean {
  return (agent.installed === true && agent.installationStatus === 'installed')
    || (agent.installed === false && ['not_detected', 'unsupported', 'error'].includes(agent.installationStatus ?? ''));
}

/** Display filters only. Card, store and backend remain responsible for write authorization. */
export function matchesSupportFilter(agent: AgentInfo, filter: SupportFilter, context: CatalogContext): boolean {
  if (filter === 'all') return true;
  const evidenceProblem = installationEvidenceProblem(agent);
  if (filter === 'automatic') {
    return context.native && !context.loading && !context.dataStale
      && agent.canInstall === true && agent.installed === false
      && agent.installationStatus === 'not_detected' && evidenceProblem === null;
  }
  // This describes the catalog's installation capability, not installation state.
  // A missing legacy capability is unknown, never affirmative manual support.
  if (filter === 'manual') return agent.canInstall === false;
  return context.loading || context.dataStale || !statusConsistent(agent)
    || evidenceProblem !== null || agent.installationStatus === 'unsupported'
    || agent.installationStatus === 'error' || typeof agent.canInstall !== 'boolean';
}

export function matchesInstalledFilter(agent: AgentInfo): boolean {
  return agent.installed === true && agent.installationStatus === 'installed'
    && installationEvidenceProblem(agent) !== 'invalid';
}
