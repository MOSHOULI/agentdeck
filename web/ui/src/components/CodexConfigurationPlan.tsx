import { useEffect, useRef, useState } from 'react';
import * as bridge from '../bridge';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
import type { CodexConflict, CodexTarget } from '../codexConfigurationContract';
const actions: Record<CodexTarget['action'], string> = { create: '拟创建', unchanged: '无变更', merge: '拟合并', replace_managed_block: '拟替换已有受管块', conflict: '存在冲突，不可应用' };
const conflicts: Record<CodexConflict, string> = { managed_block_replacement: '已有受管块将被替换，无法确认其中是否有人工修改', ambiguous_markers: '受管块标记不完整或重复', invalid_target_toml: '现有 TOML 无法解析', toml_type_conflict: '现有配置类型冲突', differing_toml_value: '现有配置值与模板不同' };
export function CodexConfigurationPlan({ busy, context }: { busy: boolean; context: string }) {
  const [result, setResult] = useState<Awaited<ReturnType<typeof bridge.previewCodexConfiguration>> | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState(false);
  const mounted = useRef(false); const running = useRef(false); const generation = useRef(0);
  const activityOwner = useRef(Symbol('codex-configuration-plan'));
  const workspaceBusy = useWorkspaceActivityStore(s => s.busy.agents);
  const mode = bridge.runtimeMode();
  const clear = () => { generation.current++; setResult(null); setError(false); };
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current++; }; }, []);
  useEffect(clear, [context, busy]);
  useEffect(() => useWorkspaceActivityStore.subscribe((state, previous) => {
    // Observe owner transitions synchronously, including overlapping operations
    // and activity that starts and finishes within one React render batch.
    if (state.owners.agents !== previous.owners.agents
      && state.owners.agents.some(owner => owner !== activityOwner.current)) clear();
  }), []);
  const preview = async () => {
    if (running.current || busy || useWorkspaceActivityStore.getState().busy.agents || mode === 'unavailable') return;
    const owner = activityOwner.current; const current = ++generation.current;
    running.current = true; setPending(true); setResult(null); setError(false);
    useWorkspaceActivityStore.getState().setActivity('agents', owner, true);
    try { const next = await bridge.previewCodexConfiguration(); if (mounted.current && current === generation.current) setResult(next); }
    catch { if (mounted.current && current === generation.current) setError(true); }
    finally { running.current = false; useWorkspaceActivityStore.getState().setActivity('agents', owner, false); if (mounted.current) setPending(false); }
  };
  return <section aria-label="Codex 只读配置计划" className="agent-details">
    <button type="button" className="button" data-testid="codex-config-preview" disabled={busy || workspaceBusy || pending || mode === 'unavailable'} onClick={() => { void preview(); }}>{mode === 'demo' ? '查看 Codex 合成配置计划' : '预览 Codex 配置计划（只读）'}</button>
    <p className="muted">不依赖安装检测，不执行 Codex、不初始化配置或备份。只读计划不能应用，也不证明配置生效。</p>
    {(pending || result || error) && <button type="button" className="button" onClick={clear}>清除 Codex 配置计划</button>}
    {pending && <p role="status">正在读取配置计划；清除仅丢弃结果，不中止读取。</p>}
    {error && <p role="alert">Codex 配置计划未完成。请检查路径与文件格式；未获得有效计划，不会改写配置。</p>}
    {result && <section aria-label="Codex 配置计划结果" data-testid="codex-config-result">
      <h3>{result.synthetic ? '合成配置计划示例，未读取本机文件' : '所选默认配置的只读计划'}</h3>
      <p>{result.plan.usesBundledDefaults ? '默认 profile 缺失，使用内置模板计算；未创建 profile。' : '使用现有默认 profile；未修改源文件。'}</p>
      <p>冲突总数：{result.plan.conflictCount}</p>
      {result.plan.targets.map(target => <section key={target.kind} aria-label={`Codex ${target.kind} 配置目标`}>
        <h4>{target.kind === 'instructions' ? '个性化指令' : '模型与设置配置'}</h4>
        <p className="agent-path">{target.path}</p>
        <p>文件状态：{target.state === 'absent' ? '不存在' : '已存在'}；计划：{actions[target.action]}。</p>
        <p>冲突数：{target.conflictCount}</p>
        {target.conflictCodes.length > 0 && <ul>{target.conflictCodes.map(code => <li key={code}>{conflicts[code]}</li>)}</ul>}
      </section>)}
      <p>仅展示固定类别与数量，不显示配置值、动态键或密钥。计划不是跨文件原子快照，文件变化后应重新预览。应用功能尚未开放。</p>
    </section>}
  </section>;
}
