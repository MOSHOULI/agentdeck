/**
 * 密钥与备份面板（M4）。
 * vault：API Key 加密存储（添加/删除/脱敏查看/复制），支持 V2 导入。
 * 备份：事件驱动 git 备份列表，按操作回滚 / 按时间点整体恢复。
 */

import React, { useEffect, useState, useRef } from 'react';
import type { ApiKeyInfo, BackupInfo } from '../bridge';
import * as bridge from '../bridge';

function formatTimeMs(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
    d.getDate()
  ).padStart(2, '0')} ${String(d.getHours()).padStart(2, '0')}:${String(
    d.getMinutes()
  ).padStart(2, '0')}`;
}

function formatBackupTime(ts: string): string {
  return formatTimeMs(Number(ts) / 1_000_000);
}

const inputStyle: React.CSSProperties = {
  flex: 1,
  padding: '8px 12px',
  borderRadius: 8,
  border: '1px solid #374151',
  background: '#1f2937',
  color: '#f3f4f6',
  fontSize: 13,
  outline: 'none',
};

export const VaultPanel: React.FC = () => {
  const [keys, setKeys] = useState<ApiKeyInfo[]>([]);
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [name, setName] = useState('');
  const [provider, setProvider] = useState('');
  const [key, setKey] = useState('');
  const operation = useRef(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = async () => {
    setKeys(await bridge.listApiKeys());
    setBackups(await bridge.listBackups());
  };

  useEffect(() => {
    refresh().catch((e) => setNotice(`加载失败: ${e}`));
  }, []);

  const onSave = async () => {
    if (operation.current) return;
    if (!name.trim() || !key.trim()) {
      setNotice('名称和密钥不能为空');
      return;
    }
    operation.current = true;
    setBusy('save');
    setNotice(null);
    try {
      await bridge.saveApiKey(name.trim(), provider.trim() || 'manual', key.trim());
      setName('');
      setProvider('');
      setKey('');
      await refresh();
      setNotice('已保存（加密存储）');
    } catch (e) {
      setNotice(`保存失败: ${e}`);
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onCopy = async (info: ApiKeyInfo) => {
    if (operation.current) return;
    operation.current = true;
    setBusy(`copy-${info.agentId}`);
    try {
      const plain = await bridge.getApiKey(info.agentId);
      if (!plain) {
        setNotice('未找到该密钥');
        return;
      }
      await navigator.clipboard.writeText(plain);
      setNotice(`已复制 ${info.agentId} 的密钥`);
    } catch (e) {
      setNotice(`复制失败: ${e}`);
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onDelete = async (info: ApiKeyInfo) => {
    if (operation.current) return;
    if (!window.confirm(`确认删除 ${info.agentId} 的 API Key？删除前会自动备份。`)) return;
    operation.current = true;
    setBusy(`del-${info.agentId}`);
    try {
      const deleted = await bridge.deleteApiKey(info.agentId);
      await refresh();
      setNotice(deleted ? `已删除 ${info.agentId}` : `密钥 ${info.agentId} 已不存在`);
    } catch (e) {
      setNotice(`删除失败: ${e}`);
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onImportV2 = async () => {
    if (operation.current) return;
    operation.current = true;
    setBusy('import-v2');
    try {
      const count = await bridge.importV2Keys();
      await refresh();
      setNotice(count > 0 ? `已从 V2 vault 导入 ${count} 个密钥` : 'V2 vault 无新密钥可导入');
    } catch (e) {
      setNotice(`V2 导入失败: ${e}`);
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  const onRollback = async (info: BackupInfo, full: boolean) => {
    if (operation.current) return;
    const label = full ? '整体恢复到该时间点' : '按操作回滚';
    if (
      !window.confirm(
        `确认${label}（${formatBackupTime(info.timestamp)}）？此操作会还原备份覆盖的文件。`
      )
    )
      return;
    operation.current = true;
    setBusy(`restore-${info.hash}`);
    try {
      const restored = await bridge.restoreBackup(info.timestamp, full);
      await refresh();
      setNotice(`已回滚 ${restored.length} 个文件`);
    } catch (e) {
      setNotice(`回滚失败: ${e}`);
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
        🔐 密钥与备份
      </h2>

      {/* 添加密钥 */}
      <div style={{ display: 'flex', gap: 8, marginBottom: 10, flexWrap: 'wrap' }}>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="名称（如 codex）"
          style={{ ...inputStyle, flex: '0 1 140px' }}
        />
        <input
          value={provider}
          onChange={(e) => setProvider(e.target.value)}
          placeholder="来源（如 manual / env）"
          style={{ ...inputStyle, flex: '0 1 140px' }}
        />
        <input
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder="API Key（由本机 vault 加密存储）"
          style={inputStyle}
          type="password"
        />
        <button
          onClick={onSave}
          disabled={busy !== null}
          style={{
            padding: '8px 18px',
            borderRadius: 8,
            border: 'none',
            background: 'linear-gradient(135deg, #6366f1, #8b5cf6)',
            color: '#fff',
            fontSize: 13,
            cursor: busy === 'save' ? 'wait' : 'pointer',
          }}
        >
          {busy === 'save' ? '保存中...' : '保存'}
        </button>
      </div>

      <div style={{ display: 'flex', gap: 8, marginBottom: 14 }}>
        <button
          onClick={onImportV2}
          disabled={busy !== null}
          style={{
            padding: '6px 14px',
            borderRadius: 8,
            border: '1px solid #4b5563',
            background: '#1f2937',
            color: '#d1d5db',
            fontSize: 13,
            cursor: busy === 'import-v2' ? 'wait' : 'pointer',
          }}
        >
          {busy === 'import-v2' ? '导入中...' : '导入 V2 vault 密钥'}
        </button>
      </div>

      {notice && (
        <div role="status" style={{ fontSize: 13, color: '#6ee7b7', marginBottom: 10 }}>{notice}</div>
      )}

      {/* 密钥列表 */}
      <h3 style={{ fontSize: 14, fontWeight: 600, color: '#9ca3af', margin: '12px 0 8px 0' }}>
        已存密钥（脱敏显示）
      </h3>
      {keys.length === 0 ? (
        <p style={{ color: '#6b7280', fontSize: 13 }}>暂无密钥，先在上面添加。</p>
      ) : (
        <ul style={{ listStyle: 'none', margin: 0, padding: 0 }}>
          {keys.map((info) => (
            <li
              key={info.agentId}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                padding: '8px 12px',
                marginBottom: 8,
                borderRadius: 8,
                background: '#1f2937',
                border: '1px solid #374151',
              }}
            >
              <span
                style={{
                  fontSize: 12,
                  padding: '2px 8px',
                  borderRadius: 99,
                  background: '#374151',
                  color: '#9ca3af',
                }}
              >
                {info.agentId}
              </span>
              <span style={{ fontSize: 12, color: '#6b7280' }}>{info.provider}</span>
              <code style={{ fontSize: 13, color: '#d1d5db', flex: 1 }}>{info.maskedKey}</code>
              <button
                onClick={() => onCopy(info)}
                disabled={busy !== null}
                style={smallBtn}
              >
                复制
              </button>
              <button
                onClick={() => onDelete(info)}
                disabled={busy !== null}
                style={{ ...smallBtn, borderColor: '#dc2626', color: '#fca5a5' }}
              >
                删除
              </button>
            </li>
          ))}
        </ul>
      )}

      {/* 备份历史 */}
      <h3 style={{ fontSize: 14, fontWeight: 600, color: '#9ca3af', margin: '18px 0 8px 0' }}>
        备份历史（每次安装/注入/vault 变动前自动备份）
      </h3>
      {backups.length === 0 ? (
        <p style={{ color: '#6b7280', fontSize: 13 }}>暂无备份记录。</p>
      ) : (
        <ul style={{ listStyle: 'none', margin: 0, padding: 0 }}>
          {backups.slice(0, 20).map((info) => (
            <li
              key={info.hash}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                padding: '8px 12px',
                marginBottom: 8,
                borderRadius: 8,
                background: '#1f2937',
                border: '1px solid #374151',
              }}
            >
              <span style={{ fontSize: 12, color: '#6b7280', whiteSpace: 'nowrap' }}>
                {formatBackupTime(info.timestamp)}
              </span>
              <span style={{ fontSize: 13, color: '#d1d5db', flex: 1 }}>{info.message}</span>
              <button
                onClick={() => onRollback(info, false)}
                disabled={busy !== null}
                style={smallBtn}
              >
                按操作回滚
              </button>
              <button
                onClick={() => onRollback(info, true)}
                disabled={busy !== null}
                style={{ ...smallBtn, borderColor: '#b45309', color: '#fcd34d' }}
              >
                整体恢复
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

const smallBtn: React.CSSProperties = {
  padding: '4px 12px',
  borderRadius: 6,
  border: '1px solid #6366f1',
  background: 'transparent',
  color: '#a5b4fc',
  fontSize: 12,
  cursor: 'pointer',
};
