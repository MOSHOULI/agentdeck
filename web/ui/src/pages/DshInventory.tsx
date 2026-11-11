import { useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import * as bridge from '../bridge';
import type { DshInventoryReport, DshInspectionResult } from '../bridge';
import type { DshIssue, DshManifestStatus } from '../dshContract';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
import { DshDistribution } from './DshDistribution';
import { DshNativeArchives } from './DshNativeArchives';
import { Icon } from '../components/Icon';
import './DshInventory.css';

const statusLabels: Record<DshManifestStatus, string> = {
  read: '已读取声明', missing: '文件缺失', corrupt: 'JSON 损坏', unreadable: '无法读取', unsupported: '不支持此输入',
};
const issueLabels: Record<DshIssue, string> = {
  path_must_be_absolute: '需要绝对路径', parent_traversal: '不接受父目录跳转', unsupported_path: '路径类型不受支持',
  other_selection_invalid: '另一选定路径无效，本文件未读取', path_limit: '路径超过限制', expected_package_manifest: '必须选择 package.json',
  missing: '未找到选定文件', permission_denied: '没有读取权限', io_failure: '文件读取失败',
  link_or_reparse_point: '链接或重解析点不受支持', not_directory: '路径中有非目录组件', not_regular_file: '不是普通文件',
  multiple_hard_links: '多个硬链接不受支持', file_changed: '读取时文件发生变化，请重试', file_too_large: '文件超过 64 KiB',
  invalid_json: 'JSON 格式无效', duplicate_json_key: 'JSON 包含重复键', json_limit: 'JSON 结构超过限制',
  unsupported_shape: '声明结构不受支持', unsupported_manifest_version: '声明格式版本不受支持',
  unsupported_metadata: '元数据不受支持', dependency_limit: '依赖数超过限制', bundle_limit: 'Bundle 选择数超过限制',
  platform_unsupported: '此平台尚无受支持的被动文件读取器',
};

function ManifestState({ value }: { value: DshInventoryReport['profile_manifest'] | DshInventoryReport['runtime_manifest'] }) {
  if (!value) return <p className="muted">未选择 Runtime manifest，未读取。</p>;
  return <p className="dsh-manifest-state"><span className="status-badge">{statusLabels[value.status]}</span>{value.issue && <span>{issueLabels[value.issue]}</span>}</p>;
}

export function DshInventory() {
  const mode = bridge.runtimeMode();
  const [profile, setProfile] = useState(mode === 'demo' ? '/synthetic-demo/profile' : '');
  const [runtime, setRuntime] = useState('');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<DshInspectionResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectionChanged, setSelectionChanged] = useState(false);
  const operation = useRef(false);
  const generation = useRef(0);
  const mounted = useRef(false);
  const workspaceBusy = useWorkspaceActivityStore(state => Object.values(state.busy).some(Boolean));

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; generation.current += 1; };
  }, []);

  const changeSelection = (field: 'profile' | 'runtime', value: string) => {
    generation.current += 1;
    (field === 'profile' ? setProfile : setRuntime)(value);
    setResult(null);
    setError(null);
    setSelectionChanged(true);
  };

  const inspect = async (event: FormEvent) => {
    event.preventDefault();
    if (operation.current || mode === 'unavailable' || !profile.trim() || Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean)) return;
    const owner = Symbol('dsh-profile-read');
    useWorkspaceActivityStore.getState().setActivity('dsh', owner, true);
    operation.current = true;
    const current = ++generation.current;
    setBusy(true);
    setResult(null);
    setError(null);
    setSelectionChanged(false);
    try {
      const next = await bridge.inspectDshProfile(profile, runtime.trim() ? runtime : null);
      if (mounted.current && current === generation.current) setResult(next);
    } catch {
      if (mounted.current && current === generation.current) {
        // Never echo an arbitrary backend exception, path, or manifest contents.
        setError('盘点未完成或响应不兼容。请确认绝对路径、package.json 文件名和桌面后端后重试。');
      }
    } finally {
      operation.current = false;
      useWorkspaceActivityStore.getState().setActivity('dsh', owner, false);
      if (mounted.current) setBusy(false);
    }
  };

  const report = result?.report;
  const metadata = report?.profile_manifest.metadata;
  const runtimeMetadata = report?.runtime_manifest?.metadata;
  return <div className="dsh-workspace">
    <header className="workspace-heading">
      <div><p className="eyebrow">PASSIVE MANIFEST INVENTORY</p><h1>DSH 声明盘点</h1><p className="muted">读取明确选择的 package.json，查看保存的声明。</p></div>
      <span className="status-badge"><Icon name="info" size={14} />只读盘点</span>
    </header>

    <div className="notice"><Icon name="info" /><span>这不是插件管理器。依赖声明和 Bundle 选择不证明软件已安装或正在运行。不会启动 DSH、扫描 HOME、读取凭据或修改配置。</span></div>
    {mode === 'demo' && <div className="notice notice-warning" role="status">合成示例模式：不会读取输入路径或任何本地文件。</div>}
    {mode === 'unavailable' && <div className="notice notice-warning" role="status">未连接桌面后端，无法读取本地文件。请在 AgentDeck 桌面应用中盘点。</div>}

    <form className="panel dsh-form" onSubmit={(event) => { void inspect(event); }} aria-label="DSH 盘点路径" aria-busy={busy}>
      <div className="dsh-field"><label htmlFor="dsh-profile">Profile 目录（绝对路径）</label>
        <input id="dsh-profile" className="control" type="text" value={profile} maxLength={4096} autoComplete="off" spellCheck={false}
          placeholder="C:\DSH\profiles\web" disabled={mode === 'unavailable'} onChange={(event) => changeSelection('profile', event.target.value)} aria-describedby="dsh-path-help" />
      </div>
      <div className="dsh-field"><label htmlFor="dsh-runtime">Runtime package.json（可选，绝对路径）</label>
        <input id="dsh-runtime" className="control" type="text" value={runtime} maxLength={4096} autoComplete="off" spellCheck={false}
          placeholder="C:\DSH\runtime\package.json" disabled={mode === 'unavailable'} onChange={(event) => changeSelection('runtime', event.target.value)} aria-describedby="dsh-path-help" />
      </div>
      <p id="dsh-path-help" className="muted">只在点击盘点后读取最多两个文件，每个不超过 64 KiB。不接受相对路径、链接或 Windows UNC/device 路径。路径不会保存。</p>
      <div className="dsh-form-actions"><button type="submit" className="button button-primary" disabled={busy || workspaceBusy || mode === 'unavailable' || !profile.trim()}><Icon name={busy ? 'refresh' : 'search'} />{busy ? '盘点中…' : mode === 'demo' ? '查看合成示例' : '盘点所选声明'}</button>
        {busy && <span className="muted" role="status">正在读取所选声明，可切换工作区。此操作不启动外部程序。</span>}
      </div>
      {selectionChanged && <p className="muted" role="status">路径已更改，旧结果已清除。{busy ? '当前读取结束后请重新盘点。' : '请重新盘点。'}</p>}
    </form>
    {error && <div className="notice notice-error" role="alert"><Icon name="alert" /><span>{error}</span></div>}

    {report && <section className="dsh-report" aria-label="DSH 声明盘点结果">
      <div className="notice" role="status"><Icon name="info" /><span>{result.synthetic ? '合成示例结果，不代表本机状态。' : '盘点报告已生成。文件缺失或不支持不代表未安装 DSH。'}</span></div>
      <div className="panel dsh-state-panel"><h2>运行状态与管理边界</h2><dl className="dsh-facts">
        <div><dt>Runtime 运行状态</dt><dd>未知</dd></div><div><dt>认证状态</dt><dd>未知</dd></div><div><dt>实际启用状态</dt><dd>未知</dd></div>
        <div><dt>管理能力</dt><dd>全部关闭</dd></div>
      </dl><p className="muted">安装、配置、插件启停、认证、运行、升级和卸载均未开放。识别 manifest 版本也不会开放写操作。</p></div>
      <div className="dsh-manifests">
        <section className="panel"><h2>Profile manifest</h2><ManifestState value={report.profile_manifest} />
          {metadata && <><p className="dsh-name">声明名称：{metadata.name ?? '未声明'}</p><h3>保存的 Bundle 选择（{metadata.saved_bundle_selections.length}）</h3>
            {metadata.saved_bundle_selections.length ? <ol className="dsh-bundles">{metadata.saved_bundle_selections.map((bundle, index) => <li key={`${index}:${bundle}`}>{bundle}</li>)}</ol> : <p className="muted">未声明 Bundle 选择，不代表运行时不存在。</p>}</>}
        </section>
        <section className="panel"><h2>Runtime manifest</h2><ManifestState value={report.runtime_manifest} />
          {runtimeMetadata && <dl className="dsh-facts"><div><dt>声明包名</dt><dd>{runtimeMetadata.name ?? '未声明'}</dd></div><div><dt>Manifest 报告版本</dt><dd>{runtimeMetadata.reported_version ?? '未声明'}</dd></div><div><dt>声明 Node 要求</dt><dd>{runtimeMetadata.declared_node_engine ?? '未声明'}</dd></div></dl>}
          <p className="muted">{report.version_recognition === 'reviewed_manifest_version' ? '声明的名称与版本匹配已审阅快照；未验证代码身份、实际版本或兼容性。' : report.version_recognition === 'unrecognized_manifest_version' ? '未识别此声明版本；未验证运行时兼容性。' : '声明版本识别状态未知。'} 不是实测版本。</p>
        </section>
      </div>
      {metadata && <section className="panel"><h2>声明依赖（{metadata.declared_dependencies.length}）</h2><p className="muted">仅 dependencies、optionalDependencies、peerDependencies。不会解析或执行包，原始依赖地址不会显示。</p>
        {metadata.declared_dependencies.length ? <div className="dsh-table-scroll"><table className="dsh-dependencies"><thead><tr><th scope="col">包名</th><th scope="col">声明区段</th><th scope="col">规格类别</th><th scope="col">已安装 / 实际启用</th></tr></thead><tbody>
          {metadata.declared_dependencies.map((dependency, index) => <tr key={`${index}:${dependency.section}:${dependency.name}`}><td>{dependency.name}</td><td>{{ dependencies: 'dependencies', optional_dependencies: 'optionalDependencies', peer_dependencies: 'peerDependencies' }[dependency.section]}</td><td>{dependency.specification_kind === 'registry_version_or_range' ? '版本或范围（未解析）' : '其他规格（已隐藏）'}</td><td>未知 / 未知</td></tr>)}
        </tbody></table></div> : <p className="muted">未声明受支持区段的依赖，不代表运行时没有插件。</p>}
      </section>}
    </section>}
    <p className="muted dsh-boundary-note">只读保证针对本次盘点，不代表整个桌面应用不写入自身数据。文件读取可能更新访问时间；文件系统检查不是对抗同权限并发修改的沙箱，也没有文件读取的硬超时。</p>
    <DshDistribution />
    <DshNativeArchives />
  </div>;
}
