import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import catalog from '../../../../agents/registry-v3.json';
import { AgentCard } from '../components/AgentCard';
import type { AgentInfo } from '../bridge';
import { notDetectedAgent } from '../test-fixtures/agents';
import { matchesInstalledFilter, matchesSupportFilter } from './catalogFilters';

vi.mock('../bridge', async (importOriginal) => ({
  ...await importOriginal<typeof import('../bridge')>(),
  runtimeMode: vi.fn(() => 'native'),
}));

const ids = ['cursor', 'cursor-cli', 'github-copilot-cli', 'kimi-code', 'kimi-code-desktop', 'qoder-cli'] as const;
const context = { native: true, loading: false, dataStale: false };

describe('mainstream manual catalog surfaces', () => {
  it.each(ids)('keeps %s discoverable without offering automatic operations', (id) => {
    const definition = catalog.agents[id];
    const agent: AgentInfo = {
      ...notDetectedAgent,
      id, name: definition.name, description: definition.description,
      uiType: definition.ui_type as AgentInfo['uiType'],
      homepage: definition.homepage, stars: 0, popular: false,
      installed: false, authVerified: false,
      installationStatus: 'unsupported', installationMessage: '不支持自动检测',
      installationDetails: { probe: { status: 'unsupported', message: '不支持自动检测' }, instances: [], ambiguous: false, message: null },
      canInstall: false, canVerifyInstall: false, canConfigure: false, canUninstall: false,
      installHelp: '仅提供官方手动指引，不执行安装、验证或配置。',
    };
    expect(matchesSupportFilter(agent, 'manual', context)).toBe(true);
    expect(matchesSupportFilter(agent, 'attention', context)).toBe(true);
    expect(matchesSupportFilter(agent, 'automatic', context)).toBe(false);
    expect(matchesInstalledFilter(agent)).toBe(false);
    const onInstall = vi.fn();
    const onConfigure = vi.fn();
    render(<AgentCard onPrepareInstall={vi.fn()} agent={agent} selected={false} onInstall={onInstall} onConfigure={onConfigure} onUninstall={vi.fn()} onSelect={vi.fn()} />);
    expect(screen.getByText('安装状态未知')).toBeInTheDocument();
    expect(screen.getByText(definition.ui_type === 'desktop' ? '桌面应用' : '终端 CLI')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '官方说明' })).toHaveAttribute('href', definition.homepage);
    const install = screen.getByRole('button', { name: `安装 ${definition.name}` });
    expect(install).toBeDisabled();
    fireEvent.click(install);
    expect(onInstall).not.toHaveBeenCalled();
    expect(onConfigure).not.toHaveBeenCalled();
  });
});
