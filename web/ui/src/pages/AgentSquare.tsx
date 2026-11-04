import React, { useEffect, useMemo, useState } from 'react';
import { useAgentStore } from '../stores/agentStore';
import { AgentCard } from '../components/AgentCard';
import { Icon } from '../components/Icon';
import './AgentWorkspace.css';

const filters = [
  { id: 'all', label: '全部' },
  { id: 'popular', label: '热度推荐' },
  { id: 'installed', label: '已安装' },
] as const;

export const AgentSquare: React.FC = () => {
  const { agents, loading, error, notice, refreshError, dataStale, selectedAgentId, loadAgents, installAgent, configureAgent, uninstallAgent, selectAgent, clearError } = useAgentStore();
  const [filter, setFilter] = useState<'all' | 'installed' | 'popular'>('all');
  const [query, setQuery] = useState('');

  useEffect(() => { void loadAgents(); }, [loadAgents]);

  const filteredAgents = useMemo(() => {
    const search = query.trim().toLocaleLowerCase();
    return [...agents]
      .sort((a, b) => Number(Boolean(b.popular)) - Number(Boolean(a.popular)) || (b.stars ?? 0) - (a.stars ?? 0) || a.name.localeCompare(b.name))
      .filter((agent) => {
        if (filter === 'installed' && (!agent.installed || agent.installationStatus !== 'installed')) return false;
        if (filter === 'popular' && !agent.popular) return false;
        return !search || [agent.name, agent.id, agent.description, agent.uiType, agent.category]
          .some((value) => value.toLocaleLowerCase().includes(search));
      });
  }, [agents, filter, query]);

  const resetFilters = () => { setQuery(''); setFilter('all'); };
  const selectedAgent = agents.find((agent) => agent.id === selectedAgentId);

  return (
    <div className="agent-workspace">
      <div className="workspace-heading agent-workspace-heading">
        <div>
          <p className="eyebrow">AGENT WORKSPACE</p>
          <h1>Agent 广场</h1>
          <p className="muted">发现并管理本地 Agent，选择一个以供其他工作区使用。</p>
        </div>
        <button type="button" className="button" onClick={() => { void loadAgents(); }} disabled={loading}>
          <Icon name="refresh" size={15} />刷新列表
        </button>
      </div>

      {notice && <div role="status" className="notice agent-notice"><Icon name="info" size={16} /><span>{notice}</span></div>}
      {refreshError && <div role="alert" className="notice notice-warning agent-notice"><Icon name="alert" size={16} /><div className="agent-error-copy"><strong>状态刷新失败</strong><span>{refreshError}</span></div></div>}
      {dataStale && agents.length > 0 && <p className="notice notice-warning agent-notice">显示上次检测结果，当前状态待刷新。成功刷新前无法安装或应用配置。</p>}
      {error && (
        <div role="alert" className="notice notice-error agent-notice">
          <Icon name="alert" size={16} />
          <div className="agent-error-copy"><strong>无法完成操作</strong><span>{error}</span></div>
          <button type="button" className="button" onClick={clearError} aria-label="关闭错误提示"><Icon name="x" size={15} /></button>
        </div>
      )}

      <div className="panel agent-directory">
        <div className="toolbar agent-toolbar">
          <div className="agent-filters" role="group" aria-label="Agent 筛选">
            {filters.map(({ id, label }) => (
              <button key={id} type="button" className={`button agent-filter${filter === id ? ' agent-filter-active' : ''}`} aria-pressed={filter === id} onClick={() => setFilter(id)}>{label}</button>
            ))}
          </div>
          <div className="agent-search">
            <Icon name="search" size={16} />
            <label htmlFor="agent-search" className="sr-only">搜索 Agent</label>
            <input id="agent-search" className="control" type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索名称、ID 或描述…" />
          </div>
        </div>

        <div className="agent-list-summary">
          <span>{loading ? <span role="status">正在读取 Agent 状态…</span> : refreshError && agents.length === 0 ? '列表暂不可用' : `显示 ${filteredAgents.length} / ${agents.length} 款 Agent`}</span>
          <span>{selectedAgent ? `当前选择：${selectedAgent.name}` : '尚未选择 Agent'}</span>
        </div>

        <div className="agent-list" aria-label="Agent 列表" aria-busy={loading}>
          {filteredAgents.map((agent) => <AgentCard key={agent.id} agent={agent} busy={loading} dataStale={dataStale} selected={selectedAgentId === agent.id} onInstall={installAgent} onConfigure={configureAgent} onUninstall={uninstallAgent} onSelect={selectAgent} />)}
          {loading && agents.length === 0 && <div className="empty-state agent-empty"><Icon name="agents" size={28} /><h2>正在加载 Agent</h2><p>读取注册表与本地安装状态。</p></div>}
          {!loading && refreshError && agents.length === 0 && <div className="empty-state agent-empty"><Icon name="alert" size={28} /><h2>暂时无法读取 Agent 列表</h2><p>检查桌面后端连接后重试。</p><button type="button" className="button" onClick={() => { void loadAgents(); }}><Icon name="refresh" size={15} />重新加载</button></div>}
          {!loading && (!refreshError || agents.length > 0) && filteredAgents.length === 0 && (
            <div className="empty-state agent-empty">
              <Icon name={query ? 'search' : 'agents'} size={28} />
              <h2>{query ? '没有匹配的 Agent' : filter === 'installed' ? '还没有检测到已安装的 Agent' : filter === 'popular' ? '暂无热度推荐' : '暂无可显示的 Agent'}</h2>
              <p>{query ? '试试其他名称、ID 或描述关键词。' : filter === 'installed' ? '无法自动检测的 Agent 不会计入已安装列表。' : '刷新列表，或更换筛选条件。'}</p>
              {(query || filter !== 'all') && <button type="button" className="button" onClick={resetFilters}>查看全部 Agent</button>}
            </div>
          )}
        </div>
      </div>
      <p className="agent-workspace-note"><Icon name="info" size={14} />安装状态来自本地检测，PATH 未检测到不代表设备上完全不存在。来源目录匹配不代表安装历史或由 AgentDeck 安装。安装仅安装软件；应用默认配置需单独确认，实例优先级有歧义时暂停。自动卸载功能尚未提供。</p>
    </div>
  );
};
