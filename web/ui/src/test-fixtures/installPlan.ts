import type { AgentInstallPreview } from '../bridge';

export const installPlan: AgentInstallPreview = {
  schemaVersion: 1, agentId: 'opencode', package: 'opencode-ai', executable: 'opencode',
  managerPath: '/synthetic/npm/bin/npm-cli.js', nodePath: '/synthetic/bin/node',
  prefix: '/synthetic/global', globalRoot: '/synthetic/global/lib/node_modules',
  packageRoot: '/synthetic/global/lib/node_modules/opencode-ai',
  launcherDirectory: '/synthetic/global/bin', scriptPolicy: 'npm_configured',
  versionPolicy: 'npm_configured', revision: 'a'.repeat(64),
};
