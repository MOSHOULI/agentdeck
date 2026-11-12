import { useEffect, useRef, useState } from 'react';
import { previewCodexInstallRecovery, confirmCodexInstallRecovery, runtimeMode } from '../bridge';
import { type CodexInstallRecovery as RecoveryPlan } from '../codexInstallRecoveryContract';
import { useAgentStore } from '../stores/agentStore';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';

export function CodexInstallRecovery({ active, reviewContext, agentContext, selected = false, dataStale = false }: {
  active: boolean; reviewContext: unknown; agentContext?: unknown; selected?: boolean; dataStale?: boolean;
}) {
  const busy = useAgentStore(state => state.loading || state.previewing);
  const [pending, setPending] = useState(false);
  const [plan, setPlan] = useState<RecoveryPlan | null>(null);
  const [error, setError] = useState(false);
  const generation = useRef(0);
  const inFlight = useRef(false);
  const workspaceBusy = useWorkspaceActivityStore(state => state.busy.agents);
  const beginActivity = () => {
    const owner = Symbol('codex-recovery-operation');
    useWorkspaceActivityStore.getState().setActivity('agents', owner, true);
    return () => useWorkspaceActivityStore.getState().setActivity('agents', owner, false);
  };
  const cancel = () => { generation.current++; setPlan(null); setError(false); };
  useEffect(() => { cancel(); return () => { generation.current++; }; }, [active, reviewContext, agentContext, selected, dataStale]);
  useEffect(() => { if (busy) cancel(); }, [busy]);
  const allowed = () => active && !dataStale && !busy && !inFlight.current && runtimeMode() === 'native'
    && !useWorkspaceActivityStore.getState().busy.agents;
  const preview = async () => {
    if (!allowed()) return;
    const request = ++generation.current;
    inFlight.current = true; setPending(true); setPlan(null); setError(false);
    const endActivity = beginActivity();
    try {
      const value = await previewCodexInstallRecovery();
      if (generation.current === request) setPlan(value);
    } catch { if (generation.current === request) setError(true); }
    finally { inFlight.current = false; setPending(false); endActivity(); }
  };
  const confirm = async () => {
    if (!allowed() || plan?.state !== 'eligible' || !plan.revision) return;
    const revision = plan.revision;
    const request = ++generation.current;
    inFlight.current = true; setPending(true); setPlan(null); setError(false);
    const endActivity = beginActivity();
    try {
      await confirmCodexInstallRecovery(revision);
      // A completed recovery stays distinct from a failed following refresh.
      useAgentStore.setState({ notice: '已解除 Codex 探测保护，未安装或清理软件。', error: null });
      await useAgentStore.getState().loadAgents();
    } catch { if (generation.current === request) setError(true); }
    finally { inFlight.current = false; setPending(false); endActivity(); }
  };
  if (!active) return null;
  return <section aria-label="Codex 持久安装保护恢复">
    <p>仅检查本地保护记录、固定安装槽与 PATH 元数据，不运行 npm 或 Codex，不清理文件。崩溃遗留或无法验证的状态不能自动恢复。</p>
    <button type="button" data-testid="codex-recovery-preview" disabled={busy || pending || workspaceBusy || dataStale || runtimeMode() !== 'native'} onClick={() => void preview()}>检查恢复探测条件</button>
    {(pending || plan || error) && <button type="button" data-testid="codex-recovery-cancel" onClick={cancel}>取消恢复检查</button>}
    {pending && <p role="status">正在处理恢复检查；取消只丢弃结果，不中断已提交的操作。</p>}
    {error && <p role="alert" data-testid="codex-recovery-error">恢复检查或确认未完成，保护状态未确认。请保留现有文件；不会自动重试。</p>}
    {plan && <section role="region" aria-label="Codex 安装保护恢复检查" data-testid="codex-recovery-result" data-state={plan.state}>
      {plan.prefix && <p className="agent-path">已记录安装位置：{plan.prefix}</p>}
      <dl>{plan.slots.map(slot => <div key={slot.slot}><dt>{slot.slot}</dt><dd>{slot.state}</dd></div>)}</dl>
      <p>{plan.limitations}</p>
      {plan.state === 'clear' && <>
        <p>当前没有待恢复的持久安装保护。</p>
        <p>此实例可能仍暂停；仅在另一实例已完成显式恢复、持久记录已清除后，可重新打开应用读取该状态。armed 或损坏记录不能靠重启解除。</p>
      </>}
      {plan.state === 'blocked' && <p>当前条件不允许解除保护。保留文件并人工核对；不会执行 npm 或 Codex。</p>}
      {plan.state === 'eligible' && <>
        <p>已只读确认受影响安装槽及当前 Codex PATH 候选均不存在。确认仅解除探测保护，不安装或清理软件。</p>
        <p>确认只解除探测保护；之后刷新可恢复 npm/Codex 查询。不安装、不删除，也不代表包真实性或运行验证。</p>
        <button type="button" data-testid="codex-recovery-confirm" disabled={busy || pending || workspaceBusy || dataStale} onClick={() => void confirm()}>确认恢复 Codex 探测</button>
      </>}
    </section>}
  </section>;
}
