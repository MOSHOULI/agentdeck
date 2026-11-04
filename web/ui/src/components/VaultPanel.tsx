/** Local encrypted credentials and event-driven recovery snapshots. */
import React, { useEffect, useRef, useState } from 'react';
import type { ApiKeyInfo, BackupInfo } from '../bridge';
import * as bridge from '../bridge';
import { useWorkspaceActivity } from '../stores/workspaceActivity';
import { Icon } from './Icon';
import './VaultUpdate.css';

type Notice = { kind: 'success' | 'error' | 'warning'; message: string };

const RESTORE_SCOPE = '仅恢复 AgentDeck 管理的密钥 vault 与配置备份副本；不会卸载或重装软件，也不会自动覆盖外部 Agent 配置。';

function backupDate(timestamp: string): Date | null {
  const date = new Date(Number(timestamp) / 1_000_000);
  return Number.isFinite(date.getTime()) ? date : null;
}

function formatBackupTime(timestamp: string): string {
  const date = backupDate(timestamp);
  if (!date) return '时间未知';
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(
    date.getDate()
  ).padStart(2, '0')} ${String(date.getHours()).padStart(2, '0')}:${String(
    date.getMinutes()
  ).padStart(2, '0')}`;
}

export const VaultPanel: React.FC = () => {
  const [keys, setKeys] = useState<ApiKeyInfo[] | null>(null);
  const [backups, setBackups] = useState<BackupInfo[] | null>(null);
  const [keyError, setKeyError] = useState<string | null>(null);
  const [backupError, setBackupError] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [provider, setProvider] = useState('');
  const [key, setKey] = useState('');
  const [visibleBackups, setVisibleBackups] = useState(20);
  const operation = useRef(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const mode = bridge.runtimeMode();
  const readOnly = mode !== 'native';
  const locked = loading || busy !== null;
  const keyListReady = keys !== null && keyError === null;
  useWorkspaceActivity('vault', locked);

  // Load the two resources independently: one failure must not hide the other.
  const refresh = async () => {
    setLoading(true);
    const [keyResult, backupResult] = await Promise.allSettled([
      bridge.listApiKeys(),
      bridge.listBackups(),
    ]);
    if (keyResult.status === 'fulfilled') {
      setKeys(keyResult.value);
      setKeyError(null);
    } else {
      setKeyError(`密钥加载失败：${String(keyResult.reason)}`);
    }
    if (backupResult.status === 'fulfilled') {
      setBackups(backupResult.value);
      setBackupError(null);
    } else {
      setBackupError(`备份加载失败：${String(backupResult.reason)}`);
    }
    setLoading(false);
  };

  const onRefresh = async () => {
    if (operation.current) return;
    operation.current = true;
    setNotice(null);
    try {
      await refresh();
    } finally {
      operation.current = false;
    }
  };

  useEffect(() => {
    void onRefresh();
  }, []);

  // The ref locks synchronously, including rapid clicks before React renders.
  const runOperation = async (id: string, action: () => Promise<void>) => {
    if (operation.current || readOnly) return;
    operation.current = true;
    setBusy(id);
    setNotice(null);
    try {
      await action();
    } finally {
      operation.current = false;
      setBusy(null);
    }
  };

  // A missing or stale list cannot establish whether a save replaces an existing key.
  const requireCurrentKeys = () => {
    if (keyListReady) return true;
    setNotice({ kind: 'error', message: '请先刷新并成功读取密钥列表，再修改或复制密钥。已填写的内容会保留。' });
    return false;
  };

  const onSave = async (event: React.FormEvent) => {
    event.preventDefault();
    if (operation.current || readOnly || !requireCurrentKeys()) return;
    if (!name.trim() || !key.trim()) {
      setNotice({ kind: 'error', message: '名称和密钥不能为空' });
      return;
    }
    if (
      keys?.some((info) => info.agentId === name.trim()) &&
      !window.confirm(`已存在 ${name.trim()} 的密钥。确认替换？替换前会自动备份。`)
    ) return;
    await runOperation('save', async () => {
      try {
        await bridge.saveApiKey(name.trim(), provider.trim() || 'manual', key.trim());
        setName('');
        setProvider('');
        setKey('');
        await refresh();
        setNotice({ kind: 'success', message: '已保存（加密存储）' });
      } catch (error) {
        // Never echo a submitted secret if an error happens to contain it.
        const message = String(error).split(key.trim()).join('[密钥已隐藏]');
        setNotice({ kind: 'error', message: `保存失败：${message}` });
      }
    });
  };

  const onCopy = async (info: ApiKeyInfo) => {
    if (operation.current || readOnly || !requireCurrentKeys()) return;
    await runOperation(`copy-${info.agentId}`, async () => {
      let plain: string | null = null;
      try {
        // This explicit user action is the only plaintext read in the workspace.
        plain = await bridge.getApiKey(info.agentId);
        if (!plain) {
          setNotice({ kind: 'error', message: '未找到该密钥，请刷新列表后重试。' });
          return;
        }
        await navigator.clipboard.writeText(plain);
        setNotice({ kind: 'success', message: `已复制 ${info.agentId} 的密钥。请留意剪贴板中的敏感内容。` });
      } catch (error) {
        const message = plain ? String(error).split(plain).join('[密钥已隐藏]') : String(error);
        setNotice({ kind: 'error', message: `复制失败：${message}` });
      } finally {
        plain = null;
      }
    });
  };

  const onDelete = async (info: ApiKeyInfo) => {
    if (operation.current || readOnly || !requireCurrentKeys()) return;
    if (!window.confirm(`确认删除 ${info.agentId} 的 API Key？删除前会自动备份。`)) return;
    await runOperation(`del-${info.agentId}`, async () => {
      try {
        const deleted = await bridge.deleteApiKey(info.agentId);
        await refresh();
        setNotice({
          kind: deleted ? 'success' : 'warning',
          message: deleted ? `已删除 ${info.agentId}` : `密钥 ${info.agentId} 已不存在`,
        });
      } catch (error) {
        setNotice({ kind: 'error', message: `删除失败：${String(error)}` });
      }
    });
  };

  const onImportV2 = async () => {
    if (operation.current || readOnly || !requireCurrentKeys()) return;
    await runOperation('import-v2', async () => {
      try {
        const count = await bridge.importV2Keys();
        await refresh();
        setNotice({
          kind: 'success',
          message: count > 0 ? `已从 V2 vault 导入 ${count} 个密钥` : 'V2 vault 无新密钥可导入',
        });
      } catch (error) {
        setNotice({ kind: 'error', message: `V2 导入失败：${String(error)}` });
      }
    });
  };

  const onRollback = async (info: BackupInfo, full: boolean) => {
    if (operation.current || readOnly) return;
    const consequence = full
      ? '将备份覆盖的内容整体还原至该时间点，并移除之后新增、已纳入备份的文件。之后的相关变更会被覆盖。'
      : '只还原该次备份变更的文件，可能覆盖或删除这些文件，不影响其他备份内容。';
    if (!window.confirm(
      `确认${full ? '整体恢复' : '按操作回滚'}（${formatBackupTime(info.timestamp)}）？\n${consequence}\n${RESTORE_SCOPE}\n恢复前会自动创建安全快照。`
    )) return;
    await runOperation(`restore-${info.hash}`, async () => {
      try {
        const restored = await bridge.restoreBackup(info.timestamp, full);
        await refresh();
        setNotice({ kind: 'success', message: `已${full ? '整体恢复' : '按操作回滚'} ${restored.length} 个文件` });
      } catch (error) {
        setNotice({ kind: 'error', message: `回滚失败：${String(error)}` });
      }
    });
  };

  return (
    <div className="vault-workspace">
      <header className="workspace-heading">
        <div>
          <p className="eyebrow">SECURITY & RECOVERY</p>
          <h1>密钥与备份</h1>
          <p className="muted">集中保管 API 密钥，在需要时恢复本机备份。</p>
        </div>
        <button className="button" onClick={onRefresh} disabled={locked}>
          <Icon name="refresh" />{loading ? '刷新中…' : '刷新列表'}
        </button>
      </header>

      {readOnly && (
        <div className="notice vault-runtime-notice" role="status">
          <Icon name="info" />
          <span>{mode === 'demo' ? '只读演示：密钥与备份均为示例。' : '未连接桌面后端。'} 请在桌面应用中管理密钥和恢复备份。</span>
        </div>
      )}
      {notice && (
        <div className={`notice notice-${notice.kind}`} role={notice.kind === 'error' ? 'alert' : 'status'}>
          <Icon name={notice.kind === 'success' ? 'check' : 'alert'} />
          <span>{notice.message}</span>
        </div>
      )}

      <section className="panel vault-keys-panel" aria-labelledby="vault-keys-heading" aria-busy={locked}>
        <div className="panel-heading">
          <div className="vault-section-title"><Icon name="key" /><h2 id="vault-keys-heading">密钥保管库</h2></div>
          <span className="status-badge"><Icon name="vault" size={14} />{keys === null ? '尚未读取' : `${keys.length} 个密钥`}</span>
        </div>
        <p className="muted vault-section-description">仅展示脱敏值。只有点击「复制」时才读取明文。</p>

        <form className="vault-form" onSubmit={onSave} aria-label="添加密钥" noValidate autoComplete="off">
          <div className="field">
            <label className="field-label" htmlFor="vault-name">名称 <span className="muted">必填</span></label>
            <input className="control" id="vault-name" value={name} onChange={(event) => setName(event.target.value)}
              placeholder="名称（如 codex）" required disabled={locked || readOnly} autoComplete="off" spellCheck={false} />
          </div>
          <div className="field">
            <label className="field-label" htmlFor="vault-provider">来源 <span className="muted">可选</span></label>
            <input className="control" id="vault-provider" value={provider} onChange={(event) => setProvider(event.target.value)}
              placeholder="来源（如 manual / env）" disabled={locked || readOnly} autoComplete="off" spellCheck={false} />
          </div>
          <div className="field vault-secret-field">
            <label className="field-label" htmlFor="vault-secret">API Key <span className="muted">必填</span></label>
            <input className="control" id="vault-secret" value={key} onChange={(event) => setKey(event.target.value)}
              placeholder="API Key（由本机 vault 加密存储）" type="password" required disabled={locked || readOnly}
              autoComplete="new-password" spellCheck={false} aria-describedby="vault-secret-help" />
          </div>
          <div className="vault-form-footer">
            <p className="muted" id="vault-secret-help">保存在本机加密 vault 中。使用相同名称会替换已有密钥。</p>
            <button className="button button-primary" type="submit" disabled={locked || readOnly || !keyListReady}>
              <Icon name="plus" />{busy === 'save' ? '保存中…' : '保存'}
            </button>
          </div>
        </form>

        <div className="vault-list-heading">
          <h3>已存密钥 <span className="muted">脱敏显示</span></h3>
          <button className="button vault-small-button" onClick={onImportV2} disabled={locked || readOnly || !keyListReady}>
            <Icon name="download" size={16} />{busy === 'import-v2' ? '导入中…' : '导入 V2 vault 密钥'}
          </button>
        </div>
        {keyError && <div className="notice notice-error" role="alert"><Icon name="alert" /><span>{keyError}{keys !== null && '；下方为上次读取的列表。'}</span></div>}
        {keys === null ? (
          <div className="empty-state"><Icon name={keyError ? 'alert' : 'key'} size={24} /><p>{keyError ? '暂时无法读取密钥，请刷新重试。' : '正在读取密钥列表…'}</p></div>
        ) : keys.length === 0 ? (
          <div className="empty-state"><Icon name="key" size={24} /><p>暂无密钥</p><span className="muted">添加 API Key，或从已有 V2 vault 导入。</span></div>
        ) : (
          <ul className="vault-key-list" aria-label="已存密钥">
            {keys.map((info) => (
              <li key={info.agentId} className="vault-key-row">
                <div className="vault-key-identity"><strong>{info.agentId}</strong><span className="muted">{info.provider}</span></div>
                <code className="vault-masked-key">{info.maskedKey}</code>
                <div className="vault-row-actions">
                  <button className="button vault-small-button" onClick={() => onCopy(info)} disabled={locked || readOnly || !keyListReady} aria-label={`复制 ${info.agentId} 的密钥`}>
                    <Icon name="copy" size={16} />{busy === `copy-${info.agentId}` ? '复制中…' : '复制'}
                  </button>
                  <button className="button button-danger vault-small-button" onClick={() => onDelete(info)} disabled={locked || readOnly || !keyListReady} aria-label={`删除 ${info.agentId} 的密钥`}>
                    <Icon name="trash" size={16} />{busy === `del-${info.agentId}` ? '删除中…' : '删除'}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="panel vault-backups-panel" aria-labelledby="vault-backups-heading" aria-busy={locked}>
        <div className="panel-heading">
          <div className="vault-section-title"><Icon name="refresh" /><h2 id="vault-backups-heading">自动备份</h2></div>
          <span className="status-badge"><Icon name="info" size={14} />{backups === null ? '尚未读取' : `${backups.length} 条记录`}</span>
        </div>
        <p className="muted vault-section-description">安装、注入或 vault 变动前自动备份；恢复前也会创建安全快照。</p>
        <p className="vault-recovery-scope"><Icon name="info" size={15} /><span>{RESTORE_SCOPE}</span></p>
        <div className="vault-recovery-guide">
          <div><strong>按操作回滚</strong><p>只还原该次备份变更的文件，可能覆盖或删除对应文件。</p></div>
          <div><strong>整体恢复</strong><p>还原备份覆盖的完整状态，并移除之后新增、已纳入备份的文件。</p></div>
        </div>
        {backupError && <div className="notice notice-error" role="alert"><Icon name="alert" /><span>{backupError}{backups !== null && '；下方为上次读取的记录。'}</span></div>}
        {backups === null ? (
          <div className="empty-state"><Icon name={backupError ? 'alert' : 'refresh'} size={24} /><p>{backupError ? '暂时无法读取备份，请刷新重试。' : '正在读取备份历史…'}</p></div>
        ) : backups.length === 0 ? (
          <div className="empty-state"><Icon name="refresh" size={24} /><p>暂无备份记录</p><span className="muted">受支持的本机变更发生时，会自动产生备份。</span></div>
        ) : (
          <>
            <ul className="vault-backup-list" aria-label="备份历史">
              {backups.slice(0, visibleBackups).map((info) => (
                <li key={info.hash} className="vault-backup-row">
                  <div className="vault-backup-details">
                    <strong>{info.message}</strong>
                    <div className="vault-backup-meta"><time dateTime={backupDate(info.timestamp)?.toISOString()}>{formatBackupTime(info.timestamp)}</time><code>{info.hash.slice(0, 8)}</code></div>
                  </div>
                  <div className="vault-row-actions">
                    <button className="button vault-small-button" onClick={() => onRollback(info, false)} disabled={locked || readOnly} aria-label={`按操作回滚 ${info.message}`}>
                      <Icon name="refresh" size={16} />{busy === `restore-${info.hash}` ? '恢复中…' : '按操作回滚'}
                    </button>
                    <button className="button vault-restore-button vault-small-button" onClick={() => onRollback(info, true)} disabled={locked || readOnly} aria-label={`整体恢复 ${info.message}`}>
                      <Icon name="alert" size={16} />整体恢复
                    </button>
                  </div>
                </li>
              ))}
            </ul>
            {backups.length > visibleBackups && <button className="button vault-more-button" onClick={() => setVisibleBackups((count) => count + 20)}>显示更多备份（剩余 {backups.length - visibleBackups} 条）<Icon name="chevron-down" size={16} /></button>}
          </>
        )}
      </section>
    </div>
  );
};
