/**
 * 在线更新面板（M6）。
 * 启动时后台静默检查注册表更新；有新版提示并一键应用（ed25519 验签 + SHA-256 校验）。
 */

import React, { useEffect, useState } from 'react';
import type { UpdateSummary } from '../bridge';
import * as bridge from '../bridge';

type UpdateState =
  | { kind: 'checking' }
  | { kind: 'available'; summary: UpdateSummary }
  | { kind: 'upToDate' }
  | { kind: 'error'; message: string }
  | { kind: 'applied'; files: string[] };

export const UpdatePanel: React.FC = () => {
  const [state, setState] = useState<UpdateState>({ kind: 'checking' });
  const [version, setVersion] = useState('v1.0.0');
  const [busy, setBusy] = useState(false);

  const check = async () => {
    setState({ kind: 'checking' });
    try {
      const summary = await bridge.checkUpdates();
      setState(summary ? { kind: 'available', summary } : { kind: 'upToDate' });
    } catch (e) {
      setState({ kind: 'error', message: String(e) });
    }
  };

  useEffect(() => {
    bridge
      .getAppVersion()
      .then(setVersion)
      .catch(() => undefined);
    check();
  }, []);

  const onApply = async () => {
    setBusy(true);
    try {
      const files = await bridge.applyUpdates();
      setState({ kind: 'applied', files });
    } catch (e) {
      setState({ kind: 'error', message: String(e) });
    } finally {
      setBusy(false);
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
      <h2 style={{ fontSize: 18, fontWeight: 600, color: '#e5e7eb', margin: '0 0 8px 0' }}>
        ⬆️ 在线更新
      </h2>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
        <span style={{ fontSize: 13, color: '#9ca3af' }}>当前版本 {version}</span>
        <button
          onClick={check}
          disabled={state.kind === 'checking'}
          style={{
            padding: '6px 14px',
            borderRadius: 8,
            border: '1px solid #4b5563',
            background: '#1f2937',
            color: '#d1d5db',
            fontSize: 13,
            cursor: state.kind === 'checking' ? 'wait' : 'pointer',
          }}
        >
          {state.kind === 'checking' ? '检查中...' : '检查更新'}
        </button>
        {state.kind === 'available' && (
          <button
            onClick={onApply}
            disabled={busy}
            style={{
              padding: '6px 14px',
              borderRadius: 8,
              border: 'none',
              background: 'linear-gradient(135deg, #6366f1, #8b5cf6)',
              color: '#fff',
              fontSize: 13,
              cursor: busy ? 'wait' : 'pointer',
            }}
          >
            {busy ? '应用中...' : '应用更新'}
          </button>
        )}
      </div>

      {state.kind === 'available' && (
        <p style={{ fontSize: 13, color: '#fcd34d', margin: '10px 0 0 0' }}>
          发现新版本 {state.summary.version}（发布于 {state.summary.released}），
          包含 {state.summary.files.join('、')}，签名指纹 {state.summary.pubkeyFp}
        </p>
      )}
      {state.kind === 'upToDate' && (
        <p style={{ fontSize: 12, color: '#6b7280', margin: '10px 0 0 0' }}>已是最新注册表。</p>
      )}
      {state.kind === 'applied' && (
        <p style={{ fontSize: 13, color: '#6ee7b7', margin: '10px 0 0 0' }}>
          已更新：{state.files.join('、')}（重启应用后生效）
        </p>
      )}
      {state.kind === 'error' && (
        <p style={{ fontSize: 12, color: '#fca5a5', margin: '10px 0 0 0' }}>
          更新检查失败：{state.message}
        </p>
      )}
    </div>
  );
};
