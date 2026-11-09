import { useEffect, useRef, useState } from 'react';
import * as bridge from '../bridge';
import { qoderTargets } from '../qoderContract';
import type { QoderPlan, QoderVerification } from '../qoderContract';
import { useWorkspaceActivity } from '../stores/workspaceActivity';
import './DshInventory.css';
export function QoderDistribution() {
  const [target, setTarget] = useState('');
  const [path, setPath] = useState('');
  const [plan, setPlan] = useState<QoderPlan | null>(null);
  const [verified, setVerified] = useState<QoderVerification | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const epoch = useRef(0); const running = useRef(false); const mounted = useRef(false);
  const native = bridge.runtimeMode() === 'native';
  useWorkspaceActivity('qoder', busy);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; epoch.current++; }; }, []);
  const clearResult = () => { epoch.current++; setPlan(null); setVerified(null); setError(false); };
  const run = async (verify: boolean) => {
    if (!native || running.current || !target || (verify && !path.trim())) return;
    clearResult(); const request = epoch.current; running.current = true; setBusy(true);
    try {
      if (verify) { const value = await bridge.verifyQoderPackage(target, path); if (mounted.current && epoch.current === request) { setVerified(value); setPlan(value.plan); } }
      else { const value = await bridge.qoderDistributionPlan(target); if (mounted.current && epoch.current === request) setPlan(value); }
    } catch { if (mounted.current && epoch.current === request) setError(true); }
    finally { running.current = false; if (mounted.current) setBusy(false); }
  };
  return <div className="dsh-workspace">
    <header className="workspace-heading"><div><p className="eyebrow">QODER DISTRIBUTION</p><h1>Qoder 分发与校验</h1><p className="muted">固定 1.1.85 分发清单，不查询滚动最新版本，不自动判断本机平台。</p></div><span className="status-badge">只读</span></header>
    <div className="notice">这里只提供分发计划和显式本地归档哈希校验，不下载、不暂存、不解包、不执行、不安装、不修改 PATH 或配置。</div>
    {!native && <p role="status">仅桌面原生后端可用；浏览器及演示模式不读取归档，也不模拟校验成功。</p>}
    <section className="panel" aria-label="Qoder 分发选择">
      <label htmlFor="qoder-target">明确分发目标（不是本机自动检测）</label>
      <select id="qoder-target" className="control" value={target} disabled={!native} onChange={event => { clearResult(); setTarget(event.target.value); }}>
        <option value="">请选择目标</option>{qoderTargets.map(id => <option value={id} key={id}>{id}</option>)}
      </select>
      <p aria-live="polite">{`当前分发目标：${target || '未选择'}`}</p>
      <label htmlFor="qoder-archive">Qoder 官方归档绝对路径</label>
      <input id="qoder-archive" className="control" value={path} maxLength={4096} disabled={!native} autoComplete="off" spellCheck={false} onChange={event => { clearResult(); setPath(event.target.value); }} />
      <p className="muted">须使用该目标的官方文件名；每包最多 256 MiB。读取边界仅审阅 Windows 与 Linux x86_64/ARM64；macOS 等主机的计划可查看，但文件校验失败关闭。哈希一致不代表运行兼容。</p>
      <div className="dsh-form-actions">
        <button type="button" className="button" disabled={!native || busy || !target} onClick={() => { void run(false); }}>查看 Qoder 分发计划</button>
        <button type="button" className="button" disabled={!native || busy || !target || !path.trim()} onClick={() => { void run(true); }}>只读校验 Qoder 归档</button>
        <button type="button" className="button" onClick={() => { clearResult(); setTarget(''); setPath(''); }}>清除 Qoder 选择与结果</button>
        {busy && <button type="button" className="button" onClick={clearResult}>取消 Qoder 结果</button>}
      </div>
    </section>
    {busy && <p role="status">正在处理。取消或修改输入仅丢弃结果，不中止底层读取；完成前不会开始另一个请求。</p>}
    {error && <p role="alert">未能确认 Qoder 分发计划或包摘要。请检查目标、官方文件名、文件权限和支持的读取平台；没有获得校验成功结论。</p>}
    {plan && <section className="panel" aria-label="Qoder 分发计划结果">
      <h2>固定分发计划：{plan.target}</h2><p>版本：{plan.version}</p><p>{`官方归档名：${plan.archiveName}`}</p>
      <p style={{ overflowWrap: 'anywhere' }}>官方包地址（仅展示，不自动下载）：<code>{plan.url}</code></p>
      <p style={{ overflowWrap: 'anywhere' }}>官方包 SHA256：<code>{plan.sha256}</code></p><p style={{ overflowWrap: 'anywhere' }}>已审阅清单 SHA256：<code>{plan.manifestHash}</code></p>
      <p>运行时声明：{plan.runtime}；变体：{plan.variant}。</p><p>{plan.requirements}</p>
      <p>未安装、未运行、未验证独立签名；选择目标不证明该平台可用。清单来源摘要不等于独立真实性证明。</p>
    </section>}
    {verified && <section className="panel" aria-label="Qoder 归档校验结果"><p role="status">归档与固定官方 SHA256 一致，仅证明所读包摘要匹配。</p><p>{`已读取字节：${verified.bytes}`}</p><p>这不是安装成功、运行兼容或执行授权。文件可在检查后改变，读取不是原子事务，访问时间可能更新。</p></section>}
    <p className="muted">受控暂存与严格解包目前仅是 Linux x86_64 上固定 baseline 的独立 CLI 能力，桌面未开放。它们也不会自动安装或执行 Qoder。</p>
  </div>;
}
