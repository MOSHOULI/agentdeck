import { useCallback, useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import * as bridge from '../bridge';
import type { QwenSection, QwenStatus } from '../qwenContract';
import { sectionNames } from '../qwenContract';
import { useWorkspaceActivity } from '../stores/workspaceActivity';
import './DshInventory.css';
import { QwenVimSetting } from './QwenVimSetting';

const statuses: Record<QwenStatus, string> = {
  not_selected: '未选择', not_read: '未读取', read: '已读取', missing: '文件缺失', unsupported: '格式或路径不受支持', unreadable: '无法读取',
};
const sections: Record<QwenSection, string> = {
  general: '通用', ui: '界面', model: '模型', model_providers: '模型提供商', context: '上下文', tools: '工具', mcp_servers: 'MCP 服务', security: '安全', experimental: '实验功能',
};
export function QwenSettings() {
  const mode = bridge.runtimeMode();
  const [settings, setSettings] = useState('');
  const [manifest, setManifest] = useState('');
  const [busy, setBusy] = useState(false);
  const [editingBusy, setEditingBusy] = useState(false);
  const [mutating, setMutating] = useState(false);
  const onEditActivity = useCallback((active: boolean, mutation: boolean) => {
    setEditingBusy(active); setMutating(mutation);
    if (mutation) { generation.current++; setResult(null); setError(false); }
  }, []);
  const [result, setResult] = useState<Awaited<ReturnType<typeof bridge.inspectQwenSettings>> | null>(null);
  const [error, setError] = useState(false);
  const generation = useRef(0);
  const running = useRef(false);
  const mounted = useRef(false);
  useWorkspaceActivity('qwen', busy || editingBusy);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; generation.current++; };
  }, []);
  const change = (set: (value: string) => void, value: string) => {
    generation.current++; set(value); setResult(null); setError(false);
  };
  const inspect = async (event: FormEvent) => {
    event.preventDefault();
    if (running.current || editingBusy || mode === 'unavailable' || !settings.trim() || !manifest.trim()) return;
    running.current = true; setBusy(true); setResult(null); setError(false);
    const current = ++generation.current;
    try {
      const next = await bridge.inspectQwenSettings(settings, manifest);
      if (mounted.current && current === generation.current) setResult(next);
    } catch {
      if (mounted.current && current === generation.current) setError(true);
    } finally {
      running.current = false;
      if (mounted.current) setBusy(false);
    }
  };
  return <div className="dsh-workspace">
    <header className="workspace-heading"><div><p className="eyebrow">QWEN SETTINGS INSPECTION</p><h1>Qwen 配置检查</h1><p className="muted">显式选择两个文件，仅查看配置区段是否存在。</p></div><span className="status-badge">受限管理</span></header>
    <div className="notice">仅审阅 Qwen 0.27.0 / schema 4。声明版本不是安装真实性证明；不执行 Qwen、不读取独立认证文件、只读检查不初始化备份或改写配置；下方单字段管理需另行确认。</div>
    {mode === 'demo' && <p role="status">合成示例模式，不读取本机文件。</p>}
    {mode === 'unavailable' && <p role="status">未连接桌面后端，请在 AgentDeck 桌面应用中检查。</p>}
    <form className="panel dsh-form" aria-label="Qwen 检查路径" aria-busy={busy} onSubmit={event => { void inspect(event); }}>
      <label htmlFor="qwen-settings">settings.json 绝对路径</label>
      <input className="control" id="qwen-settings" value={settings} maxLength={4096} autoComplete="off" spellCheck={false} disabled={mode === 'unavailable' || mutating} onChange={e => change(setSettings, e.target.value)} />
      <label htmlFor="qwen-manifest">运行时 package.json 绝对路径</label>
      <input className="control" id="qwen-manifest" value={manifest} maxLength={4096} autoComplete="off" spellCheck={false} disabled={mode === 'unavailable' || mutating} onChange={e => change(setManifest, e.target.value)} />
      <p className="muted">路径仅在本页保留，不自动发现。每文件最多 64 KiB，严格 JSON；注释、BOM、未知版本和链接不受支持。文件不受支持不代表 Qwen 未安装。</p>
      <div className="dsh-form-actions"><button type="submit" className="button button-primary" disabled={busy || editingBusy || mode === 'unavailable' || !settings.trim() || !manifest.trim()}>{busy ? '检查中…' : mode === 'demo' ? '查看合成示例' : '检查所选文件'}</button>
        <button type="button" className="button" disabled={mutating} onClick={() => { generation.current++; setResult(null); setError(false); setSettings(''); setManifest(''); }}>清除路径与结果</button></div>
      {busy && <p role="status">正在读取。清除或修改路径后，旧请求的结果将被丢弃；文件读取本身无法中止。</p>}
    </form>
    {error && <p role="alert">检查未完成或响应不兼容。请核对绝对路径、文件名与桌面后端后重试。</p>}
    {result && <section className="panel" aria-label="Qwen 检查结果">
      <h2>{result.synthetic ? '合成示例结果' : '所选文件摘要'}</h2>
      <p>运行时声明：{statuses[result.summary.manifest]}</p><p>设置文件：{statuses[result.summary.settings]}</p>
      <p>{result.summary.reviewed ? '声明匹配已审阅版本，尚未验证实际安装或文件归属。' : '未识别已审阅版本，不读取设置摘要。'}</p>
      {result.summary.sections && <ul>{sectionNames.map(key => <li key={key}>{sections[key]}：{result.summary.sections![key] ? '区段存在' : '区段未声明'}</li>)}</ul>}
      <p className="muted">只报告区段存在性，不显示值或动态键，不代表有效配置或实际生效。认证状态未知；其他配置层、环境变量和命令行覆盖未解析。此摘要不授权安装或配置写入；下方仅提供单字段管理。</p>
    </section>}
    <QwenVimSetting settings={settings} manifest={manifest} inspecting={busy} onActivity={onEditActivity} />
    <p className="muted">读取可能更新访问时间。文件检查不是对抗同权限并发修改的沙箱，也无文件读取硬超时。</p>
  </div>;
}
