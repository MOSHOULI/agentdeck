import type { AgentInfo, InstallationInstance } from '../bridge';

// The same DTO contract is used by store, component and navigation regressions.
export const notDetectedAgent: AgentInfo = {
  id: 'opencode', name: 'OpenCode', installed: false,
  installationStatus: 'not_detected', installationMessage: null,
  installSource: 'unknown', uninstallHelp: '安装来源未验证，请使用原安装方式或官方卸载说明。',
  canInstall: true, canVerifyInstall: true, canUninstall: false, canConfigure: true,
  authVerified: false, source: 'open', uiType: 'all', stars: 194000,
  popular: true, description: '开源 AI 编程代理', icon: 'opencode', category: 'ai-coding',
  homepage: 'https://opencode.ai',
};

export const installedAgent: AgentInfo = {
  ...notDetectedAgent, installed: true, installationStatus: 'installed',
  installationDetails: {
    probe: { status: 'installed', message: null }, instances: [], ambiguous: false, message: null,
  },
};

// Entirely synthetic observations. Never execute or inspect these paths.
export const activeInstance: InstallationInstance = {
  path: '/synthetic/bin/example-agent',
  resolvedPath: '/synthetic/lib/example-agent/launcher',
  entrypointPath: null,
  version: '2.4.0', active: true,
  source: { kind: 'unknown', reason: '合成夹具：启动文件无法证明安装来源。' },
};

export const observedAgent: AgentInfo = {
  ...installedAgent, version: '2.4.0',
  installationDetails: {
    probe: { status: 'installed', message: null },
    instances: [activeInstance], ambiguous: false, message: null,
  },
};

export const npmMatchedAgent: AgentInfo = {
  ...observedAgent, installSource: 'npm', uninstallHelp: '请参阅官方手动卸载说明。',
  installationDetails: {
    ...observedAgent.installationDetails!,
    instances: [{
      ...activeInstance,
      resolvedPath: '/synthetic/npm/bin/example-agent.cmd',
      entrypointPath: '/synthetic/npm/node_modules/example-agent/dist/cli.js',
      source: {
        kind: 'npm', package: 'example-agent', packageRoot: '/synthetic/npm/node_modules/example-agent',
        globalRoot: '/synthetic/npm/node_modules', prefix: '/synthetic/npm', managerPath: '/synthetic/npm/npm.cmd',
      },
    }],
  },
};

export const ambiguousAgent: AgentInfo = {
  ...observedAgent,
  installationDetails: {
    ...observedAgent.installationDetails!, ambiguous: true,
    message: '合成夹具：PATH 中存在多个不同实例，当前优先级需要确认。',
    instances: [activeInstance, {
      path: '/synthetic/other-bin/example-agent', resolvedPath: '/synthetic/other-lib/example-agent/cli',
      entrypointPath: null, version: null, active: false,
      source: { kind: 'unknown', reason: '合成夹具：仅观察到另一个候选，没有执行版本检测。' },
    }],
  },
};

export const agentCatalog: AgentInfo[] = [
  notDetectedAgent,
  { ...installedAgent, id: 'cli-tool', name: 'CLI Tool', version: '1.0.0', uiType: 'cli', stars: 100, popular: false, description: '终端工具' },
  { ...notDetectedAgent, id: 'manual', name: 'Manual Agent', uiType: 'vscode-extension', installationStatus: 'unsupported', installationMessage: '请在 VS Code 查看扩展状态。', canInstall: false, canVerifyInstall: false, canConfigure: false, stars: 200, popular: false, description: '手动管理扩展' },
];
