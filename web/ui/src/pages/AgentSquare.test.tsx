import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { AgentSquare } from './AgentSquare';
import type { AgentInfo } from '../bridge';

vi.mock('../bridge', () => ({
  getAgents: vi.fn(),
  installAgent: vi.fn(),
  uninstallAgent: vi.fn(),
  verifyAgentAuth: vi.fn(),
  collectMemory: vi.fn(),
  searchMemory: vi.fn(),
  injectMemory: vi.fn(),
  toggleAutoInject: vi.fn(),
}));

import * as bridge from '../bridge';

const agents: AgentInfo[] = [
  {
    id: 'opencode',
    name: 'OpenCode',
    installed: false,
    authVerified: false,
    source: 'open',
    uiType: 'all',
    stars: 194000,
    popular: true,
    description: '开源 AI 编程代理',
    icon: 'opencode',
    category: 'ai-coding',
  },
  {
    id: 'cli-tool',
    name: 'CLI Tool',
    installed: false,
    authVerified: false,
    source: 'open',
    uiType: 'cli',
    stars: 100,
    popular: false,
    description: '终端工具',
    icon: 'cli-tool',
    category: 'ai-coding',
  },
];

describe('AgentSquare', () => {
  beforeEach(() => {
    vi.mocked(bridge.getAgents).mockReset();
  });

  it('渲染标题与三个筛选按钮', async () => {
    vi.mocked(bridge.getAgents).mockResolvedValue(agents);
    render(<AgentSquare />);
    expect(screen.getByText('Agent 广场')).toBeInTheDocument();
    expect(screen.getByText('全部')).toBeInTheDocument();
    expect(screen.getByText('🔥 热度推荐')).toBeInTheDocument();
    expect(screen.getByText('✓ 已安装')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText('OpenCode')).toBeInTheDocument());
    expect(screen.getByText('CLI Tool')).toBeInTheDocument();
  });

  it('getAgents 失败时显示错误横幅', async () => {
    vi.mocked(bridge.getAgents).mockRejectedValue(new Error('boom'));
    render(<AgentSquare />);
    await waitFor(() =>
      expect(screen.getByText(/加载 Agent 列表失败/)).toBeInTheDocument()
    );
  });
});
