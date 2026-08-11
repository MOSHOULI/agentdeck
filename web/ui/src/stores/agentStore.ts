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

  // 记忆缓存
  memories: Record<string, MemoryEntry[]>; // agentId → entries

  // 操作
  loadAgents: () => Promise<void>;
  installAgent: (id: string) => Promise<void>;
  uninstallAgent: (id: string) => Promise<void>;
  verifyAuth: (id: string) => Promise<boolean>;
  selectAgent: (id: string | null) => void;
  collectMemories: (agentId: string) => Promise<void>;
  searchMemories: (query: string) => Promise<MemoryEntry[]>;
  injectMemories: (agentId: string, memoryIds: string[]) => Promise<void>;
  toggleAutoInject: (memoryId: string, enabled: boolean) => Promise<void>;
  clearError: () => void;
}

export const useAgentStore = create<AgentState>((set) => ({
  agents: [],
  selectedAgentId: null,
  loading: false,
  error: null,
  memories: {},

  loadAgents: async () => {
    set({ loading: true, error: null });
    try {
      const agents = await bridge.getAgents();
      set({ agents, loading: false });
    } catch (e) {
      set({ error: `加载 Agent 列表失败: ${e}`, loading: false });
    }
  },

  installAgent: async (id: string) => {
    set({ loading: true, error: null });
    try {
      await bridge.installAgent(id);
      const agents = await bridge.getAgents();
      set({ agents, loading: false });
    } catch (e) {
      set({ error: `安装失败: ${e}`, loading: false });
    }
  },

  uninstallAgent: async (id: string) => {
    set({ loading: true, error: null });
    try {
      await bridge.uninstallAgent(id);
      const agents = await bridge.getAgents();
      set({ agents, loading: false });
    } catch (e) {
      set({ error: `卸载失败: ${e}`, loading: false });
    }
  },

  verifyAuth: async (id: string) => {
    try {
      const verified = await bridge.verifyAgentAuth(id);
      set((state) => ({
        agents: state.agents.map((a) =>
          a.id === id ? { ...a, authVerified: verified } : a
        ),
      }));
      return verified;
    } catch {
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
}));
