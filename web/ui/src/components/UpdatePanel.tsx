/** Signed registry metadata updates, independent of desktop application releases. */
import React, { useEffect, useRef, useState } from 'react';
import type { UpdateSummary } from '../bridge';
import * as bridge from '../bridge';
import { useWorkspaceActivity } from '../stores/workspaceActivity';
import { Icon } from './Icon';
import './VaultUpdate.css';

type UpdateState =
  | { kind: 'checking' }
  | { kind: 'available'; summary: UpdateSummary }
  | { kind: 'noUpdate' }
  | { kind: 'error'; message: string; phase: 'check' | 'apply' }
  | { kind: 'applied'; files: string[] }
  | { kind: 'unavailable' };

export const UpdatePanel: React.FC = () => {
  const mode = bridge.runtimeMode();
  const readOnly = mode !== 'native';
  const [state, setState] = useState<UpdateState>({ kind: readOnly ? 'unavailable' : 'checking' });
  const [version, setVersion] = useState('未知');
  const [checkedAt, setCheckedAt] = useState<Date | null>(null);
  const operation = useRef(false);
  const [busy, setBusy] = useState(false);
  const checking = state.kind === 'checking';
  useWorkspaceActivity('updates', busy || checking);

  const check = async () => {
    if (operation.current || readOnly) return;
    operation.current = true;
    setState({ kind: 'checking' });
    try {
      const summary = await bridge.checkUpdates();
      setState(summary ? { kind: 'available', summary } : { kind: 'noUpdate' });
      setCheckedAt(new Date());
    } catch (error) {
      setState({ kind: 'error', message: String(error), phase: 'check' });
    } finally {
      operation.current = false;
    }
  };

  useEffect(() => {
    if (readOnly) return;
    bridge.getAppVersion().then((value) => setVersion(value.trim() || '未知')).catch(() => undefined);
    void check();
  }, []);

  const onApply = async () => {
    if (operation.current || readOnly || state.kind !== 'available') return;
    operation.current = true;
    setBusy(true);
    try {
      const files = await bridge.applyUpdates();
      setState({ kind: 'applied', files });
    } catch (error) {
      setState({ kind: 'error', message: String(error), phase: 'apply' });
    } finally {
      operation.current = false;
      setBusy(false);
    }
  };

  const status = busy ? '正在应用更新' : ({
    checking: '正在检查', available: '有可用更新', noUpdate: '检查完成',
    error: '需要处理', applied: '已应用更新', unavailable: '尚未检查',
  } as const)[state.kind];
  const statusIcon = busy || checking ? 'refresh' : state.kind === 'error' ? 'alert'
    : state.kind === 'noUpdate' || state.kind === 'applied' ? 'check' : 'info';

  return (
    <div className="update-workspace">
      <header className="workspace-heading">
        <div>
          <p className="eyebrow">REGISTRY UPDATES</p>
          <h1>注册表更新</h1>
          <p className="muted">获取经过签名验证的 Agent 与 MCP 定义。</p>
        </div>
        <button className="button" onClick={check} disabled={busy || checking || readOnly}>
          <Icon name="refresh" />{checking ? '检查中…' : '检查更新'}
        </button>
      </header>

      <div className="notice update-scope-note"><Icon name="info" /><span>此处仅更新注册表数据，不会升级 AgentDeck 桌面应用或已安装的 Agent。</span></div>

      <section className="panel update-status-panel" aria-labelledby="update-status-heading" aria-busy={busy || checking}>
        <div className="panel-heading">
          <div className="update-section-title"><Icon name="updates" /><h2 id="update-status-heading">更新状态</h2></div>
          <span className={`status-badge update-status-${state.kind}`}><Icon name={statusIcon} size={14} />{status}</span>
        </div>
        <dl className="update-metadata">
          <div><dt>桌面应用版本</dt><dd>{version}</dd></div>
          <div><dt>更新内容</dt><dd>Agent / MCP 注册表</dd></div>
          <div><dt>上次检查成功</dt><dd>{checkedAt ? <time dateTime={checkedAt.toISOString()}>{checkedAt.toLocaleString([], { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })}</time> : '尚无记录'}</dd></div>
        </dl>

        {checking && <div className="empty-state update-checking" role="status"><Icon name="refresh" size={28} /><p>正在检查注册表更新…</p><span className="muted">获取发布清单并验证签名，请稍候。</span></div>}
        {state.kind === 'unavailable' && (
          <div className="notice notice-warning" role="status"><Icon name="info" /><div><strong>{mode === 'demo' ? '只读演示不会检查真实更新。' : '未连接桌面后端，无法检查更新。'}</strong><p>请在 AgentDeck 桌面应用中检查。当前注册表是否最新尚未验证。</p></div></div>
        )}
        {state.kind === 'noUpdate' && (
          <div className="notice notice-success" role="status"><Icon name="check" /><div><strong>暂无可应用的注册表更新。</strong><p>本次检查未找到适用于当前应用的注册表更新，不代表桌面应用已是最新版。</p></div></div>
        )}
        {state.kind === 'error' && (
          <div className="notice notice-error update-error" role="alert"><Icon name="alert" /><div><strong>{state.phase === 'check' ? '检查更新失败' : '应用更新失败'}</strong><p>{state.message}</p><p>请检查网络或发布源后重新检查。签名与文件校验不能跳过。</p></div></div>
        )}
        {state.kind === 'applied' && (
          <div className="notice notice-success" role="status"><Icon name="check" /><div><strong>注册表已应用</strong><p>{state.files.length > 0 ? `已更新：${state.files.join('、')}` : '本次未返回更新文件。'}</p><p>可前往 Agent 广场刷新列表；桌面应用版本保持不变。</p></div></div>
        )}
        {state.kind === 'available' && (
          <div className="update-release">
            <div className="update-release-heading">
              <div><p className="eyebrow">AVAILABLE REGISTRY</p><h3>发现新版本 {state.summary.version}</h3><p className="muted">注册表发布于 {state.summary.released}</p></div>
              <button className="button button-primary" onClick={onApply} disabled={busy || readOnly}>
                <Icon name="download" />{busy ? '应用中…' : '应用更新'}
              </button>
            </div>
            <dl className="update-release-details">
              <div><dt>待更新文件</dt><dd><ul className="update-file-list">{state.summary.files.map((file) => <li key={file}><Icon name="download" size={15} /><code>{file}</code></li>)}</ul></dd></div>
              <div><dt>签名公钥指纹</dt><dd><code>{state.summary.pubkeyFp}</code></dd></div>
            </dl>
            <p className="update-verified"><Icon name="check" size={16} />发布清单签名已验证。应用前还会校验下载文件。</p>
            {busy && <p className="muted" role="status">正在下载、校验并写入注册表。完成前不能重复应用。</p>}
          </div>
        )}
      </section>

      <section className="panel update-security-panel" aria-labelledby="update-security-heading">
        <div className="panel-heading"><div className="update-section-title"><Icon name="vault" /><h2 id="update-security-heading">每次更新都经过校验</h2></div></div>
        <ol className="update-verification-steps">
          <li><span className="update-step-number">01</span><div><h3>验证发布来源</h3><p>Ed25519 签名验证发布清单。</p></div></li>
          <li><span className="update-step-number">02</span><div><h3>校验文件完整性</h3><p>核对 SHA-256、大小与数据结构。</p></div></li>
          <li><span className="update-step-number">03</span><div><h3>安全应用</h3><p>全部校验通过后，再应用注册表。</p></div></li>
        </ol>
      </section>
    </div>
  );
};
