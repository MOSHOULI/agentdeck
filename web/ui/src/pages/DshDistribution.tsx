import {useEffect,useRef,useState} from 'react';
import * as bridge from '../bridge';
import type {DshRootPlan,DshRootVerification} from '../dshDistributionContract';
import {useWorkspaceActivityStore} from '../stores/workspaceActivity';
export function DshDistribution(){
 const [path,setPath]=useState(''),[plan,setPlan]=useState<DshRootPlan|null>(null),[verified,setVerified]=useState<DshRootVerification|null>(null),[error,setError]=useState(false),[busy,setBusy]=useState(false);
 const mounted=useRef(false),running=useRef(false),epoch=useRef(0);const native=bridge.runtimeMode()==='native';const active=useWorkspaceActivityStore(s=>Object.values(s.busy).some(Boolean));
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;epoch.current++;};},[]);
 const invalidate=()=>{epoch.current++;setPlan(null);setVerified(null);setError(false);};
 const run=async(verify:boolean)=>{
  if(!native||running.current||Object.values(useWorkspaceActivityStore.getState().busy).some(Boolean)||(verify&&!path.trim()))return;
  invalidate();const request=epoch.current;const selected=path;running.current=true;setBusy(true);const owner=Symbol('dsh-root-read');useWorkspaceActivityStore.getState().setActivity('dsh',owner,true);
  try{if(verify){const result=await bridge.verifyDshPackage(selected);if(mounted.current&&epoch.current===request){setVerified(result);setPlan(result.plan);}}else{const result=await bridge.dshDistributionPlan();if(mounted.current&&epoch.current===request)setPlan(result);}}
  catch{if(mounted.current&&epoch.current===request)setError(true);}
  finally{running.current=false;useWorkspaceActivityStore.getState().setActivity('dsh',owner,false);if(mounted.current)setBusy(false);}
 };
 return <section className="panel" aria-label="DSH 根包分发与只读校验">
  <h2>固定 DSH 根包（不是安装）</h2>
  <p>固定 @deepseek-ai/dsh 0.2.2-rc.1 预发布版，不自动追随 latest。不下载、不解包、不安装、不调用 npm/npx/Node/DSH，不改 PATH 或配置。</p>
  <label htmlFor="dsh-root-archive">DSH 官方根归档绝对路径</label>
  <input id="dsh-root-archive" className="control" value={path} maxLength={4096} disabled={!native||(active&&!busy)} autoComplete="off" spellCheck={false} onChange={e=>{invalidate();setPath(e.target.value);}}/>
  <button type="button" disabled={!native||busy||active} onClick={()=>{void run(false);}}>查看 DSH 根包计划</button>
  <button type="button" disabled={!native||busy||active||!path.trim()} onClick={()=>{void run(true);}}>只读校验 DSH 根归档</button>
  <button type="button" disabled={active&&!busy} onClick={()=>{invalidate();setPath('');}}>清除 DSH 根包选择与结果</button>
  {busy&&<button type="button" onClick={invalidate}>取消 DSH 根包结果</button>}
  {busy&&<p role="status">正在处理，修改输入或取消仅废弃结果，不停止底层读取；真实结算前继续保持忙碌。</p>}
  {!native&&<p>仅原生桌面后端可用；浏览器与演示模式不读取或模拟根包验证成功。</p>}
  {error&&<p role="alert">DSH 根包未确认。请检查精确文件名、固定摘要与读取边界；不安全文件或不支持的主机失败关闭，没有安装或运行结论。</p>}
  {plan&&<section aria-label="DSH 根包计划结果">
   <p>{`固定版本：${plan.version}（预发布）`}</p><p>{`官方归档名：${plan.archive_name}`}</p>
   <p style={{overflowWrap:'anywhere'}}>{`官方地址（仅展示）：${plan.url}`}</p><p style={{overflowWrap:'anywhere'}}>{`根包 SHA256：${plan.sha256}`}</p><p style={{overflowWrap:'anywhere'}}>{`根包 SHA512 SRI：${plan.integrity}`}</p>
   <p>精确长度：25197 字节；读取上限：65536 字节。Node ≥24.9.0 仅为声明，未实测兼容。</p>
   <p>90 个直接依赖，其中 8 项范围；未解析传递依赖。根包未声明 scripts 不表示依赖没有 postinstall。未声明 os/cpu 不表示平台兼容，未验证独立签名。</p>
  </section>}
  {verified&&<section aria-label="DSH 根归档校验结果"><p role="status">根归档与固定长度、SHA256 和 SHA512 一致。</p><p>{`已读取字节：${verified.bytes}`}</p><p>仅根包完整性；依赖未解析、未安装、未运行、平台兼容性未验证。</p></section>}
  <p>只读文件边界已审阅 Windows 与 Linux x86_64/ARM64；其他主机失败关闭。访问时间可能改变，无法完全防止同用户竞争，文件系统 I/O 无硬超时。摘要匹配不是独立签名或执行授权。</p>
 </section>;
}
