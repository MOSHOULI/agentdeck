import React, { useEffect, useMemo, useState } from 'react';
import { useAgentStore } from '../stores/agentStore';
import { AgentCard } from '../components/AgentCard';
import { Icon } from '../components/Icon';
import { runtimeMode } from '../bridge';
import { matchesInstalledFilter, matchesSupportFilter, type SupportFilter } from './catalogFilters';
import './AgentWorkspace.css';

const filters = [
  { id: 'all', label: '全部' },
  { id: 'installed', label: '已安装' },
] as const;

export const AgentSquare: React.FC<{ onOpenDshInventory?: () => void }> = ({ onOpenDshInventory }) => {
  const { agents, loading, error, notice, refreshError, dataStale, selectedAgentId, loadAgents, installAgent, configureAgent, uninstallAgent, selectAgent, clearError } = useAgentStore();
  const [filter, setFilter] = useState<'all' | 'installed'>('all');
  const [query, setQuery] = useState('');
  const [supportFilter, setSupportFilter] = useState<SupportFilter>('all');
  const native = runtimeMode() === 'native';

  useEffect(() => { void loadAgents(); }, [loadAgents]);

  const filteredAgents = useMemo(() => {
    const search = query.trim().toLocaleLowerCase();
    return [...agents]
      // Legacy popularity/star fields are not verified recommendations or ranking signals.
      .sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id))
      .filter((agent) => {
        if (filter === 'installed' && !matchesInstalledFilter(agent)) return false;
        if (!matchesSupportFilter(agent, supportFilter, { native, loading, dataStale })) return false;
        return !search || [agent.name, agent.id, agent.description, agent.uiType, agent.category]
          .some((value) => value.toLocaleLowerCase().includes(search));
      });
  }, [agents, filter, query, supportFilter, native, loading, dataStale]);

  const resetFilters = () => { setQuery(''); setFilter('all'); setSupportFilter('all'); };
  const supportHint = supportFilter === 'automatic'
    ? !native ? '当前为只读模式。请在桌面应用中刷新检测后查看可自动安装项。'
      : loading || dataStale ? '当前检测状态待刷新，可自动安装列表暂不展示候选。'
      : '仅显示本次检测未发现、支持自动安装且检测证据无冲突的 Agent；安装仍需单独确认。'
    : supportFilter === 'manual' ? '仅表示目录未提供自动安装能力，不代表未安装；请查看条目详情及官方说明。'
      : supportFilter === 'attention' ? '包含检测出错、无法自动验证、证据冲突或待刷新的条目，不等于软件损坏。'
        : '按安装能力或检测状态筛选，可与已安装和搜索组合使用。';
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
          <div className="agent-support-filter">
            <label htmlFor="agent-support-filter">支持能力与状态</label>
            <select id="agent-support-filter" className="control" value={supportFilter} aria-describedby="agent-support-hint" onChange={(event) => setSupportFilter(event.target.value as SupportFilter)}>
              <option value="all">全部能力与状态</option>
              <option value="automatic">当前可自动安装</option>
              <option value="manual">仅手动安装</option>
              <option value="attention">待确认状态</option>
            </select>
          </div>
          <div className="agent-search">
            <Icon name="search" size={16} />
            <label htmlFor="agent-search" className="sr-only">搜索 Agent</label>
            <input id="agent-search" className="control" type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索名称、ID 或描述…" />
          </div>
        </div>

        <p id="agent-support-hint" className="agent-support-hint">{supportHint}</p>

        <div className="agent-list-summary">
          <span>{loading ? <span role="status">正在读取 Agent 状态…</span> : refreshError && agents.length === 0 ? '列表暂不可用' : `显示 ${filteredAgents.length} / ${agents.length} 款 Agent`}</span>
          <span>按名称排序</span>
          <span>{selectedAgent ? `当前选择：${selectedAgent.name}` : '尚未选择 Agent'}</span>
        </div>

        <div className="agent-list" aria-label="Agent 列表" aria-busy={loading}>
          {filteredAgents.map((agent) => <AgentCard key={agent.id} agent={agent} busy={loading} dataStale={dataStale} selected={selectedAgentId === agent.id} onInstall={installAgent} onConfigure={configureAgent} onUninstall={uninstallAgent} onSelect={selectAgent} onOpenDshInventory={onOpenDshInventory} />)}
          {loading && agents.length === 0 && <div className="empty-state agent-empty"><Icon name="agents" size={28} /><h2>正在加载 Agent</h2><p>读取注册表与本地安装状态。</p></div>}
          {!loading && refreshError && agents.length === 0 && <div className="empty-state agent-empty"><Icon name="alert" size={28} /><h2>暂时无法读取 Agent 列表</h2><p>检查桌面后端连接后重试。</p><button type="button" className="button" onClick={() => { void loadAgents(); }}><Icon name="refresh" size={15} />重新加载</button></div>}
          {!loading && (!refreshError || agents.length > 0) && filteredAgents.length === 0 && (
            <div className="empty-state agent-empty">
              <Icon name={query ? 'search' : 'agents'} size={28} />
              <h2>{query ? '没有匹配的 Agent' : supportFilter !== 'all' ? '没有符合当前筛选条件的 Agent' : filter === 'installed' ? '还没有检测到已安装的 Agent' : '暂无可显示的 Agent'}</h2>
              <p>{query ? '试试其他名称、ID 或描述关键词。' : supportFilter !== 'all' ? '当前能力、安装状态和搜索条件需要同时满足；可查看全部或刷新检测。' : filter === 'installed' ? '无法自动检测的 Agent 不会计入已安装列表。' : '刷新列表，或更换筛选条件。'}</p>
              {(query || filter !== 'all' || supportFilter !== 'all') && <button type="button" className="button" onClick={resetFilters}>查看全部 Agent</button>}
            </div>
          )}
        </div>
      </div>
      <p className="agent-workspace-note"><Icon name="info" size={14} />安装状态来自本地检测，PATH 未检测到不代表设备上完全不存在。来源目录匹配不代表安装历史或由 AgentDeck 安装。安装仅安装软件；应用默认配置需单独确认，实例优先级有歧义时暂停。自动卸载功能尚未提供。</p>
    </div>
  );
};
