import { useEffect, useRef, useState } from 'react';
import * as bridge from '../bridge';
import type { QwenVimPreview } from '../qwenContract';
interface Props {
  settings: string; manifest: string; inspecting: boolean;
  onActivity: (busy: boolean, mutating: boolean) => void;
}
export function QwenVimSetting({ settings, manifest, inspecting, onActivity }: Props) {
  const [enabled, setEnabled] = useState(false);
  const [busy, setBusy] = useState(false);
  const [mutating, setMutating] = useState(false);
  const [preview, setPreview] = useState<QwenVimPreview | null>(null);
  const [restoreConfirm, setRestoreConfirm] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState(false);
  const generation = useRef(0);
  const running = useRef(false);
  const mounted = useRef(false);
  const mode = bridge.runtimeMode();
  const available = mode === 'native' && !!settings.trim() && !!manifest.trim();
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current++; }; }, []);
  useEffect(() => { onActivity(busy, mutating); }, [busy, mutating, onActivity]);
  useEffect(() => {
    generation.current++; setPreview(null); setRestoreConfirm(false); setMessage(''); setError(false);
  }, [settings, manifest]);
  const invalidate = () => { generation.current++; setPreview(null); setRestoreConfirm(false); setMessage(''); setError(false); };
  const run = async (action: 'preview' | 'apply' | 'restore') => {
    if (running.current || inspecting || !available || (action === 'apply' && (!preview || !preview.changes)) || (action === 'restore' && !restoreConfirm)) return;
    const selectedPreview = preview;
    running.current = true; onActivity(true, action !== 'preview'); setBusy(true); setMutating(action !== 'preview'); setMessage(''); setError(false);
    const current = ++generation.current;
    setPreview(null); setRestoreConfirm(false);
    try {
      if (action === 'preview') {
        const next = await bridge.previewQwenVim(settings, manifest, enabled);
        if (mounted.current && current === generation.current) setPreview(next);
      } else {
        const changed = action === 'apply'
          ? await bridge.applyQwenVim(settings, manifest, selectedPreview!.after, selectedPreview!.token)
          : await bridge.restoreQwenVim(settings, manifest);
        if (mounted.current && current === generation.current) setMessage(changed
          ? action === 'apply' ? '所选文件的 Vim 模式字段已应用。尚未验证 Qwen 实际运行效果。' : '受管字段已撤销。尚未验证 Qwen 实际运行效果。'
          : '操作完成，未改变配置字段。');
      }
    } catch {
      if (mounted.current && current === generation.current) {
        setError(true);
        setMessage(action === 'preview' ? '无法生成兼容预览。请检查所选文件与版本。'
          : '未能确认操作完成。请检查文件与所有权记录；不要假定配置未改变或重复提交。重新预览或处理冲突后再试。');
      }
    } finally {
      running.current = false;
      if (mounted.current) { setBusy(false); setMutating(false); }
    }
  };
  return <section className="panel" aria-label="Qwen Vim 模式管理">
    <h2>Vim 模式：单字段管理</h2>
    <p>仅支持 Qwen 0.27.0 / schema 4 的 general.vimMode。请先停止 Qwen 和其他配置编辑器。此操作不是跨进程事务。</p>
    <p className="muted">只修改所选文件中的布尔值；JSON 排版可能改变，其他字段按语义保留。其他配置层可能覆盖此值。旁置所有权记录仅保存原布尔值或缺省状态，不备份完整配置；外部修改受管字段时会拒绝撤销。</p>
    {mode !== 'native' && <p>此功能仅在桌面后端开放；演示模式不会预览或写入文件。</p>}
    <label><input type="checkbox" checked={enabled} disabled={busy || inspecting || !available} onChange={event => { invalidate(); setEnabled(event.target.checked); }} />启用 Vim 模式</label>
    <div className="dsh-form-actions">
      <button type="button" className="button" disabled={busy || inspecting || !available} onClick={() => { void run('preview'); }}>预览 Vim 变更</button>
      <button type="button" className="button" disabled={busy || inspecting || !available} onClick={() => { invalidate(); setRestoreConfirm(true); }}>撤销受管 Vim 设置</button>
    </div>
    {busy && <p role="status">{mutating ? '正在处理配置，不能取消已提交的文件操作。' : '正在生成只读预览…'}</p>}
    {preview && <div role="dialog" aria-label="确认 Vim 变更">
      <p>原值：{preview.before === null ? '未声明（官方默认关闭）' : preview.before ? '启用' : '关闭'}；目标：{preview.after ? '启用' : '关闭'}。</p>
      <p>{preview.changes ? '确认后才会写入所选文件。' : '当前字段与目标一致，无需应用。'}</p>
      <button type="button" className="button button-primary" disabled={busy || !preview.changes} onClick={() => { void run('apply'); }}>确认应用 Vim 变更</button>
      <button type="button" className="button" onClick={invalidate}>取消 Vim 变更</button>
    </div>}
    {restoreConfirm && <div role="dialog" aria-label="确认撤销 Vim 设置">
      <p>仅根据所选文件旁的 AgentDeck 所有权记录撤销受管字段；不是恢复完整配置。若字段已被外部更改，将拒绝覆盖。</p>
      <button type="button" className="button button-primary" disabled={busy} onClick={() => { void run('restore'); }}>确认撤销 Vim 设置</button>
      <button type="button" className="button" onClick={invalidate}>取消撤销</button>
    </div>}
    {message && <p role={error ? 'alert' : 'status'}>{message}</p>}
  </section>;
}
