/** Registry-backed MCP configuration. Checkboxes describe configuration, not process health. */
import React, { useEffect, useRef, useState } from 'react';
import type { McpServer } from '../bridge';
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
  const [busy, setBusy] = useState<string | null>('load');
  const [loadError, setLoadError] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const mode = bridge.runtimeMode();
  const readOnly = mode !== 'native';
  const canRead = mode !== 'unavailable';

  useWorkspaceActivity('mcp', busy !== null);

  useEffect(() => {
    let active = true;
    operation.current = true;
    bridge.getMcpServers()
      .then((items) => { if (active) { setServers(items); setLoadError(false); } })
      .catch((error) => { if (active) { setLoadError(true); setNotice({ tone: 'error', text: `加载 MCP 列表失败: ${error}` }); } })
      .finally(() => { if (active) { operation.current = false; setBusy(null); } });
    return () => { active = false; };
  }, []);

  const refresh = async () => {
    if (operation.current || !canRead) return;
    operation.current = true;
    setBusy('load');
    setNotice(null);
    try {
      setServers(await bridge.getMcpServers());
      setLoadError(false);
    } catch (error) {
      setLoadError(true);
      setNotice({ tone: 'error', text: `加载 MCP 列表失败: ${error}` });
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onToggle = async (server: McpServer, agentId: string, enabled: boolean) => {
    if (operation.current || readOnly || loadError || !server.supportedAgents.includes(agentId)) return;
    operation.current = true;
    const previous = server.enabledAgents;
    setBusy(`${server.name}-${agentId}`);
    setNotice(null);
    setServers((items) => items?.map((item) => item.name === server.name ? {
      ...item,
      enabledAgents: enabled ? [...new Set([...item.enabledAgents, agentId])] : item.enabledAgents.filter((id) => id !== agentId),
    } : item) ?? null);
    try {
      await bridge.toggleMcpForAgent(server.name, agentId, enabled);
      const label = agents.find((agent) => agent.id === agentId)?.name ?? agentId;
      setNotice({ tone: 'success', text: enabled ? `已将 ${server.name} 加入 ${label} 配置。配置状态不代表服务已启动或连接成功。` : `已从 ${label} 配置中移除 ${server.name}。此操作不会停止服务进程。` });
    } catch (error) {
      setServers((items) => items?.map((item) => item.name === server.name ? { ...item, enabledAgents: previous } : item) ?? null);
      setNotice({ tone: 'error', text: `切换失败，已恢复原配置选项: ${error}` });
    } finally {
      operation.current = false;
      setBusy(null);
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
        <div><strong>开关代表配置包含状态</strong><p>更改会写回对应 Agent 配置文件，写入前自动备份。此处不检测服务器是否在线，也不负责启动或停止进程。</p></div>
      </div>

      {notice && <div className={`notice notice-${notice.tone} mcp-feedback`} role={notice.tone === 'error' ? 'alert' : 'status'}><Icon name={notice.tone === 'success' ? 'check' : 'alert'} /><span>{notice.text}</span>{loadError && <button className="button" onClick={() => void refresh()} disabled={busy !== null || !canRead}>重试加载</button>}</div>}

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
                    const pending = busy === `${server.name}-${agentId}`;
                    return (
                      <label className="mcp-agent-row" key={agentId}>
                        <input type="checkbox" aria-label={`${server.name} 的 ${label} 配置`} checked={enabled} onChange={(event) => void onToggle(server, agentId, event.target.checked)} />
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
