/**
 * 记忆管理面板（M3）。
 * 采集 Codex/Claude/OpenCode 会话 → FTS5 搜索 → 注入记忆文件。
 */

import React, { useState } from 'react';
import { useAgentStore } from '../stores/agentStore';
import type { MemoryEntry } from '../bridge';

const AGENTS = [
  { id: 'codex', label: 'Codex' },
  { id: 'claude-code', label: 'Claude' },
  { id: 'opencode', label: 'OpenCode' },
] as const;

function formatTime(ts: number): string {
  const d = new Date(ts * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
    d.getDate()
  ).padStart(2, '0')} ${String(d.getHours()).padStart(2, '0')}:${String(
    d.getMinutes()
  ).padStart(2, '0')}`;
}

export const MemoryPanel: React.FC = () => {
  const { collectMemories, searchMemories, injectMemories, toggleAutoInject } = useAgentStore();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<MemoryEntry[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refreshAll = async () => {
    const entries = await searchMemories('');
    setResults(entries);
  };

  const onCollect = async (agentId: string) => {
    setBusy(agentId);
    setNotice(null);
    try {
      await collectMemories(agentId);
      await refreshAll();
      setNotice(`已采集 ${agentId} 的记忆`);
    } catch (e) {
      setNotice(`采集失败: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const onSearch = async () => {
    setBusy('search');
    setNotice(null);
    try {
      setResults(await searchMemories(query));
    } finally {
      setBusy(null);
    }
  };

  const onToggleAuto = async (entry: MemoryEntry, enabled: boolean) => {
    const previous = entry.autoInject;
    setResults((prev) =>
      prev.map((r) => (r.id === entry.id ? { ...r, autoInject: enabled } : r))
    );
    try {
      await toggleAutoInject(entry.id, enabled);
    } catch (e) {
      setResults((prev) =>
        prev.map((r) => (r.id === entry.id ? { ...r, autoInject: previous } : r))
      );
      setNotice(`切换自动注入失败: ${e}`);
    }
  };

  const onInject = async (entry: MemoryEntry) => {
    setBusy(`inject-${entry.id}`);
    try {
      await injectMemories(entry.agentId, [entry.id]);
      setNotice(`已注入到 ${entry.agentId} 的记忆文件`);
    } catch (e) {
      setNotice(`注入失败: ${e}`);
    } finally {
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
        🧠 记忆管理
      </h2>

      {/* 采集按钮 */}
      <div style={{ display: 'flex', gap: 8, marginBottom: 14, flexWrap: 'wrap' }}>
        {AGENTS.map((a) => (
          <button
            key={a.id}
            onClick={() => onCollect(a.id)}
            disabled={busy === a.id}
            style={{
              padding: '6px 14px',
              borderRadius: 8,
              border: '1px solid #4b5563',
              background: '#1f2937',
              color: '#d1d5db',
              fontSize: 13,
              cursor: busy === a.id ? 'wait' : 'pointer',
            }}
          >
            {busy === a.id ? '采集中...' : `采集 ${a.label}`}
          </button>
        ))}
      </div>

      {/* 搜索 */}
      <div style={{ display: 'flex', gap: 8, marginBottom: 14 }}>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && onSearch()}
          placeholder="全文搜索记忆（FTS5）"
          style={{
            flex: 1,
            padding: '8px 12px',
            borderRadius: 8,
            border: '1px solid #374151',
            background: '#1f2937',
            color: '#f3f4f6',
            fontSize: 13,
            outline: 'none',
          }}
        />
        <button
          onClick={onSearch}
          disabled={busy === 'search'}
          style={{
            padding: '8px 18px',
            borderRadius: 8,
            border: 'none',
            background: 'linear-gradient(135deg, #6366f1, #8b5cf6)',
            color: '#fff',
            fontSize: 13,
            cursor: busy === 'search' ? 'wait' : 'pointer',
          }}
        >
          {busy === 'search' ? '搜索中...' : '搜索'}
        </button>
      </div>

      {notice && (
        <div style={{ fontSize: 13, color: '#6ee7b7', marginBottom: 10 }}>{notice}</div>
      )}

      {/* 结果 */}
      {results.length === 0 ? (
        <p style={{ color: '#6b7280', fontSize: 13 }}>
          暂无记忆，先点击上方「采集」按钮同步会话历史。
        </p>
      ) : (
        <ul style={{ listStyle: 'none', margin: 0, padding: 0 }}>
          {results.slice(0, 30).map((entry) => (
            <li
              key={entry.id}
              style={{
                padding: '10px 12px',
                marginBottom: 8,
                borderRadius: 8,
                background: '#1f2937',
                border: '1px solid #374151',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8 }}>
                <span
                  style={{
                    fontSize: 11,
                    padding: '2px 8px',
                    borderRadius: 99,
                    background: '#374151',
                    color: '#9ca3af',
                  }}
                >
                  {entry.agentId}
                </span>
                <span style={{ fontSize: 11, color: '#6b7280' }}>
                  {formatTime(entry.collectedAt)}
                </span>
              </div>
              <p
                style={{
                  margin: '8px 0',
                  fontSize: 13,
                  color: '#d1d5db',
                  lineHeight: 1.5,
                  wordBreak: 'break-word',
                }}
              >
                {entry.content}
              </p>
              <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <label style={{ fontSize: 12, color: '#9ca3af', display: 'flex', gap: 6 }}>
                  <input
                    type="checkbox"
                    checked={entry.autoInject}
                    onChange={(e) => onToggleAuto(entry, e.target.checked)}
                  />
                  自动注入
                </label>
                <button
                  onClick={() => onInject(entry)}
                  disabled={busy === `inject-${entry.id}`}
                  style={{
                    padding: '4px 12px',
                    borderRadius: 6,
                    border: '1px solid #6366f1',
                    background: 'transparent',
                    color: '#a5b4fc',
                    fontSize: 12,
                    cursor: 'pointer',
                  }}
                >
                  注入
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};
