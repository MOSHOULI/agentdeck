/** Registry-backed MCP configuration. Checkboxes describe configuration, not process health. */
import React, { useEffect, useRef, useState } from 'react';
import type { McpServer, McpTogglePreview } from '../bridge';
import * as bridge from '../bridge';
import { useAgentStore } from '../stores/agentStore';
import { useWorkspaceActivity } from '../stores/workspaceActivity';
import { Icon } from './Icon';
import './MemoryMcp.css';

type Notice = { tone: 'success' | 'error' | 'warning'; text: string };

export const McpPanel: React.FC = () => {
  const agents = useAgentStore((state) => state.agents);
  const [servers, setServers] = useState<McpServer[] | null>(null);
  const operation = useRef(false);
  const generation = useRef(0);
  const applying = useRef(false);
  const returnFocus = useRef<HTMLInputElement | null>(null);
  const previewHeading = useRef<HTMLHeadingElement | null>(null);
  const [preview, setPreview] = useState<McpTogglePreview | null>(null);
  const [busy, setBusy] = useState<string | null>('load');
  const [loadError, setLoadError] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const mode = bridge.runtimeMode();
  const readOnly = mode !== 'native';
  const canRead = mode !== 'unavailable';

  useWorkspaceActivity('mcp', busy !== null);

  useEffect(() => {
    let active = true;
    const request = ++generation.current;
    operation.current = true;
    bridge.getMcpServers()
      .then((items) => { if (active && request === generation.current) { setServers(items); setLoadError(false); } })
      .catch((error) => { if (active && request === generation.current) { setLoadError(true); setNotice({ tone: 'error', text: `加载 MCP 列表失败: ${error}` }); } })
      .finally(() => { if (active && request === generation.current) { operation.current = false; setBusy(null); } });
    return () => { active = false; ++generation.current; };
  }, []);

  useEffect(() => { if (preview) previewHeading.current?.focus(); }, [preview]);

  const refresh = async () => {
    if (operation.current || !canRead) return;
    const request = ++generation.current;
    operation.current = true;
    setBusy('load');
    setNotice(null);
    try {
      const items = await bridge.getMcpServers();
      if (request !== generation.current) return;
      setServers(items);
      setLoadError(false);
    } catch (error) {
      if (request !== generation.current) return;
      setLoadError(true);
      setNotice({ tone: 'error', text: `加载 MCP 列表失败: ${error}` });
    } finally {
      if (request === generation.current) { operation.current = false; setBusy(null); }
    }
  };

  const cancelPreview = () => {
    if (applying.current) return;
    ++generation.current;
    operation.current = false;
    setPreview(null);
    setBusy(null);
    // Wait for the fieldset to become enabled before restoring keyboard focus.
    window.setTimeout(() => returnFocus.current?.focus(), 0);
  };

  const onToggle = async (server: McpServer, agentId: string, enabled: boolean, input: HTMLInputElement) => {
    if (operation.current || readOnly || loadError || !server.supportedAgents.includes(agentId)) return;
    const request = ++generation.current;
    operation.current = true;
    returnFocus.current = input;
    setBusy('preview');
    setNotice(null);
    try {
      const result = await bridge.previewMcpToggle(server.name, agentId, enabled);
      if (request !== generation.current) return;
      if (result.agentId !== agentId || result.serverName !== server.name || result.enabled !== enabled
          || typeof result.format !== 'string' || typeof result.revision !== 'string' || !result.revision
          || typeof result.configPath !== 'string' || !result.configPath || typeof result.changed !== 'boolean' || typeof result.fileExists !== 'boolean') {
        throw new Error('预览与请求不一致，请重新预览。');
      }
      setPreview(result);
      setBusy('confirm');
    } catch (error) {
      if (request !== generation.current) return;
      operation.current = false;
      setBusy(null);
      setNotice({ tone: 'error', text: `预览失败，未提交配置修改: ${error}` });
    }
  };

  const confirmPreview = async () => {
    if (!preview || applying.current || busy !== 'confirm' || readOnly) return;
    const confirmed = preview;
    const request = ++generation.current;
    applying.current = true;
    setBusy('apply');
    setNotice(null);
    try {
      await bridge.applyMcpTogglePreview(confirmed.serverName, confirmed.agentId, confirmed.enabled, confirmed.revision);
      if (request !== generation.current) return;
      setPreview(null);
      const label = agents.find((agent) => agent.id === confirmed.agentId)?.name ?? confirmed.agentId;
      // Read the current configuration after a confirmed write; do not turn a
      // successful write into a failure claim when only the follow-up read fails.
      try {
        const items = await bridge.getMcpServers();
        if (request !== generation.current) return;
        setServers(items);
        setLoadError(false);
        setNotice({ tone: 'success', text: confirmed.enabled ? `已将 ${confirmed.serverName} 加入 ${label} 配置。配置状态不代表服务已启动或连接成功。` : `已从 ${label} 配置中移除 ${confirmed.serverName}。此操作不会停止服务进程。` });
      } catch (error) {
        if (request !== generation.current) return;
        setLoadError(true);
        setNotice({ tone: 'warning', text: `配置修改已完成，但重新读取失败。以下为旧状态，请刷新确认: ${error}` });
      }
    } catch (error) {
      if (request !== generation.current) return;
      setPreview(null);
      setLoadError(true);
      setNotice({ tone: 'error', text: `应用未完成，请刷新配置并重新预览。不会自动重试写入: ${error}` });
    } finally {
      if (request === generation.current) {
        applying.current = false;
        operation.current = false;
        setBusy(null);
      }
    }
  };

  return (
    <div className="mcp-workspace">
      <header className="workspace-heading">
        <div>
          <p className="eyebrow">MODEL CONTEXT PROTOCOL</p>
          <h1>MCP 服务器</h1>
          <p className="muted">为支持的 Agent 管理 MCP 配置，统一连接外部工具与上下文。</p>
        </div>
        <button className="button" disabled={busy !== null || !canRead} onClick={() => void refresh()}><Icon name="refresh" className={busy === 'load' ? 'mcp-spinning' : undefined} size={16} />{busy === 'load' ? '读取配置中…' : '刷新配置'}</button>
      </header>

      <div className="notice mcp-config-notice">
        <Icon name="info" />
        <div><strong>开关代表配置包含状态</strong><p>先预览目标与操作，确认后才写回对应 Agent 配置文件，写入前自动备份。此处不检测服务器是否在线，也不负责启动或停止进程。</p></div>
      </div>

      {notice && <div className={`notice notice-${notice.tone} mcp-feedback`} role={notice.tone === 'error' ? 'alert' : 'status'}><Icon name={notice.tone === 'success' ? 'check' : 'alert'} /><span>{notice.text}</span>{loadError && <button className="button" onClick={() => void refresh()} disabled={busy !== null || !canRead}>重试加载</button>}</div>}

      {(busy === 'preview' || preview) && <section className="panel mcp-change-preview" aria-labelledby="mcp-preview-title" aria-busy={busy === 'preview' || busy === 'apply'}>
        <h2 id="mcp-preview-title" ref={previewHeading} tabIndex={-1}>配置变更预览</h2>
        {preview ? <>
          <dl className="mcp-preview-details">
            <div><dt>Agent / 服务</dt><dd>{agents.find((agent) => agent.id === preview.agentId)?.name ?? preview.agentId} / {preview.serverName}</dd></div>
            <div><dt>目标文件</dt><dd><code>{preview.configPath}</code></dd></div>
            <div><dt>配置格式</dt><dd>{preview.format}</dd></div>
            <div><dt>操作</dt><dd>{preview.enabled ? '加入或更新此服务配置' : '移除此服务配置'}</dd></div>
            <div><dt>文件状态</dt><dd>{preview.fileExists ? '现有配置文件' : '配置文件尚不存在'}</dd></div>
            <div><dt>预计变化</dt><dd>{preview.changed ? '配置内容将发生变化' : '配置内容无变化'}</dd></div>
          </dl>
          <p className="muted">预览不展示配置内容或密钥。启用会按注册表定义新增或替换该同名服务器配置，自定义字段可能被替换。JSON、JSONC、YAML 的格式或注释可能变化。确认时会重新核对文件与变更；预览失效时不会自动重试。确认仍会执行备份，包括内容无变化的请求。</p>
        </> : <p role="status">正在读取变更预览，尚未提交写入…</p>}
        <div className="mcp-preview-actions">
          <button className="button" disabled={busy === 'apply'} onClick={cancelPreview}>取消</button>
          {preview && <button className="button button-primary" disabled={busy !== 'confirm'} onClick={() => void confirmPreview()}>{busy === 'apply' ? '正在应用…' : '确认并应用'}</button>}
        </div>
      </section>}

      {servers === null ? (
        <div className="panel empty-state mcp-empty" role={busy === 'load' ? 'status' : undefined}>
          <Icon name={busy === 'load' ? 'refresh' : 'alert'} size={30} className={busy === 'load' ? 'mcp-spinning' : undefined} />
          <h2>{busy === 'load' ? '正在读取 MCP 配置' : '暂时无法读取 MCP 配置'}</h2>
          <p>{busy === 'load' ? '从本地注册表读取可用服务与 Agent 配置。' : canRead ? '请检查错误详情后重试。未读取到配置前，不会显示任何已启用状态。' : '请在 AgentDeck 桌面应用中读取和管理本地配置。'}</p>
        </div>
      ) : servers.length === 0 ? (
        <div className="panel empty-state mcp-empty"><Icon name="mcp" size={30} /><h2>暂无可用 MCP 服务器</h2><p>{loadError ? '上次读取的注册表中没有 MCP 服务，请重试以确认当前状态。' : '当前注册表中没有 MCP 服务。更新注册表后，可刷新配置重新读取。'}</p></div>
      ) : (
        <section aria-labelledby="mcp-services-title">
          <div className="mcp-list-heading"><h2 id="mcp-services-title">注册表服务 <span className="muted">{servers.length}</span></h2><span className="muted">仅显示注册表支持的 Agent</span></div>
          {loadError && <p className="muted mcp-stale-warning">以下保留上次成功读取的配置；刷新成功前已暂停修改。</p>}
          <ul className="mcp-server-grid">
            {servers.map((server) => (
              <li key={server.name} className="panel mcp-server">
                <div className="mcp-server-heading"><div className="mcp-server-mark"><Icon name="mcp" size={21} /></div><h3>{server.name}</h3><span className="status-badge">{server.kind.toUpperCase()}</span></div>
                <p className="mcp-server-description muted">{server.description}</p>
                {(server.url || server.command) && <div className="mcp-endpoint"><span className="field-label">{server.kind === 'http' ? '服务地址' : '启动命令'}</span><code>{server.url ?? [server.command, ...server.args].filter(Boolean).join(' ')}</code></div>}
                <fieldset className="mcp-agent-config" disabled={busy !== null || readOnly || loadError}>
                  <legend>包含此服务的 Agent 配置</legend>
                  {server.supportedAgents.length === 0 ? <p className="muted">此服务尚无受支持的 Agent 配置目标。</p> : server.supportedAgents.map((agentId) => {
                    const label = agents.find((agent) => agent.id === agentId)?.name ?? agentId;
                    const enabled = server.enabledAgents.includes(agentId);
                    const pending = busy === 'apply' && preview?.serverName === server.name && preview.agentId === agentId;
                    return (
                      <label className="mcp-agent-row" key={agentId}>
                        <input type="checkbox" aria-label={`${server.name} 的 ${label} 配置`} checked={enabled} onChange={(event) => void onToggle(server, agentId, event.target.checked, event.currentTarget)} />
                        <span>{label}</span>
                        <span className={`mcp-config-state${enabled ? ' mcp-config-included' : ''}`}><Icon name={pending ? 'refresh' : enabled ? 'check' : 'x'} className={pending ? 'mcp-spinning' : undefined} size={14} />{pending ? '正在写入' : enabled ? '已包含' : '未包含'}</span>
                      </label>
                    );
                  })}
                </fieldset>
                <p className="mcp-server-footer muted"><Icon name="info" size={14} />{readOnly ? '只读视图，配置修改不可用' : '修改配置不会启动或停止服务'}</p>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
};
