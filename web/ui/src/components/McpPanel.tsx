/**
 * MCP 管理面板（M5）。
 * 展示内置注册表的 MCP 服务器，按 agent 独立开关（实时写回配置文件）。
 * 开关 agent 列表来自后端 supportedAgents（registry-v3.json config_inject.mcp）。
 */

import React, { useEffect, useState, useRef } from 'react';
import type { McpServer } from '../bridge';
import * as bridge from '../bridge';
import { useAgentStore } from '../stores/agentStore';

export const McpPanel: React.FC = () => {
  const agents = useAgentStore((s) => s.agents);
  const [servers, setServers] = useState<McpServer[] | null>(null);
  const operation = useRef(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = async () => {
    setServers(await bridge.getMcpServers());
  };

  useEffect(() => {
    refresh().catch((e) => { setServers([]); setNotice(`加载 MCP 列表失败: ${e}`); });
  }, []);

  const onToggle = async (server: McpServer, agentId: string, enabled: boolean) => {
    if (operation.current) return;
    const previous = server.enabledAgents;
    setServers((prev) =>
      prev
        ? prev.map((s) =>
            s.name === server.name
              ? {
                  ...s,
                  enabledAgents: enabled
                    ? [...new Set([...s.enabledAgents, agentId])]
                    : s.enabledAgents.filter((a) => a !== agentId),
                }
              : s
          )
        : prev
    );
    operation.current = true;
    setBusy(`${server.name}-${agentId}`);
    setNotice(null);
    try {
      await bridge.toggleMcpForAgent(server.name, agentId, enabled);
      setNotice(`${enabled ? '已启用' : '已停用'} ${server.name} → ${agentId}`);
    } catch (e) {
      setServers((prev) =>
        prev
          ? prev.map((s) => (s.name === server.name ? { ...s, enabledAgents: previous } : s))
          : prev
      );
      setNotice(`切换失败: ${e}`);
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  return (
    <div
      style={{
        marginTop: 32,
        padding: 20,
        borderRadius: 12,
        border: '1px solid #374151',
        background: '#111827',
      }}
    >
      <h2 style={{ fontSize: 18, fontWeight: 600, color: '#e5e7eb', margin: '0 0 12px 0' }}>
        🔌 MCP 服务器
      </h2>
      <p style={{ fontSize: 13, color: '#6b7280', margin: '0 0 14px 0' }}>
        开关实时写回各 agent 配置文件（写前自动 git 备份）。
      </p>

      {notice && (
        <div role="status" style={{ fontSize: 13, color: '#6ee7b7', marginBottom: 10 }}>{notice}</div>
      )}

      {servers === null ? (
        <p style={{ color: '#6b7280', fontSize: 13 }}>加载中...</p>
      ) : servers.length === 0 ? (
        <p style={{ color: '#6b7280', fontSize: 13 }}>暂无可用 MCP 服务器。</p>
      ) : (
        <ul style={{ listStyle: 'none', margin: 0, padding: 0 }}>
          {servers.map((server) => (
            <li
              key={server.name}
              style={{
                padding: '12px 14px',
                marginBottom: 10,
                borderRadius: 8,
                background: '#1f2937',
                border: '1px solid #374151',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 4 }}>
                <span style={{ fontSize: 14, fontWeight: 600, color: '#e5e7eb' }}>
                  {server.name}
                </span>
                <span
                  style={{
                    fontSize: 11,
                    padding: '2px 8px',
                    borderRadius: 99,
                    background: server.kind === 'http' ? '#1e3a8a' : '#374151',
                    color: server.kind === 'http' ? '#93c5fd' : '#9ca3af',
                  }}
                >
                  {server.kind}
                </span>
              </div>
              <p style={{ fontSize: 12, color: '#9ca3af', margin: '0 0 8px 0' }}>
                {server.description}
                {server.url ? `（${server.url}）` : server.command ? `（${server.command}）` : ''}
              </p>
              <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap' }}>
                {server.supportedAgents.map((agentId) => {
                  const label =
                    agents.find((a) => a.id === agentId)?.name ?? agentId;
                  const enabled = server.enabledAgents.includes(agentId);
                  const isBusy = busy !== null;
                  return (
                    <label
                      key={agentId}
                      style={{ fontSize: 12, color: '#9ca3af', display: 'flex', gap: 6 }}
                    >
                      <input
                        type="checkbox"
                        checked={enabled}
                        disabled={isBusy}
                        onChange={(e) => onToggle(server, agentId, e.target.checked)}
                      />
                      {label}
                    </label>
                  );
                })}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};
