/**
 * Agent 状态管理（Zustand）。
 * 管理 agent 列表、安装/卸载、认证验证、记忆采集等全局状态。
 */

import { create } from 'zustand';
import type { AgentInfo, MemoryEntry } from '../bridge';
import * as bridge from '../bridge';

export type InstallTargetSnapshot = Pick<bridge.AgentInstallPreview,
  'agentId' | 'package' | 'executable' | 'prefix' | 'globalRoot' | 'packageRoot' | 'launcherDirectory' | 'managerPath' | 'nodePath'>;
export interface InstallSession {
  state: 'submitted' | 'unconfirmed' | 'completed';
  target: InstallTargetSnapshot | null;
}
function targetSnapshot(plan: bridge.AgentInstallPreview): InstallTargetSnapshot {
  const { agentId, package: pkg, executable, prefix, globalRoot, packageRoot, launcherDirectory, managerPath, nodePath } = plan;
  return { agentId, package: pkg, executable, prefix, globalRoot, packageRoot, launcherDirectory, managerPath, nodePath };
}

interface AgentState {
  installSessions: Record<string, InstallSession>;

  // 数据
  agents: AgentInfo[];
  selectedAgentId: string | null;
  loading: boolean;
  previewing: boolean;
  error: string | null;
  notice: string | null;
  refreshError: string | null;
  dataStale: boolean;

  // 记忆缓存
  memories: Record<string, MemoryEntry[]>; // agentId → entries

  // 操作
  loadAgents: () => Promise<void>;
  prepareInstall: (id: string) => Promise<bridge.AgentInstallPreview | null>;
  installAgent: (id: string, revision: string) => Promise<void>;
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
  // Pending review tokens are transient and never copied into session history.
  const reviews = new Map<string, bridge.AgentInstallPreview>();
  // Refresh is deliberately outside the operation's catch: a completed install
  // must not be reported as failed merely because its follow-up read failed.
  const refreshAgents = async () => {
    try {
      const agents = await bridge.getAgents();
      set({ agents, dataStale: false, refreshError: null });
    } catch {
      set({ dataStale: true, refreshError: '加载 Agent 列表失败。当前状态待刷新，已暂停安装与配置操作。' });
    }
  };

  const runMutation = async (id: string, operation: 'install' | 'configure' | 'uninstall', revision?: string) => {
    if (get().loading) return;
    if (get().dataStale) {
      set({ error: get().error || '本地状态尚未刷新，请先刷新列表。' });
      return;
    }
    const agent = get().agents.find((entry) => entry.id === id);
    const installed = agent?.installed === true && agent.installationStatus === 'installed';
    if (operation === 'uninstall') {
      // Matching current files to a package manager does not enable management.
      set({ error: `自动卸载功能尚未提供；来源目录匹配不代表安装历史或管理权限。${agent?.uninstallHelp || '请使用原安装方式或官方卸载说明。'}` });
      return;
    }
    const evidenceProblem = agent ? bridge.installationEvidenceProblem(agent) : null;
    if (evidenceProblem) {
      set({ error: evidenceProblem === 'invalid'
        ? '安装检测证据不完整或状态不一致，已暂停安装与应用配置。请刷新列表。'
        : 'PATH 候选优先级存在歧义或当前实例未确认，已暂停安装与应用配置。请先确认实际使用的实例并刷新列表。' });
      return;
    }
    const allowed = operation === 'install'
      ? agent?.canInstall === true && agent.installed === false && agent.installationStatus === 'not_detected'
      : installed && agent?.canConfigure === true;
    if (!allowed) {
      set({ error: operation === 'install' ? '当前状态不允许自动安装，请刷新列表或查看官方说明。' : '仅能为已检测安装且支持配置的 Agent 应用配置。' });
      return;
    }
    if (operation === 'install' && !bridge.validInstallRevision(revision)) {
      set({ error: '请先检查有效安装预览。' });
      return;
    }
    if (operation === 'install') {
      const plan = reviews.get(id);
      const target = plan?.agentId === id && plan.revision === revision ? targetSnapshot(plan) : null;
      reviews.delete(id);
      set(state => ({ installSessions: { ...state.installSessions, [id]: { state: 'submitted', target } } }));
    }
    set({ loading: true, dataStale: true, error: null, notice: null, refreshError: null });
    try {
      const outcome = await (operation === 'install' ? bridge.installAgent(id, revision!) : bridge.configureAgent(id));
      if (operation === 'install') set(state => ({ installSessions: {
        ...state.installSessions, [id]: { ...state.installSessions[id], state: 'completed' },
      } }));
      const notice = operation === 'install'
        ? '软件安装操作已完成，未应用配置。'
        : '默认 profile 配置操作已完成（写前备份）。';
      set({ notice: outcome.warnings.length ? `${notice}注意：${outcome.warnings.join('；')}` : notice });
    } catch (e) {
      if (operation === 'install') set(state => ({ installSessions: {
        ...state.installSessions, [id]: { ...state.installSessions[id], state: 'unconfirmed' },
      } }));
      set({ error: operation === 'install'
        ? '安装请求已发送，但完成状态未确认。也可能在内部预检阶段被拒绝，不能据此判断是否执行或写入；请查看本会话安装记录。不会自动重试、回滚或卸载。'
        : `配置失败（可能已写入部分配置，请检查备份与当前配置）: ${e}` });
    }
    await refreshAgents();
    set({ loading: false });
  };

  return {
    installSessions: {},
    agents: [],
    selectedAgentId: null,
    loading: false,
    previewing: false,
    error: null,
    notice: null,
    refreshError: null,
    dataStale: true,
    memories: {},

    loadAgents: async () => {
      if (get().loading) return;
      set({ loading: true, dataStale: true });
      reviews.clear();
      await refreshAgents();
      set({ loading: false });
    },

    prepareInstall: async (id: string) => {
      if (get().loading || get().dataStale) return null;
      const agent = get().agents.find(entry => entry.id === id);
      if (!agent || agent.canInstall !== true || agent.installed !== false
        || agent.installationStatus !== 'not_detected' || bridge.installationEvidenceProblem(agent)) return null;
      reviews.delete(id);
      set({ loading: true, previewing: true, error: null });
      try {
        const plan = await bridge.previewAgentInstall(id);
        if (get().agents.find(entry => entry.id === id) !== agent || get().dataStale) return null;
        reviews.set(id, { ...plan });
        return plan;
      } catch {
        set({ error: '无法取得有效安装预览，请刷新后重新检查。' });
        return null;
      } finally {
        // Cancellation/unmount only discards the UI result, not this global lock.
        set({ loading: false, previewing: false });
      }
    },
    installAgent: (id: string, revision: string) => runMutation(id, 'install', revision),
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
