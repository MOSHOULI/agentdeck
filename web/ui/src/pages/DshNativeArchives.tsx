import {useEffect, useRef, useState} from 'react';
import * as bridge from '../bridge';
import {dshNativeEvidence, dshNativePackages, requireDshNativePackage, type DshNativePackage, type DshNativeVerification} from '../dshNativeContract';
import {useWorkspaceActivityStore} from '../stores/workspaceActivity';

export function DshNativeArchives() {
  const [selected, setSelected] = useState<DshNativePackage | ''>('');
  const [path, setPath] = useState('');
  const [result, setResult] = useState<DshNativeVerification | null>(null);
  const [error, setError] = useState(false);
  const [busy, setBusy] = useState(false);
  const mounted = useRef(false), running = useRef(false), generation = useRef(0);
  const native = bridge.runtimeMode() === 'native';
  const active = useWorkspaceActivityStore(s => Object.values(s.busy).some(Boolean));
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; generation.current++; };
  }, []);
  const invalidate = () => { generation.current++; setResult(null); setError(false); };
  const verify = async () => {
    if (!native || !selected || !path.trim() || running.current || Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean)) return;
    invalidate();
    const request = generation.current;
    const owner = Symbol('dsh-native-archive-read');
    running.current = true;
    setBusy(true);
    useWorkspaceActivityStore.getState().setActivity('dsh', owner, true);
    try {
      const next = await bridge.verifyDshNativePackage(selected, path);
      if (mounted.current && generation.current === request) setResult(next);
    } catch {
      if (mounted.current && generation.current === request) setError(true);
    } finally {
      running.current = false;
      useWorkspaceActivityStore.getState().setActivity('dsh', owner, false);
      if (mounted.current) setBusy(false);
    }
  };
  return <section className="panel" aria-label="DSH 原生依赖静态校验">
    <h2>固定原生依赖归档（不是安装）</h2>
    <p>显式选择一个已审阅的固定包；不会根据本机自动选择平台。仅静态核验，不下载、不解包到磁盘、不执行脚本或加载原生模块。</p>
    <label htmlFor="dsh-native-package">DSH 原生依赖包</label>
    <select id="dsh-native-package" className="control" value={selected} disabled={!native || (active && !busy)} onChange={e => {
      const value = e.target.value;
      if (value !== '') requireDshNativePackage(value);
      invalidate(); setSelected(value); setPath('');
    }}>
      <option value="">请选择固定包</option>
      {dshNativePackages.map(id => <option key={id} value={id}>{id} · {dshNativeEvidence[id].version}</option>)}
    </select>
    {selected && <p>{`只接受归档：${dshNativeEvidence[selected].archive_name}；精确长度：${dshNativeEvidence[selected].bytes} 字节。`}</p>}
    <label htmlFor="dsh-native-archive">DSH 原生依赖归档绝对路径</label>
    <input id="dsh-native-archive" className="control" value={path} maxLength={4096} autoComplete="off" spellCheck={false} disabled={!native || (active && !busy)} onChange={e => { invalidate(); setPath(e.target.value); }}/>
    <button id="dsh-native-verify" type="button" disabled={!native || busy || active || !selected || !path.trim()} onClick={() => { void verify(); }}>只读校验 DSH 原生依赖归档</button>
    <button id="dsh-native-clear" type="button" disabled={active && !busy} onClick={() => { invalidate(); setSelected(''); setPath(''); }}>清除 DSH 原生依赖选择与结果</button>
    {busy && <button id="dsh-native-cancel" type="button" onClick={invalidate}>取消 DSH 原生依赖结果</button>}
    {busy && <p role="status">正在处理。修改选择、清除或取消仅废弃结果，不停止底层读取；读取结束前继续保持忙碌。</p>}
    {!native && <p>仅原生桌面后端可用；浏览器与演示模式不会读取或模拟原生依赖校验成功。</p>}
    {error && <p id="dsh-native-error" role="alert">DSH 原生依赖归档未确认。请检查固定包、精确文件名、摘要与读取边界；不安全文件或不支持的主机失败关闭，没有安装或运行结论。</p>}
    {result && <section id="dsh-native-result" aria-label="DSH 原生依赖归档校验结果">
      <p role="status">所选原生依赖归档静态校验通过。</p>
      <p>{`包：${result.package_id}（${result.package} ${result.version}）`}</p>
      <p>{`归档：${result.archive_name}；已读取字节：${result.bytes}；普通成员数：${result.regular_member_count}`}</p>
      <p style={{overflowWrap: 'anywhere'}}>{`SHA256：${result.sha256}`}</p>
      <p style={{overflowWrap: 'anywhere'}}>{`SHA512 SRI：${result.integrity}`}</p>
      <p>固定摘要、成员与已审阅声明及来源快照一致。依赖闭包未完成、依赖未解析、不可据此安装、未执行生命周期脚本、未加载原生代码、未验证运行或独立签名，平台兼容性未验证。</p>
    </section>}
    <p>只读核验按固定上限在内存中解压并检查 TAR，不提取文件。只确认所选归档，不表示整个 DSH 可安装或可用。摘要匹配不是独立签名或执行授权。访问时间可能改变；无法完全防止同用户竞争，文件系统 I/O 无硬超时。</p>
  </section>;
}
