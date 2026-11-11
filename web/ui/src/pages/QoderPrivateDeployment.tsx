import { qoderPayloads, type QoderRelease } from '../qoderReleases';
import { useEffect, useRef, useState } from 'react';
import * as bridge from '../bridge';
import { qoderDeploymentTarget, validQoderDeploymentPath } from '../qoderDeploymentContract';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';

type Selection = { release: QoderRelease; source: string; destination: string };
type Preview = Selection & { token: string };
type Attempt = Selection & { state: 'pending' | 'complete' | 'unconfirmed' };
export function QoderPrivateDeployment({ active = true, release = '1.1.85' }: { active?: boolean; release?: QoderRelease }) {
  const [source, setSource] = useState('');
  const [destination, setDestination] = useState('');
  const [preview, setPreview] = useState<Preview | null>(null);
  const [attempt, setAttempt] = useState<Attempt | null>(null);
  const [busy, setBusy] = useState<'preview' | 'deploy' | null>(null);
  const [error, setError] = useState<'preview' | 'unsupported' | null>(null);
  const mounted = useRef(false); const generation = useRef(0); const running = useRef(false);
  const visible = useRef(active); visible.current = active;
  const workspaceBusy = useWorkspaceActivityStore(state => Object.values(state.busy).some(Boolean));
  const native = bridge.runtimeMode() === 'native';
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current++; }; }, []);
  useEffect(() => {
    if (!active) { generation.current++; setPreview(null); setError(null); }
  }, [active]);
  useEffect(() => { generation.current++; setPreview(null); setError(null); }, [release]);
  const discard = () => { generation.current++; setPreview(null); setError(null); };
  const start = (kind: 'preview' | 'deploy') => {
    if (!native || !visible.current || running.current || Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean)) return null;
    running.current = true;
    const owner = Symbol('qoder-private-deployment');
    useWorkspaceActivityStore.getState().setActivity('qoder', owner, true);
    setBusy(kind);
    return owner;
  };
  const finish = (owner: symbol) => {
    running.current = false;
    useWorkspaceActivityStore.getState().setActivity('qoder', owner, false);
    if (mounted.current) setBusy(null);
  };
  const inspect = async () => {
    if (!validQoderDeploymentPath(source) || !validQoderDeploymentPath(destination)) return;
    const owner = start('preview'); if (!owner) return;
    discard(); const request = generation.current;
    const selected = { release, source, destination };
    try {
      const value = await bridge.previewQoderDeployment(selected.source, selected.destination, selected.release);
      if (mounted.current && visible.current && generation.current === request) setPreview({ ...selected, token: value.token });
    } catch (e) {
      if (mounted.current && visible.current && generation.current === request) setError(e === 'QODER_DEPLOY_UNSUPPORTED_HOST' ? 'unsupported' : 'preview');
    } finally { finish(owner); }
  };
  const deploy = async () => {
    if (!preview || preview.release !== release) return;
    const owner = start('deploy'); if (!owner) return;
    const approved = { ...preview };
    // Consume this confirmation before IPC starts; never automatically replay it.
    discard(); setAttempt({ release: approved.release, source: approved.source, destination: approved.destination, state: 'pending' });
    try {
      await bridge.deployQoderPrivate(approved.source, approved.destination, approved.token, true, approved.release);
      if (mounted.current) setAttempt({ release: approved.release, source: approved.source, destination: approved.destination, state: 'complete' });
    } catch {
      if (mounted.current) setAttempt({ release: approved.release, source: approved.source, destination: approved.destination, state: 'unconfirmed' });
    } finally { finish(owner); }
  };
  return <section className="panel" aria-label="Qoder 私有可执行部署">
    <h2>私有可执行文件部署（不是激活或运行）</h2>
    <p>仅 Linux x86_64；所选固定版本 / linux-amd64-baseline，不代表上游最新版本。Windows、macOS 等主机拒绝，不模拟成功。</p>
    <p>来源须为完整受控解包缓存；目标须是全新绝对目录，父目录已存在、属于当前用户且权限为 0700。将复制官方固定载荷并将新文件设为 0700；缓存保持不变。</p>
    <p>不下载、不执行 Qoder、不改 PATH 或配置。失败可能留下已可执行的部分文件；保留现场人工检查，不原地重试、自动清理或回滚。权限设置不证明运行、登录或兼容性。</p>
    {!native && <p>仅桌面原生后端可用；演示模式不创建可执行文件。</p>}
    <label htmlFor="qoder-deploy-source">Qoder 已解包缓存绝对目录</label>
    <input id="qoder-deploy-source" className="control" value={source} disabled={!native || !!busy} maxLength={4096} autoComplete="off" spellCheck={false} onChange={e => { discard(); setSource(e.target.value); }} />
    <label htmlFor="qoder-deploy-destination">Qoder 全新私有部署绝对目录</label>
    <input id="qoder-deploy-destination" className="control" value={destination} disabled={!native || !!busy} maxLength={4096} autoComplete="off" spellCheck={false} onChange={e => { discard(); setDestination(e.target.value); }} />
    <div className="dsh-form-actions">
      <button type="button" disabled={!native || !active || !!busy || workspaceBusy || !validQoderDeploymentPath(source) || !validQoderDeploymentPath(destination)} onClick={() => { void inspect(); }}>预览 Qoder 私有部署</button>
      <button type="button" disabled={busy === 'deploy'} onClick={() => { discard(); setSource(''); setDestination(''); setAttempt(null); }}>清除 Qoder 部署选择与结果</button>
      {busy === 'preview' && <button type="button" onClick={discard}>取消 Qoder 部署预览结果</button>}
    </div>
    {busy === 'preview' && <p role="status">正在只读核验源与目标；取消仅丢弃预览，不中断底层读取，完成前不开始另一操作。</p>}
    {error && <p role="alert">{error === 'unsupported' ? '当前主机不支持 Qoder 私有部署；仅支持 Linux x86_64。没有执行部署。' : 'Qoder 部署预览未完成；源或目标不受支持、不安全或已变化。没有获得部署确认。'}</p>}
    {preview && preview.release === release && <section role="dialog" aria-label="确认 Qoder 私有可执行部署">
      <h3>明确确认创建可执行文件</h3>
      <p>{`固定版本：${preview.release}；目标：${qoderDeploymentTarget}`}</p>
      <p>{`已审核部署来源：${preview.source}`}</p><p>{`已审核部署目标：${preview.destination}`}</p>
      <p>{`载荷字节：${qoderPayloads[preview.release].bytes}；新文件权限：0700`}</p><p>{`载荷 SHA256：${qoderPayloads[preview.release].sha256}`}</p>
      <p>这次确认仅适用于上述源、全新目标与本次预览；变化后必须重新预览。不激活或执行，失败可能残留可执行文件。</p>
      <button type="button" disabled={!!busy || workspaceBusy || !active} onClick={() => { void deploy(); }}>确认部署 Qoder 可执行文件</button>
      <button type="button" onClick={discard}>取消 Qoder 部署确认</button>
    </section>}
    {attempt && <section aria-label="Qoder 私有部署提交记录">
      <p>{`实际提交版本：${attempt.release}`}</p>
      <p>{`实际提交来源：${attempt.source}`}</p><p>{`实际提交目标：${attempt.destination}`}</p>
      <p role={attempt.state === 'unconfirmed' ? 'alert' : 'status'}>{attempt.state === 'pending' ? 'Qoder 私有部署正在进行；不可取消，隐藏页面不会停止，等待真实结算。' : attempt.state === 'complete' ? 'Qoder 私有可执行文件部署已完成；未激活、未运行。' : 'Qoder 部署未确认完成；目标可能已有可执行文件。请保留现场，不原地重试或自动清理。'}</p>
      <p>仅本次会话提交记录，不是当前文件状态、所有权、签名或清理授权。关闭应用后不保留。</p>
    </section>}
  </section>;
}
