/** Local memory collection, search, and explicit file export. */
import React, { useEffect, useRef, useState } from 'react';
import { useAgentStore } from '../stores/agentStore';
import { useWorkspaceActivity } from '../stores/workspaceActivity';
import type { CollectionStatus, MemoryEntry } from '../bridge';
import { getMemoryCollectionStatus, runtimeMode, setMemoryCollectionEnabled } from '../bridge';
import { Icon } from './Icon';
import './MemoryMcp.css';

const AGENTS = [
  { id: 'codex', label: 'Codex' },
  { id: 'claude-code', label: 'Claude' },
  { id: 'opencode', label: 'OpenCode' },
] as const;

type Notice = { tone: 'success' | 'error' | 'warning'; text: string };

function agentLabel(id: string): string {
  return AGENTS.find((agent) => agent.id === id)?.label ?? id;
}

function formatTime(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  if (Number.isNaN(date.getTime())) return '时间未知';
  return date.toLocaleString('zh-CN', {
    year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit',
  });
}

export const MemoryPanel: React.FC = () => {
  const { collectMemories, searchMemories, injectMemories, toggleAutoInject } = useAgentStore();
  const [collection, setCollection] = useState<CollectionStatus | null>(null);
  const [collectionLoading, setCollectionLoading] = useState(true);
  const [collectionError, setCollectionError] = useState<string | null>(null);
  const [collectionReadAt, setCollectionReadAt] = useState<number | null>(null);
  const [query, setQuery] = useState('');
  const [submittedQuery, setSubmittedQuery] = useState<string | null>(null);
  const [failedQuery, setFailedQuery] = useState<string | null>(null);
  const [results, setResults] = useState<MemoryEntry[]>([]);
  // A synchronous guard also blocks rapid keyboard events before React renders.
  const operation = useRef(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const mode = runtimeMode();
  const readOnly = mode !== 'native';
  const canRead = mode !== 'unavailable';
  const writeDisabled = readOnly || busy !== null;

  useWorkspaceActivity('memory', busy !== null);

  useEffect(() => {
    let active = true;
    getMemoryCollectionStatus()
      .then((status) => { if (active) { setCollection(status); setCollectionReadAt(Date.now() / 1000); } })
      .catch((error) => { if (active) setCollectionError(`读取自动采集设置失败: ${error}`); })
      .finally(() => { if (active) setCollectionLoading(false); });
    return () => { active = false; };
  }, []);

  const retryCollection = async () => {
    if (operation.current || !canRead) return;
    operation.current = true;
    setBusy('settings');
    setCollectionLoading(true);
    setCollectionError(null);
    setNotice(null);
    try {
      setCollection(await getMemoryCollectionStatus());
      setCollectionReadAt(Date.now() / 1000);
    } catch (error) {
      setCollectionError(`读取自动采集设置失败: ${error}`);
    } finally {
      setCollectionLoading(false);
      operation.current = false;
      setBusy(null);
    }
  };

  const onAutoCollection = async (enabled: boolean) => {
    if (operation.current || !collection || collectionError || readOnly) return;
    if (enabled && !window.confirm('启用后将立即并每小时读取 Codex、Claude 与 OpenCode 本地会话，保存到本机记忆库。确认允许自动采集？')) return;
    operation.current = true;
    setBusy('auto-collection');
    setNotice(null);
    try {
      await setMemoryCollectionEnabled(enabled);
      // The persisted write is known, but worker timing/error fields need a fresh read.
      setCollection({ ...collection, enabled, lastRunAt: null, nextRunAt: null, lastError: null });
      setCollectionLoading(true);
      try {
        const status = await getMemoryCollectionStatus();
        setCollection(status);
        setCollectionReadAt(Date.now() / 1000);
        setCollectionError(null);
        setNotice({ tone: 'success', text: status.enabled ? '已启用自动采集，可随时关闭。' : '已关闭自动采集；已有记忆不会删除。' });
      } catch (error) {
        setCollectionError(`自动采集设置已保存，但读取最新状态失败: ${error}`);
        setNotice({ tone: 'warning', text: '当前自动采集状态与计划尚未确认，请刷新采集状态。' });
      } finally {
        setCollectionLoading(false);
      }
    } catch (error) {
      setNotice({ tone: 'error', text: `更新自动采集设置失败: ${error}` });
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onCollect = async (agentId: string) => {
    if (operation.current || readOnly) return;
    operation.current = true;
    setBusy(`collect-${agentId}`);
    setNotice(null);
    setFailedQuery(null);
    try {
      await collectMemories(agentId);
      try {
        setResults(await searchMemories(''));
        setQuery('');
        setSubmittedQuery('');
        setNotice({ tone: 'success', text: `已采集 ${agentLabel(agentId)} 的本地会话，并刷新最近记忆。` });
      } catch (error) {
        setFailedQuery('');
        setNotice({ tone: 'warning', text: `已采集 ${agentLabel(agentId)}，但记忆列表刷新失败: ${error}` });
      }
    } catch (error) {
      setNotice({ tone: 'error', text: `采集失败: ${error}` });
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onSearch = async (searchQuery = query) => {
    if (operation.current || !canRead) return;
    operation.current = true;
    setBusy('search');
    setQuery(searchQuery);
    setNotice(null);
    setFailedQuery(null);
    try {
      setResults(await searchMemories(searchQuery));
      setSubmittedQuery(searchQuery);
    } catch (error) {
      setFailedQuery(searchQuery);
      setNotice({ tone: 'error', text: `搜索失败: ${error}` });
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onToggleAuto = async (entry: MemoryEntry, enabled: boolean) => {
    if (operation.current || readOnly) return;
    operation.current = true;
    setBusy(`toggle-${entry.id}`);
    setNotice(null);
    setFailedQuery(null);
    const previous = entry.autoInject;
    setResults((entries) => entries.map((item) => item.id === entry.id ? { ...item, autoInject: enabled } : item));
    try {
      await toggleAutoInject(entry.id, enabled);
      setNotice(enabled
        ? { tone: 'success', text: '已开启此条记忆的自动导出，并更新导出文件。' }
        : { tone: 'warning', text: '已关闭此条记忆的自动导出。若此前已手动选择导出，此条仍会保留在导出文件中。' });
    } catch (error) {
      setResults((entries) => entries.map((item) => item.id === entry.id ? { ...item, autoInject: previous } : item));
      setNotice({ tone: 'error', text: `更新自动导出失败，已恢复原设置: ${error}` });
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onExport = async (entry: MemoryEntry) => {
    if (operation.current || readOnly) return;
    operation.current = true;
    setBusy(`export-${entry.id}`);
    setNotice(null);
    setFailedQuery(null);
    try {
      await injectMemories(entry.agentId, [entry.id]);
      setNotice({ tone: 'success', text: `已导出到 ~/.agentdeck/memory/${entry.agentId}.md（不代表 Agent 已读取）。本次手动选择已替换该 Agent 的上次手动选择，自动导出记忆仍会保留。` });
    } catch (error) {
      setNotice({ tone: 'error', text: `导出失败: ${error}` });
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  return (
    <div className="memory-workspace">
      <header className="workspace-heading">
        <div>
          <p className="eyebrow">LOCAL MEMORY</p>
          <h1>记忆管理</h1>
          <p className="muted">采集本机会话，检索上下文，再按需导出为 Agent 可读取的文件。</p>
        </div>
        <span className="status-badge"><Icon name="memory" size={15} />本地记忆库</span>
      </header>

      <div className="memory-overview">
        <section className="panel memory-collection" aria-labelledby="memory-collection-title">
          <div className="panel-heading">
            <h2 id="memory-collection-title"><Icon name="download" />会话采集</h2>
            <div className="toolbar memory-status-actions">
              <span className="status-badge">{collectionLoading ? '读取设置中' : collectionError && collection ? '状态待刷新' : collection ? (collection.enabled ? '自动采集已开启' : '自动采集已关闭') : '设置未读取'}</span>
              <button className="button" onClick={() => void retryCollection()} disabled={busy !== null || collectionLoading || !canRead}><Icon name="refresh" size={14} />刷新采集状态</button>
            </div>
          </div>
          <p className="muted">读取所选 Agent 的本地会话并保存到本机。请勿采集包含敏感信息的会话。</p>
          <div className="toolbar memory-collect-actions" aria-label="手动采集来源">
            {AGENTS.map((agent) => (
              <button className="button" key={agent.id} onClick={() => void onCollect(agent.id)} disabled={writeDisabled}>
                <Icon name={busy === `collect-${agent.id}` ? 'refresh' : 'plus'} className={busy === `collect-${agent.id}` ? 'memory-spinning' : undefined} size={15} />
                {busy === `collect-${agent.id}` ? `正在采集 ${agent.label}` : `采集 ${agent.label}`}
              </button>
            ))}
          </div>
          <div className="memory-auto-collection">
            <label className="check-row">
              <input type="checkbox" checked={collection?.enabled ?? false} disabled={collection === null || collectionError !== null || writeDisabled} onChange={(event) => void onAutoCollection(event.target.checked)} aria-describedby="memory-collection-help" />
              <span>允许每小时自动采集本地会话</span>
            </label>
            <p id="memory-collection-help" className="muted">默认关闭。开启需确认授权；上方按钮仅执行一次手动采集。</p>
            {!collectionError && collection?.lastRunAt != null && <p className="memory-schedule muted">上次自动采集：{formatTime(collection.lastRunAt)}</p>}
            {!collectionError && collection?.enabled && collection.nextRunAt != null && <p className="memory-schedule muted">下次计划采集：{formatTime(collection.nextRunAt)}</p>}
          </div>
          {collectionReadAt !== null && !collectionError && <p className="memory-status-snapshot muted">状态读取于 {formatTime(collectionReadAt)}，可手动刷新获取最新计划。</p>}
          {collectionError && collection && <p className="memory-status-stale muted">当前状态未确认，已隐藏旧的运行计划与错误记录；刷新成功前暂停修改自动采集设置。</p>}
          {collectionError && (
            <div className="notice notice-error memory-inline-notice" role="alert">
              <Icon name="alert" /><span>{collectionError}</span>
              <button className="button" onClick={() => void retryCollection()} disabled={busy !== null || !canRead}>重试读取设置</button>
            </div>
          )}
          {!collectionError && collection?.lastError && <div className="notice notice-warning memory-inline-notice" role="status"><Icon name="alert" /><span>最近自动采集失败：{collection.lastError}</span></div>}
        </section>

        <aside className="panel memory-export-guide" aria-labelledby="memory-export-title">
          <div className="panel-heading"><h2 id="memory-export-title"><Icon name="external" />文件导出</h2></div>
          <p>采集不会让 Agent 自动读取记忆。导出仅维护本机 Markdown 文件，仍需自行让 Agent 读取。</p>
          <code className="memory-export-path">~/.agentdeck/memory/</code>
          <dl className="memory-export-rules">
            <div><dt>导出此条</dt><dd>替换该 Agent 上次手动选择的记忆，保留其自动导出记忆。</dd></div>
            <div><dt>自动导出</dt><dd>开关会更新导出文件。关闭后移除自动选择，已手动选择的记忆仍保留。</dd></div>
          </dl>
        </aside>
      </div>

      <section className="panel memory-library" aria-labelledby="memory-library-title">
        <div className="panel-heading">
          <h2 id="memory-library-title"><Icon name="search" />检索记忆</h2>
          {submittedQuery !== null && <span className="status-badge">本次查询 {results.length} 条</span>}
        </div>
        <form className="memory-search" onSubmit={(event) => { event.preventDefault(); void onSearch(); }}>
          <div className="field memory-search-field">
            <label className="field-label" htmlFor="memory-search-input">搜索本地记忆</label>
            <input id="memory-search-input" className="control" value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter' && !event.nativeEvent.isComposing) { event.preventDefault(); void onSearch(); } }} placeholder="全文搜索记忆（FTS5）" disabled={!canRead} />
          </div>
          <button className="button button-primary" type="submit" disabled={busy !== null || !canRead}>
            <Icon name={busy === 'search' ? 'refresh' : 'search'} className={busy === 'search' ? 'memory-spinning' : undefined} size={16} />{busy === 'search' ? '搜索中…' : '搜索'}
          </button>
          <button className="button" type="button" onClick={() => void onSearch('')} disabled={busy !== null || !canRead}>最近记忆</button>
        </form>
        <p className="memory-search-help muted">支持全文检索；留空搜索可查看最近记忆。每次最多返回 50 条。</p>

        {notice && (
          <div className={`notice notice-${notice.tone} memory-inline-notice`} role={notice.tone === 'error' ? 'alert' : 'status'}>
            <Icon name={notice.tone === 'success' ? 'check' : 'alert'} />
            <span>{notice.text}</span>
            {failedQuery !== null && <button className="button" onClick={() => void onSearch(failedQuery)} disabled={busy !== null || !canRead}>重试查询</button>}
          </div>
        )}

        {busy === 'search' && <p className="memory-loading muted" role="status"><Icon name="refresh" className="memory-spinning" />正在检索本地记忆…</p>}
        {results.length === 0 ? (
          <div className="empty-state memory-empty">
            <Icon name={submittedQuery?.trim() ? 'search' : 'memory'} size={30} />
            <h3>{submittedQuery === null ? '从本地记忆开始' : submittedQuery.trim() ? '没有匹配的记忆' : '记忆库暂无内容'}</h3>
            <p>{submittedQuery === null ? '查看最近记忆，或输入关键词搜索已有内容。需要新内容时，可在上方手动采集。' : submittedQuery.trim() ? `未找到与“${submittedQuery}”匹配的内容。请尝试其他关键词，或查看最近记忆。` : '可在上方选择 Agent，采集其本地会话。采集完成后，记忆会显示在这里。'}</p>
            {!canRead && <p>连接桌面后端后即可读取本地记忆。</p>}
          </div>
        ) : (
          <>
            <p className="memory-results-caption muted">{submittedQuery?.trim() ? `“${submittedQuery}”的搜索结果` : '最近记忆'}{failedQuery !== null ? ' · 以下保留上次成功查询的结果' : ''}</p>
            <ul className="memory-entries" aria-label="记忆查询结果">
              {results.map((entry) => (
                <li key={entry.id}>
                  <article className="memory-entry" aria-label={`${agentLabel(entry.agentId)} 记忆 ${entry.id}`}>
                    <div className="memory-entry-meta">
                      <span className="status-badge">{agentLabel(entry.agentId)}</span>
                      <span className="memory-entry-id muted">#{entry.id}</span>
                      <time dateTime={Number.isNaN(new Date(entry.collectedAt * 1000).getTime()) ? undefined : new Date(entry.collectedAt * 1000).toISOString()}>{formatTime(entry.collectedAt)}</time>
                    </div>
                    <p className="memory-entry-content">{entry.content}</p>
                    <div className="memory-entry-footer">
                      <label className="check-row">
                        <input type="checkbox" aria-label={`${agentLabel(entry.agentId)} 记忆 ${entry.id} 的自动导出`} disabled={writeDisabled} checked={entry.autoInject} onChange={(event) => void onToggleAuto(entry, event.target.checked)} />
                        <span>{busy === `toggle-${entry.id}` ? '更新导出设置中…' : '自动导出'}</span>
                      </label>
                      <button className="button" aria-label={`导出 ${agentLabel(entry.agentId)} 记忆 ${entry.id}`} onClick={() => void onExport(entry)} disabled={writeDisabled}>
                        <Icon name={busy === `export-${entry.id}` ? 'refresh' : 'external'} className={busy === `export-${entry.id}` ? 'memory-spinning' : undefined} size={15} />
                        {busy === `export-${entry.id}` ? '导出中…' : '导出此条'}
                      </button>
                    </div>
                  </article>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
    </div>
  );
};
