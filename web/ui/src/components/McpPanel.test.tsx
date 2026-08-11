import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { McpPanel } from './McpPanel';
import type { AgentInfo, McpServer } from '../bridge';
import { useAgentStore } from '../stores/agentStore';

vi.mock('../bridge', () => ({
  getMcpServers: vi.fn(),
  toggleMcpForAgent: vi.fn(),
}));

import * as bridge from '../bridge';

const servers: McpServer[] = [
  {
    name: 'ragflow',
    description: 'RAGFlow 本地知识库',
    kind: 'http',
    url: 'http://127.0.0.1:9382/mcp',
    args: [],
    supportedAgents: ['codex', 'opencode'],
    enabledAgents: ['codex'],
  },
];

const agents: AgentInfo[] = [
  {
    id: 'codex',
    name: 'Codex',
    installed: true,
    authVerified: false,
    source: 'open',
    uiType: 'cli+desktop',
    stars: 104000,
    popular: true,
    description: 'x',
    icon: 'codex',
    category: 'ai-coding',
  },
  {
    id: 'opencode',
    name: 'OpenCode',
    installed: true,
    authVerified: false,
    source: 'open',
    uiType: 'all',
    stars: 194000,
    popular: true,
    description: 'x',
    icon: 'opencode',
    category: 'ai-coding',
  },
];

describe('McpPanel', () => {
  beforeEach(() => {
    useAgentStore.setState({ agents });
    vi.mocked(bridge.getMcpServers).mockReset();
    vi.mocked(bridge.toggleMcpForAgent).mockReset();
    vi.mocked(bridge.getMcpServers).mockResolvedValue(servers);
  });

  it('展示服务器与按 agent 开关状态', async () => {
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText('ragflow')).toBeInTheDocument());
    const codex = screen.getByLabelText('Codex') as HTMLInputElement;
    const opencode = screen.getByLabelText('OpenCode') as HTMLInputElement;
    expect(codex.checked).toBe(true);
    expect(opencode.checked).toBe(false);
  });

  it('切换开关调用后端并乐观更新', async () => {
    vi.mocked(bridge.toggleMcpForAgent).mockResolvedValue(undefined);
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText('ragflow')).toBeInTheDocument());

    fireEvent.click(screen.getByLabelText('OpenCode'));
    await waitFor(() =>
      expect(bridge.toggleMcpForAgent).toHaveBeenCalledWith('ragflow', 'opencode', true)
    );
    expect((screen.getByLabelText('OpenCode') as HTMLInputElement).checked).toBe(true);
  });

  it('切换失败时回滚开关', async () => {
    vi.mocked(bridge.toggleMcpForAgent).mockRejectedValue(new Error('boom'));
    render(<McpPanel />);
    await waitFor(() => expect(screen.getByText('ragflow')).toBeInTheDocument());

    fireEvent.click(screen.getByLabelText('OpenCode'));
    await waitFor(() => expect(screen.getByText(/切换失败/)).toBeInTheDocument());
    expect((screen.getByLabelText('OpenCode') as HTMLInputElement).checked).toBe(false);
  });
});
