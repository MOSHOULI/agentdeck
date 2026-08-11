/**
 * Agent 广场 —— 主界面。
 * 以卡片网格展示全部 17 款 Agent，支持安装/卸载/选择。
 * 首行展示「热度推荐」，后续按 GitHub Stars 降序排列。
 */

import React, { useEffect, useState } from 'react';
import { useAgentStore } from '../stores/agentStore';
import { AgentCard } from '../components/AgentCard';

export const AgentSquare: React.FC = () => {
  const {
    agents,
    loading,
    error,
    selectedAgentId,
    loadAgents,
    installAgent,
    uninstallAgent,
    selectAgent,
    clearError,
  } = useAgentStore();

  const [filter, setFilter] = useState<'all' | 'installed' | 'popular'>('all');

  useEffect(() => {
    loadAgents();
  }, [loadAgents]);

  // 按规则排序：热度标记优先 → 星数降序
  const sortedAgents = [...agents].sort((a, b) => {
    if (a.popular && !b.popular) return -1;
    if (!a.popular && b.popular) return 1;
    return (b.stars ?? 0) - (a.stars ?? 0);
  });

  const filteredAgents = sortedAgents.filter((a) => {
    if (filter === 'installed') return a.installed;
    if (filter === 'popular') return a.popular;
    return true;
  });

  // 分组：有面板 / 纯 CLI
  const withUI = filteredAgents.filter((a) =>
    ['all', 'cli+desktop', 'desktop', 'vscode-extension', 'ide-extension'].includes(a.uiType)
  );
  const pureCli = filteredAgents.filter((a) =>
    ['cli', 'cli-bot'].includes(a.uiType)
  );

  return (
    <div style={{ padding: 24, maxWidth: 1200, margin: '0 auto' }}>
      {/* 页头 */}
      <div style={{ marginBottom: 24 }}>
        <h1 style={{ fontSize: 28, fontWeight: 700, color: '#f3f4f6', margin: '0 0 8px 0' }}>
          Agent 广场
        </h1>
        <p style={{ fontSize: 14, color: '#9ca3af', margin: 0 }}>
          一站式发现、安装、配置本地 AI Agent。共 {agents.length} 款 Agent 可用。
        </p>
      </div>

      {/* 筛选 */}
      <div style={{ display: 'flex', gap: 8, marginBottom: 20 }}>
        {(['all', 'popular', 'installed'] as const).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            style={{
              padding: '6px 16px',
              borderRadius: 8,
              border: filter === f ? '1px solid #6366f1' : '1px solid #374151',
              background: filter === f ? '#1e1b4b' : 'transparent',
              color: filter === f ? '#a5b4fc' : '#9ca3af',
              fontSize: 13,
              cursor: 'pointer',
              fontWeight: 500,
            }}
          >
            {f === 'all' ? '全部' : f === 'popular' ? '🔥 热度推荐' : '✓ 已安装'}
          </button>
        ))}
      </div>

      {/* 错误提示 */}
      {error && (
        <div
          style={{
            padding: 12,
            borderRadius: 8,
            background: '#7f1d1d',
            color: '#fca5a5',
            fontSize: 13,
            marginBottom: 16,
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
          }}
        >
          <span>{error}</span>
          <button
            onClick={clearError}
            style={{
              background: 'transparent',
              border: 'none',
              color: '#fca5a5',
              cursor: 'pointer',
              fontSize: 16,
            }}
          >
            ×
          </button>
        </div>
      )}

      {/* 加载中 */}
      {loading && <p style={{ color: '#6b7280', fontSize: 14 }}>加载中...</p>}

      {/* 🖥️ 有桌面/GUI 面板 */}
      {withUI.length > 0 && (
        <>
          <h2 style={{ fontSize: 18, fontWeight: 600, color: '#e5e7eb', marginBottom: 12 }}>
            🖥️ 桌面 / GUI / IDE 面板
          </h2>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))',
              gap: 16,
              marginBottom: 32,
            }}
          >
            {withUI.map((agent) => (
              <AgentCard
                key={agent.id}
                agent={agent}
                selected={selectedAgentId === agent.id}
                onInstall={installAgent}
                onUninstall={uninstallAgent}
                onSelect={selectAgent}
              />
            ))}
          </div>
        </>
      )}

      {/* ⌨️ 纯 CLI */}
      {pureCli.length > 0 && (
        <>
          <h2 style={{ fontSize: 18, fontWeight: 600, color: '#e5e7eb', marginBottom: 12 }}>
            ⌨️ 纯终端 CLI
          </h2>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))',
              gap: 16,
            }}
          >
            {pureCli.map((agent) => (
              <AgentCard
                key={agent.id}
                agent={agent}
                selected={selectedAgentId === agent.id}
                onInstall={installAgent}
                onUninstall={uninstallAgent}
                onSelect={selectAgent}
              />
            ))}
          </div>
        </>
      )}

      {/* 空状态 */}
      {!loading && filteredAgents.length === 0 && (
        <div style={{ textAlign: 'center', padding: 60, color: '#6b7280' }}>
          {filter === 'installed' ? '还没有安装任何 Agent' : '没有找到 Agent'}
        </div>
      )}
    </div>
  );
};
