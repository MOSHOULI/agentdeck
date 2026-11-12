import { useEffect, useRef, useState } from 'react';
import * as bridge from '../bridge';
import { validQoderDeploymentPath } from '../qoderDeploymentContract';
import type { DeploymentInspectionState } from '../qoderDeploymentInspectionContract';
import type { QoderRelease } from '../qoderReleases';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
const labels: Record<DeploymentInspectionState, string> = {
  payload_verified: '私有部署载荷与固定版本匹配（未运行）', missing: '部署目录不存在',
  incomplete: '固定成员不完整，内容未验证；保留现场人工检查', unsupported_or_changed: '部署不受支持或已变化；不能确认完整',
};
export function QoderDeploymentInspection({ active, release }: { active: boolean; release: QoderRelease }) {
  const [path, setPath] = useState(''); const [result, setResult] = useState<DeploymentInspectionState | null>(null);
  const [error, setError] = useState(false); const [busy, setBusy] = useState(false);
  const epoch = useRef(0); const running = useRef(false); const mounted = useRef(false); const visible = useRef(active); visible.current = active;
  const workspaceBusy = useWorkspaceActivityStore(s => Object.values(s.busy).some(Boolean));
  const native = bridge.runtimeMode() === 'native';
  const discard = () => { epoch.current++; setResult(null); setError(false); };
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; epoch.current++; }; }, []);
  useEffect(() => { discard(); }, [active, release]);
  async function inspect() {
    if (!native || !visible.current || running.current || !validQoderDeploymentPath(path) || Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean)) return;
    discard(); const request = epoch.current; running.current = true; setBusy(true);
    const owner = Symbol('qoder-deployment-inspection'); useWorkspaceActivityStore.getState().setActivity('qoder', owner, true);
    try {
      const report = await bridge.inspectQoderDeployment(path, release);
      if (mounted.current && visible.current && request === epoch.current) setResult(report.state);
    } catch {
      if (mounted.current && visible.current && request === epoch.current) setError(true);
    } finally {
      running.current = false; useWorkspaceActivityStore.getState().setActivity('qoder', owner, false);
      if (mounted.current) setBusy(false);
    }
  }
  return <section className="panel" aria-label="Qoder 已部署目录只读检查">
    <h2>重新检查私有部署（只读）</h2>
    <p>显式选择部署目录，核验当前所选固定版本的载荷、记录与权限。不是运行、激活、签名或所有权证明；不授权删除、修复或覆盖。仅 Linux x86_64。</p>
    <label htmlFor="qoder-inspect-deployment">Qoder 已部署绝对目录</label>
    <input id="qoder-inspect-deployment" className="control" value={path} maxLength={4096} autoComplete="off" spellCheck={false} disabled={!native} onChange={e => { discard(); setPath(e.target.value); }} />
    <button type="button" disabled={!native || !active || busy || workspaceBusy || !validQoderDeploymentPath(path)} onClick={() => { void inspect(); }}>只读检查 Qoder 私有部署</button>
    <button type="button" onClick={() => { discard(); setPath(''); }}>清除 Qoder 部署检查</button>
    {busy && <><button type="button" onClick={discard}>取消 Qoder 部署检查结果</button><p role="status">正在只读检查。编辑或取消仅丢弃结果，不终止读取；完成前不开始另一操作。</p></>}
    {!native && <p>演示模式不读取本地部署，也不模拟成功。</p>}
    {error && <p role="alert">无法安全检查当前部署；未执行、修复或删除任何文件。</p>}
    {result && <section data-testid="qoder-deployment-inspection-result" data-state={result}>
      <p>{labels[result]}</p><p>仅本次只读观察，文件仍可能变化。保留未知或未完成文件，人工检查；没有清理或执行授权。</p>
    </section>}
  </section>;
}
