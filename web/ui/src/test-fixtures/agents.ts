import type { AgentInfo } from '../bridge';

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
};

export const agentCatalog: AgentInfo[] = [
  notDetectedAgent,
  { ...installedAgent, id: 'cli-tool', name: 'CLI Tool', version: '1.0.0', uiType: 'cli', stars: 100, popular: false, description: '终端工具' },
  { ...notDetectedAgent, id: 'manual', name: 'Manual Agent', uiType: 'vscode-extension', installationStatus: 'unsupported', installationMessage: '请在 VS Code 查看扩展状态。', canInstall: false, canVerifyInstall: false, canConfigure: false, stars: 200, popular: false, description: '手动管理扩展' },
];
