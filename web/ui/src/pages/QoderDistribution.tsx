import { QoderDeploymentInspection } from "./QoderDeploymentInspection";
import { qoderReleases, type QoderRelease } from '../qoderReleases';
import { QoderPrivateDeployment } from './QoderPrivateDeployment';
import { QoderScopeInspection } from './QoderScopeInspection';
import { useEffect, useRef, useState } from 'react';
import * as bridge from '../bridge';
import { qoderTargets, qoderCacheLabels, qoderCacheRecoveryGuidance } from '../qoderContract';
import type { QoderPlan, QoderVerification, QoderCacheObservation, QoderWriteOperation, QoderWriteResult } from '../qoderContract';
import { useWorkspaceActivity, useWorkspaceActivityStore } from '../stores/workspaceActivity';
import './DshInventory.css';
import './QoderDistribution.css';
export function QoderDistribution({ active = true }: { active?: boolean }) {
  const [release, setRelease] = useState<QoderRelease>('1.1.85');
  const [operation, setOperation] = useState<QoderWriteOperation>('stage');
  const [source, setSource] = useState('');
  const [destination, setDestination] = useState('');
  const [confirmation, setConfirmation] = useState<{ release: QoderRelease; operation: QoderWriteOperation; source: string; destination: string } | null>(null);
  const [attempt, setAttempt] = useState<{ release: QoderRelease; operation: QoderWriteOperation; source: string; destination: string; outcome: 'pending' | 'complete' | 'unconfirmed' } | null>(null);
  const [diagnosedPath, setDiagnosedPath] = useState<string | null>(null);
  const [writing, setWriting] = useState(false);
  const [written, setWritten] = useState<QoderWriteResult | null>(null);
  const [writeError, setWriteError] = useState(false);
  const [target, setTarget] = useState('');
  const [path, setPath] = useState('');
  const [cachePath, setCachePath] = useState('');
  const [cacheReport, setCacheReport] = useState<QoderCacheObservation | null>(null);
  const [cacheError, setCacheError] = useState<false | 'unsupported' | 'failed'>(false);
  const otherBusy = useWorkspaceActivityStore(state => Object.values(state.busy).some(Boolean));
  const [plan, setPlan] = useState<QoderPlan | null>(null);
  const [verified, setVerified] = useState<QoderVerification | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const epoch = useRef(0); const running = useRef(false); const mounted = useRef(false);
  const native = bridge.runtimeMode() === 'native';
  useWorkspaceActivity('qoder', busy);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; epoch.current++; }; }, []);
  const clearResult = () => { epoch.current++; setConfirmation(null); setWritten(null); setWriteError(false); setPlan(null); setVerified(null); setError(false); setCacheReport(null); setCacheError(false); setDiagnosedPath(null); };
  // Hidden workspaces remain mounted. Navigation discards selections/results,
  // never cancels a submitted native write or erases its historical settlement.
  useEffect(() => { if (!active) clearResult(); }, [active]);
  const run = async (verify: boolean) => {
    if (!native || !active || running.current || Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean) || !target || (verify && !path.trim())) return;
    clearResult(); const request = epoch.current; running.current = true; setBusy(true);
    try {
      if (verify) { const value = await bridge.verifyQoderPackage(target, path, release); if (mounted.current && epoch.current === request) { setVerified(value); setPlan(value.plan); } }
      else { const value = await bridge.qoderDistributionPlan(target, release); if (mounted.current && epoch.current === request) setPlan(value); }
    } catch { if (mounted.current && epoch.current === request) setError(true); }
    finally { running.current = false; if (mounted.current) setBusy(false); }
  };
  const inspectCache = async () => {
    if (!native || !active || running.current || !cachePath.trim() || Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean)) return;
    const requestedPath = cachePath;
    clearResult(); const request = epoch.current; running.current = true; setBusy(true);
    try { const value = await bridge.inspectQoderCache(requestedPath, release); if (mounted.current && epoch.current === request) { setCacheReport(value); setDiagnosedPath(requestedPath); } }
    catch (error) { if (mounted.current && epoch.current === request) { setCacheError(error === 'QODER_CACHE_UNSUPPORTED_HOST' ? 'unsupported' : 'failed'); setDiagnosedPath(requestedPath); } }
    finally { running.current = false; if (mounted.current) setBusy(false); }
  };
  const writeCache = async () => {
    if (!native || !active || running.current || !confirmation || Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean)) return;
    const approved = { ...confirmation };
    setAttempt({ ...approved, outcome: 'pending' });
    clearResult(); const request = epoch.current; running.current = true; setBusy(true); setWriting(true);
    const owner = Symbol('qoder-cache-write');
    // This publisher outlives component unmount and is released only when IPC settles.
    useWorkspaceActivityStore.getState().setActivity('qoder', owner, true);
    try {
      const value = await bridge.writeQoderCache(approved.operation, approved.source, approved.destination, true, approved.release);
      if (mounted.current) { setAttempt({ ...approved, outcome: 'complete' }); if (epoch.current === request) setWritten(value); }
    } catch { if (mounted.current) { setAttempt({ ...approved, outcome: 'unconfirmed' }); if (epoch.current === request) setWriteError(true); } }
    finally {
      running.current = false;
      useWorkspaceActivityStore.getState().setActivity('qoder', owner, false);
      if (mounted.current) { setBusy(false); setWriting(false); }
    }
  };
  return <div className="dsh-workspace">
    <header className="workspace-heading"><div><p className="eyebrow">QODER DISTRIBUTION</p><h1>Qoder 分发与校验</h1><p className="muted">固定 1.1.85 / 1.1.86 分发清单，不查询滚动最新版本，不自动判断本机平台。</p></div><span className="status-badge">显式受控操作</span></header>
    <div className="notice">分发计划与诊断只读；Linux x86_64 可明确确认后写入全新受控缓存。不下载、不执行、不激活、不修改 PATH 或配置；私有可执行部署须在独立区域预览确认。</div>
    {!native && <p role="status">仅桌面原生后端可用；浏览器及演示模式不读取归档，也不模拟校验成功。</p>}
    <section className="panel" aria-label="Qoder 分发选择">
      <label htmlFor="qoder-release">Qoder 已审阅版本</label>
      <select id="qoder-release" value={release} disabled={!native || writing || (otherBusy && !busy)} onChange={e => { clearResult(); setRelease(e.target.value as QoderRelease); }}>
        {qoderReleases.map(value => <option key={value} value={value}>{value}</option>)}
      </select>
      <p>版本明确选择，不自动升级或迁移；旧缓存与部署原样保留。新版本须使用相同版本的来源与全新目标目录。</p>
      <fieldset className="qoder-targets" disabled={!native || writing}>
        <legend>明确分发目标（不是本机自动检测）</legend>
        <div className="qoder-target-grid">
          {qoderTargets.map(id => <label className="qoder-target-option" key={id}>
            <input type="radio" name="qoder-target" value={id} checked={target === id} onChange={() => { clearResult(); setTarget(id); }} />
            <span>{id}</span>
          </label>)}
        </div>
      </fieldset>
      <p aria-live="polite">{`当前分发目标：${target || '未选择'}`}</p>
      <label htmlFor="qoder-archive">Qoder 官方归档绝对路径</label>
      <input id="qoder-archive" className="control" value={path} maxLength={4096} disabled={!native || writing} autoComplete="off" spellCheck={false} onChange={event => { clearResult(); setPath(event.target.value); }} />
      <p className="muted">须使用该目标的官方文件名；每包最多 256 MiB。读取边界仅审阅 Windows 与 Linux x86_64/ARM64；macOS 等主机的计划可查看，但文件校验失败关闭。哈希一致不代表运行兼容。</p>
      <div className="dsh-form-actions">
        <button type="button" className="button" disabled={!native || !active || busy || otherBusy || !target} onClick={() => { void run(false); }}>查看 Qoder 分发计划</button>
        <button type="button" className="button" disabled={!native || !active || busy || otherBusy || !target || !path.trim()} onClick={() => { void run(true); }}>只读校验 Qoder 归档</button>
        <button type="button" className="button" disabled={writing} onClick={() => { clearResult(); setTarget(''); setPath(''); setCachePath(''); setAttempt(null); }}>清除 Qoder 选择与结果</button>
        {busy && !writing && <button type="button" className="button" onClick={clearResult}>取消 Qoder 结果</button>}
      </div>
    </section>
    <section className="panel" aria-label="Qoder 缓存只读诊断">
      <h2>缓存状态与保留计划</h2>
      <p>仅 Linux x86_64 后端支持所选固定版本 baseline 缓存诊断；Windows、macOS 等主机失败关闭，不模拟成功。</p>
      <label htmlFor="qoder-cache">Qoder 缓存绝对目录</label>
      <input id="qoder-cache" className="control" value={cachePath} maxLength={4096} disabled={!native || writing} autoComplete="off" spellCheck={false} onChange={event => { clearResult(); setCachePath(event.target.value); }} />
      <button type="button" className="button" disabled={!native || !active || busy || otherBusy || !cachePath.trim()} onClick={() => { void inspectCache(); }}>只读诊断 Qoder 缓存</button>
      <p>不创建锁、不删除、不修复、不激活、不执行。读取可能更新访问时间；缓存契约匹配不是历史所有权证明或清理授权。</p>
    </section>
    {cacheError && <p>{`未获得诊断结论的目录：${diagnosedPath}`}</p>}
    {cacheError && <p role="alert">{cacheError === 'unsupported' ? '当前主机不支持 Qoder 缓存诊断；仅支持 Linux x86_64，没有读取缓存或获得清理授权。' : 'Qoder 缓存诊断未完成；没有获得完整性或清理授权，请检查明确目录与读取边界；目录可能缺失或不可读，不能推断为空、安全或可删除。'}</p>}
    {cacheReport && <section className="panel" aria-label="Qoder 缓存诊断结果">
      <p>{`本次只读诊断目录：${diagnosedPath}`}</p>
      <p role="status">{qoderCacheLabels[cacheReport.state]}</p>
      <p>{qoderCacheRecoveryGuidance[cacheReport.state]}</p>
      <p>{cacheReport.recoveryPlan === 'retain_cache_no_action' ? '保留缓存，不执行操作。' : '保留现场，人工检查。'}</p>
      <p>任何清理均须独立人工复核。本报告不授权删除、不表示安装或运行成功，也不是原子快照。</p>
    </section>}
    {busy && !writing && <p role="status">正在处理。取消或修改输入仅丢弃结果，不中止底层读取；完成前不会开始另一个请求。</p>}
    {error && <p role="alert">未能确认 Qoder 分发计划或包摘要。请检查目标、官方文件名、文件权限和支持的读取平台；没有获得校验成功结论。</p>}
    {plan && <section className="panel" aria-label="Qoder 分发计划结果">
      <h2>固定分发计划：{plan.target}</h2><p>版本：{plan.version}</p><p>{`官方归档名：${plan.archiveName}`}</p>
      <p style={{ overflowWrap: 'anywhere' }}>官方包地址（仅展示，不自动下载）：<code>{plan.url}</code></p>
      <p style={{ overflowWrap: 'anywhere' }}>官方包 SHA256：<code>{plan.sha256}</code></p><p style={{ overflowWrap: 'anywhere' }}>已审阅清单 SHA256：<code>{plan.manifestHash}</code></p>
      <p>运行时声明：{plan.runtime}；变体：{plan.variant}。</p><p>{plan.requirements}</p>
      <p>未安装、未运行、未验证独立签名；选择目标不证明该平台可用。清单来源摘要不等于独立真实性证明。</p>
    </section>}
    {verified && <section className="panel" aria-label="Qoder 归档校验结果"><p role="status">归档与固定官方 SHA256 一致，仅证明所读包摘要匹配。</p><p>{`已读取字节：${verified.bytes}`}</p><p>这不是安装成功、运行兼容或执行授权。文件可在检查后改变，读取不是原子事务，访问时间可能更新。</p></section>}
    <section className="panel" aria-label="Qoder 受控缓存写入">
      <h2>创建受控缓存（不是安装）</h2>
      <p>仅 Linux x86_64，所选固定版本 / linux-amd64-baseline。来源必须是固定官方归档或已完成的受控暂存目录。目标必须是不存在的全新绝对目录，父目录已存在。</p>
      <fieldset disabled={!native || !active || busy || otherBusy}>
        <legend>缓存操作</legend>
        <button type="button" onClick={() => { clearResult(); setOperation('stage'); }}>选择暂存归档</button>
        <button type="button" onClick={() => { clearResult(); setOperation('unpack'); }}>选择解包缓存</button>
        <p>{`缓存操作：${operation === 'stage' ? '暂存归档' : '解包缓存'}`}</p>
        <label htmlFor="qoder-write-source">Qoder 缓存写入来源绝对路径</label>
        <input id="qoder-write-source" value={source} maxLength={4096} onChange={e => { clearResult(); setSource(e.target.value); }} />
        <label htmlFor="qoder-write-destination">Qoder 全新缓存绝对目录</label>
        <input id="qoder-write-destination" value={destination} maxLength={4096} onChange={e => { clearResult(); setDestination(e.target.value); }} />
        <button type="button" disabled={!source.startsWith('/') || !destination.startsWith('/')} onClick={() => { clearResult(); setConfirmation({ release, operation, source, destination }); }}>预览 Qoder 缓存写入</button>
      </fieldset>
      <p>不覆盖现有目录、不赋执行权限、不执行 Qoder。失败可能留下部分私有文件；保留现场人工检查，不原地重试或自动清理。记录不构成执行或删除授权。</p>
      {active && confirmation && <section role="dialog" aria-label="确认 Qoder 缓存写入">
        <p>{`操作：${confirmation.operation === 'stage' ? '暂存归档' : '解包缓存'}；固定目标：linux-amd64-baseline；版本：${confirmation.release}`}</p>
        <p>{`来源：${confirmation.source}`}</p><p>{`全新目录：${confirmation.destination}`}</p>
        <button type="button" disabled={!active || busy || otherBusy} onClick={() => { void writeCache(); }}>确认创建 Qoder 缓存</button>
        <button type="button" onClick={() => setConfirmation(null)}>取消 Qoder 写入确认</button>
      </section>}
      {attempt && <section aria-label="Qoder 上次提交记录">
        <h3>上次实际提交（仅当前会话记录）</h3>
        <p>{`已提交版本：${attempt.release}`}</p>
        <p>{`已提交操作：${attempt.operation === 'stage' ? '暂存归档' : '解包缓存'}`}</p>
        <p>{`已提交来源：${attempt.source}`}</p><p>{`已提交目标：${attempt.destination}`}</p>
        <p>{attempt.outcome === 'pending' ? '仍在等待真实结算。' : attempt.outcome === 'complete' ? '写入曾报告完成；这不是当前目录状态或执行授权。' : '写入未确认完成；目标可能不存在或留下部分文件，不原地重试。'}</p>
        <button type="button" disabled={!native || !active || busy || otherBusy || attempt.outcome === 'pending'} onClick={() => { clearResult(); setCachePath(attempt.destination); setRelease(attempt.release); }}>将上次提交目标填入只读诊断</button>
        <p>此按钮仅填写上次实际提交的目标，不读取或修改文件。请再明确点击只读诊断；后续编辑写入表单不会改写这条记录，清除选择与结果会移除它。</p>
      </section>}
      {writing && <p role="status">正在写入受控缓存；不可取消或修改输入，隐藏工作区不会停止操作。等待真实结算，失败可能残留部分文件。</p>}
      {writeError && <p role="alert">Qoder 缓存写入未确认完成。仅支持 Linux x86_64；目标不可存在，不安全来源会被拒绝。可能留下部分私有目录，请保留现场，不原地重试。</p>}
      {written && <section aria-label="Qoder 缓存写入结果"><p role="status">{qoderCacheLabels[written.state]}</p><p>未安装、未运行；没有删除授权。可另行只读诊断明确目标目录。</p></section>}
    </section>
    <QoderPrivateDeployment active={active} release={release} />
    <QoderDeploymentInspection active={active} release={release} />
    <QoderScopeInspection />
  </div>;
}
