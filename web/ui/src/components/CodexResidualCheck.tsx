import { useEffect, useRef, useState } from 'react';
import { inspectCodexInstallResidual } from '../bridge';
import { type ResidualSlots } from '../codexResidualContract';
import { useAgentStore } from '../stores/agentStore';

export function CodexResidualCheck({ sessionRef }: { sessionRef: string }) {
  const busy = useAgentStore(state => state.loading || state.previewing);
  const request = useRef(0);
  const [pending, setPending] = useState(false);
  const [slots, setSlots] = useState<ResidualSlots | null>(null);
  const [error, setError] = useState(false);
  const close = () => { request.current++; setPending(false); setSlots(null); setError(false); };
  useEffect(() => { close(); return () => { request.current++; }; }, [sessionRef]);
  useEffect(() => { if (busy) close(); }, [busy]);
  const inspect = async () => {
    const generation = ++request.current;
    setPending(true); setSlots(null); setError(false);
    try {
      const result = await inspectCodexInstallResidual(sessionRef);
      if (request.current === generation) setSlots(result);
    } catch { if (request.current === generation) setError(true); }
    finally { if (request.current === generation) setPending(false); }
  };
  return <section aria-label="Codex 固定槽位只读检查">
    <p>仅按本会话后端保存的已审核目标检查四个固定槽位。不会读取文件内容、执行 npm 或 Agent、修改或清理文件。</p>
    <button disabled={busy || pending} onClick={() => void inspect()}>只读检查固定残留槽位</button>
    {(pending || slots || error) && <button onClick={close}>关闭检查结果</button>}
    {pending && <p role="status">正在读取槽位元数据；关闭只隐藏结果，不中断已经开始的只读检查。</p>}
    {error && <p role="alert">本会话只读检查不可用或路径发生变化。未清理或执行程序。请保留现有文件。</p>}
    {slots && <section role="region" aria-label="Codex 固定槽位只读检查结果" data-testid="codex-residual-result">
      <p>未运行 npm 或 Agent；所有权未知；不支持清理。</p>
      <dl>{slots.map(({slot, state}) => <div key={slot} aria-label={`${slot}: ${state}`}><dt>{slot}</dt><dd data-testid="codex-residual-state">{state}</dd></div>)}</dl>
      <p>missing＝未找到，file＝文件，directory＝目录，blocked＝拒绝检查。它们不是安装成功、文件来源或清理权限的证明。前后观察不是原子快照，结果可能随时过期；请保留现有数据人工核对。</p>
    </section>}
  </section>;
}
