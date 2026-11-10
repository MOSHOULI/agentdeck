import { useEffect, useRef, useState } from 'react';
import * as bridge from '../bridge';
import type { AnimationsPreview, AnimationsRecovery } from '../codexAnimationsContract';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
type Result = { kind: 'preview'; preview: AnimationsPreview } | { kind: 'recovery'; recovery: AnimationsRecovery } | { kind: 'applied' | 'restored' | 'error' };
export function CodexAnimations({ busy, context }: { busy: boolean; context: string }) {
  const [enabled, setEnabled] = useState(false); const [result, setResult] = useState<Result | null>(null); const [pending, setPending] = useState(false);
  const mounted = useRef(false); const running = useRef(false); const generation = useRef(0);
  const workspaceBusy = useWorkspaceActivityStore(s => s.busy.agents); const native = bridge.runtimeMode() === 'native';
  const clear = () => { generation.current++; setResult(null); };
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current++; }; }, []);
  useEffect(clear, [context, busy]);
  useEffect(() => { if (workspaceBusy && !running.current) clear(); }, [workspaceBusy]);
  const run = async (operation: () => Promise<Result>) => {
    if (running.current || busy || useWorkspaceActivityStore.getState().busy.agents || !native) return;
    const current = ++generation.current; const owner = Symbol('codex-animations');
    running.current = true; setPending(true); setResult(null); useWorkspaceActivityStore.getState().setActivity('agents', owner, true);
    try { const next = await operation(); if (mounted.current && current === generation.current) setResult(next); }
    catch { if (mounted.current && current === generation.current) setResult({ kind: 'error' }); }
    finally { running.current = false; useWorkspaceActivityStore.getState().setActivity('agents', owner, false); if (mounted.current) setPending(false); }
  };
  const disabled = busy || workspaceBusy || pending || !native;
  return <section aria-label="Codex 动画单字段管理" className="agent-details">
    <h3>Codex 终端动画</h3>
    <p>仅 Linux x86_64 支持。固定默认 ~/.codex/config.toml，须已有普通 [tui] 的 animations 布尔值。其他平台或不同 CODEX_HOME 不支持；不执行 Codex，也不证明 Codex 已加载配置。</p>
    {!native && <p>浏览器与演示模式不支持此文件操作，不会模拟应用成功。</p>}
    <label><input type="checkbox" data-testid="codex-animations-enabled" checked={enabled} disabled={disabled} onChange={e => { clear(); setEnabled(e.target.checked); }} />启用终端动画</label>
    <button className="button" data-testid="codex-animations-preview" disabled={disabled} onClick={() => { void run(async () => ({ kind: 'preview', preview: await bridge.previewCodexAnimations(enabled) })); }}>预览动画变更</button>
    <button className="button" data-testid="codex-animations-inspect" disabled={disabled} onClick={() => { void run(async () => ({ kind: 'recovery', recovery: await bridge.inspectCodexAnimationsRecovery() })); }}>检查动画恢复状态</button>
    {(result || pending) && <button className="button" data-testid="codex-animations-cancel" onClick={clear}>取消确认 / 清除结果</button>}
    {pending && <p role="status">操作进行中。清除或离开页面只丢弃展示，不会中止已发出的操作。</p>}
    {result && <section data-testid="codex-animations-result" data-status={result.kind}>
      {result.kind === 'preview' && <><p>当前值：{String(result.preview.before)}；目标值：{String(result.preview.after)}。仅修改此布尔值，保留首次原值供恢复。</p>{result.preview.changes ? <button className="button" disabled={disabled} data-testid="codex-animations-confirm-apply" onClick={() => { const p = result.preview; void run(async () => { await bridge.applyCodexAnimations(p.after, p.token); return { kind: 'applied' }; }); }}>确认应用动画变更</button> : <p>无变更，不需要应用。</p>}</>}
      {result.kind === 'recovery' && <><p>{result.recovery.state === 'no_receipt' ? '没有受管收据，不执行恢复。' : result.recovery.state === 'cleanup_receipt' ? '配置已是首次原图，可确认清理收据。' : '可确认恢复首次原值；任何配置漂移都会拒绝。'}</p>{result.recovery.token && <button className="button" disabled={disabled} data-testid="codex-animations-confirm-restore" onClick={() => { const token = result.recovery.token!; void run(async () => { await bridge.restoreCodexAnimations(token); return { kind: 'restored' }; }); }}>确认恢复动画配置</button>}</>}
      {result.kind === 'applied' && <p role="status">动画配置文件操作完成。可检查恢复状态；不代表 Codex 运行效果已验证。</p>}
      {result.kind === 'restored' && <p role="status">动画配置恢复操作完成。</p>}
      {result.kind === 'error' && <p role="alert">动画操作未完成或结果未确认。平台、路径、格式或文件漂移可能不受支持。不要自动重试写入；请重新检查恢复状态，保留收据。</p>}
    </section>}
    <p className="muted">仅受管精确原图恢复，不支持并发编辑合并。锁不约束外部编辑器；进程中断可能遗留含完整配置的临时文件。没有断电或外部写入原子性保证。</p>
  </section>;
}
