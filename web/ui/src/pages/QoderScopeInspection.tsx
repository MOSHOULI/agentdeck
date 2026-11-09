import { useEffect, useRef, useState } from 'react';
import { inspectQoderScopes, runtimeMode } from '../bridge';
import { scopeIds, scopeLabels, flags, flagLabels, type ScopePaths, type ScopeReport } from '../qoderScopeContract';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
const empty = ():ScopePaths=>({user_settings:'',project_settings:'',local_settings:'',project_mcp:''});
const ids={user_settings:'user',project_settings:'project',local_settings:'local',project_mcp:'mcp'};
const statuses={not_selected:'未选择',read:'已读取固定摘要',missing:'所选文件缺失',unsupported:'不支持或不安全；不等于 Qoder 拒绝此配置',unreadable:'未能读取'};
export function QoderScopeInspection(){
  const [paths,setPaths]=useState<ScopePaths>(empty);const [report,setReport]=useState<ScopeReport|null>(null);const [error,setError]=useState(false);const [busy,setBusy]=useState(false);
  const epoch=useRef(0),running=useRef(false),mounted=useRef(false);
  const active=useWorkspaceActivityStore(s=>Object.values(s.busy).some(Boolean));const native=runtimeMode()==='native';
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;epoch.current++;};},[]);
  const invalidate=()=>{epoch.current++;setReport(null);setError(false);};
  const inspect=async()=>{
    if(!native||running.current||!scopeIds.some(id=>!!paths[id])||Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean))return;
    const selected={...paths};invalidate();const request=epoch.current;running.current=true;setBusy(true);
    const owner=Symbol('qoder-scope-read');useWorkspaceActivityStore.getState().setActivity('qoder',owner,true);
    try{const value=await inspectQoderScopes(selected);if(mounted.current&&epoch.current===request)setReport(value);}
    catch{if(mounted.current&&epoch.current===request)setError(true);}
    finally{running.current=false;useWorkspaceActivityStore.getState().setActivity('qoder',owner,false);if(mounted.current)setBusy(false);}
  };
  return <section className="panel" aria-label="Qoder 多作用域只读检查">
    <h2>显式配置作用域检查（只读）</h2>
    <p>明确选择 1–4 个文件，不自动扫描或展开环境变量，不读取独立凭据文件。所选设置可能含凭据，其字节仅用于解析，不显示名称或值。BYOK 请使用官方 /model 向导。</p>
    <p>仅 Windows 与 Linux x86_64/ARM64 读取边界已审阅；其他主机失败关闭。2026-11-08 文档契约不是运行版本适配器，不证明安装、信任或配置生效。</p>
    {scopeIds.map(id=><div key={id}><label htmlFor={`qoder-scope-${ids[id]}`}>{scopeLabels[id]}</label><input id={`qoder-scope-${ids[id]}`} className="control" value={paths[id]} maxLength={4096} disabled={!native || (active&&!busy)} autoComplete="off" spellCheck={false} onChange={e=>{invalidate();setPaths({...paths,[id]:e.target.value});}} /></div>)}
    <button type="button" disabled={!native||active||busy||!scopeIds.some(id=>!!paths[id])} onClick={()=>{void inspect();}}>只读检查 Qoder 作用域</button>
    <button type="button" disabled={active&&!busy} onClick={()=>{invalidate();setPaths(empty());}}>清除 Qoder 作用域选择与结果</button>
    {busy&&<button type="button" onClick={invalidate}>取消 Qoder 作用域结果</button>}
    {busy&&<p role="status">正在读取所选配置；取消或修改输入仅废弃结果，不中止读取。真实结算前继续保持忙碌。</p>}
    {!native&&<p>仅原生桌面可读取；浏览器和演示模式不调用文件检查或模拟成功。</p>}
    {error&&<p role="alert">Qoder 作用域检查未完成；请检查明确选择、预期文件名及读取边界。没有配置生效、安全或修复结论。</p>}
    {report&&<section aria-label="Qoder 作用域检查结果">
      {report.scopes.map(item=><section key={item.scope} aria-label={`Qoder 作用域 ${item.scope}`} data-scope={item.scope}>
        <h3>{scopeLabels[item.scope]}</h3><p>{statuses[item.status]}</p>
        {item.summary&&<><p>{`MCP 条目数：${item.summary.mcp_server_count}`}</p>{flags.map(flag=><p key={flag}>{`${flagLabels[flag]}：${item.summary![flag]?'存在':'未见'}`}</p>)}</>}
      </section>)}
      <p>{report.overlap===null?'重复 MCP 标识数：未知（所选文件未全部读取）':`重复 MCP 标识数：${report.overlap}`}</p>
      <p>重复只表示潜在重叠，不是冲突或生效优先级；不合并配置。未选作用域不视为空，作用域名称是用户声明，不验证项目归属。分区存在不代表类型正确。</p>
      <p>不执行、不写入、不备份、不恢复。读取可能改变访问时间，非跨文件原子快照；词法去重不消除物理别名，不支持的语法不代表 Qoder 拒绝它。</p>
    </section>}
  </section>;
}
