/**
 * Agent 状态管理（Zustand）。
 * 管理 agent 列表、安装/卸载、认证验证、记忆采集等全局状态。
 */

import { create } from 'zustand';
import type { AgentInfo, MemoryEntry } from '../bridge';
import * as bridge from '../bridge';

interface AgentState {
  // 数据
  agents: AgentInfo[];
  selectedAgentId: string | null;
  loading: boolean;
  error: string | null;
  notice: string | null;
  refreshError: string | null;
  dataStale: boolean;

  // 记忆缓存
  memories: Record<string, MemoryEntry[]>; // agentId → entries

  // 操作
  loadAgents: () => Promise<void>;
  installAgent: (id: string) => Promise<void>;
  configureAgent: (id: string) => Promise<void>;
  uninstallAgent: (id: string) => Promise<void>;
  verifyAuth: (id: string) => Promise<boolean>;
  selectAgent: (id: string | null) => void;
  collectMemories: (agentId: string) => Promise<void>;
  searchMemories: (query: string) => Promise<MemoryEntry[]>;
  injectMemories: (agentId: string, memoryIds: string[]) => Promise<void>;
  toggleAutoInject: (memoryId: string, enabled: boolean) => Promise<void>;
  clearError: () => void;
}

export const useAgentStore = create<AgentState>((set, get) => {
  // Refresh is deliberately outside the operation's catch: a completed install
  // must not be reported as failed merely because its follow-up read failed.
  const refreshAgents = async () => {
    try {
      const agents = await bridge.getAgents();
      set({ agents, dataStale: false, refreshError: null });
    } catch (e) {
      set({ dataStale: true, refreshError: `加载 Agent 列表失败: ${e}。当前状态待刷新，已暂停安装与配置操作。` });
    }
  };

  const runMutation = async (id: string, operation: 'install' | 'configure' | 'uninstall') => {
    if (get().loading) return;
    if (get().dataStale) {
      set({ error: get().error || '本地状态尚未刷新，请先刷新列表。' });
      return;
    }
    const agent = get().agents.find((entry) => entry.id === id);
    const installed = agent?.installed === true && agent.installationStatus === 'installed';
    if (operation === 'uninstall') {
      // No currently supported install source has verified provenance. Do not
      // infer package-manager ownership from a binary found on PATH.
      set({ error: agent?.uninstallHelp || '安装来源未验证，无法自动卸载。请使用原安装方式或官方卸载说明。' });
      return;
    }
    const allowed = operation === 'install'
      ? agent?.canInstall === true && agent.installed === false && agent.installationStatus === 'not_detected'
      : installed && agent?.canConfigure === true;
    if (!allowed) {
      set({ error: operation === 'install' ? '当前状态不允许自动安装，请刷新列表或查看官方说明。' : '仅能为已检测安装且支持配置的 Agent 应用配置。' });
      return;
    }
    set({ loading: true, dataStale: true, error: null, notice: null, refreshError: null });
    try {
      const outcome = await (operation === 'install' ? bridge.installAgent(id) : bridge.configureAgent(id));
      const notice = operation === 'install'
        ? '软件安装操作已完成，未应用配置。'
        : '默认 profile 配置操作已完成（写前备份）。';
      set({ notice: outcome.warnings.length ? `${notice}注意：${outcome.warnings.join('；')}` : notice });
    } catch (e) {
      set({ error: operation === 'install'
        ? `安装失败（软件可能已安装，请查看检测状态）: ${e}`
        : `配置失败（可能已写入部分配置，请检查备份与当前配置）: ${e}` });
    }
    await refreshAgents();
    set({ loading: false });
  };

  return {
    agents: [],
    selectedAgentId: null,
    loading: false,
    error: null,
    notice: null,
    refreshError: null,
    dataStale: true,
    memories: {},

    loadAgents: async () => {
      if (get().loading) return;
      set({ loading: true, dataStale: true });
      await refreshAgents();
      set({ loading: false });
    },

    installAgent: (id: string) => runMutation(id, 'install'),
    configureAgent: (id: string) => runMutation(id, 'configure'),
    uninstallAgent: (id: string) => runMutation(id, 'uninstall'),

    verifyAuth: async (id: string) => {
      try {
        const verified = await bridge.verifyAgentAuth(id);
        set((state) => ({
          agents: state.agents.map((a) =>
            a.id === id ? { ...a, authVerified: verified } : a
          ),
        }));
        return verified;
      } catch (e) {
        set({ error: `认证检查失败: ${e}` });
        return false;
      }
    },

    selectAgent: (id: string | null) => set({ selectedAgentId: id }),

    collectMemories: async (agentId: string) => {
      try {
        const entries = await bridge.collectMemory(agentId);
        set((state) => ({
          memories: { ...state.memories, [agentId]: entries },
        }));
      } catch (e) {
        set({ error: `采集记忆失败: ${e}` });
        throw e;
      }
    },

    searchMemories: async (query: string) => {
      return bridge.searchMemory(query);
    },

    injectMemories: async (agentId: string, memoryIds: string[]) => {
      try {
        await bridge.injectMemory(agentId, memoryIds);
      } catch (e) {
        set({ error: `注入记忆失败: ${e}` });
        throw e;
      }
    },

    toggleAutoInject: async (memoryId: string, enabled: boolean) => {
      try {
        await bridge.toggleAutoInject(memoryId, enabled);
      } catch (e) {
        set({ error: `切换自动注入失败: ${e}` });
        throw e;
      }
    },

    clearError: () => set({ error: null }),
  };
});
