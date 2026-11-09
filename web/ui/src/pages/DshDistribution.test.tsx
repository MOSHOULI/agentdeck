import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import * as bridge from '../bridge';
import {dshRootPlan,type DshRootVerification} from '../dshDistributionContract';
import {DshDistribution} from './DshDistribution';
import {DshInventory} from './DshInventory';
import {dshDemoReport} from '../dshContract';
import {useWorkspaceActivityStore} from '../stores/workspaceActivity';
const verified:DshRootVerification={state:'root_archive_verified',bytes:25197,plan:dshRootPlan};
beforeEach(()=>{vi.spyOn(bridge,'runtimeMode').mockReturnValue('native');vi.spyOn(bridge,'dshDistributionPlan').mockResolvedValue(dshRootPlan);vi.spyOn(bridge,'verifyDshPackage').mockResolvedValue(verified);});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
function fill(){fireEvent.change(screen.getByLabelText('DSH 官方根归档绝对路径'),{target:{value:'/selected/dsh-0.2.2-rc.1.tgz'}});}
function verify(){fireEvent.click(screen.getByRole('button',{name:'只读校验 DSH 根归档'}));}
function deferred<T>(){let resolve!:(v:T)=>void;let reject!:(e:Error)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b;});return{resolve,reject,promise};}
it('shows explicit plan and root-only success with no automatic inspection',async()=>{render(<DshDistribution/>);expect(bridge.dshDistributionPlan).not.toHaveBeenCalled();expect(bridge.verifyDshPackage).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'查看 DSH 根包计划'}));expect(await screen.findByRole('region',{name:'DSH 根包计划结果'})).toHaveTextContent('90 个直接依赖');fill();verify();expect(await screen.findByRole('region',{name:'DSH 根归档校验结果'})).toHaveTextContent('依赖未解析、未安装、未运行');});
it.each(['path','clear','cancel','unmount'])('drops late result after %s but owns busy until settlement',async action=>{
 const work=deferred<DshRootVerification>();vi.mocked(bridge.verifyDshPackage).mockReturnValue(work.promise);const view=render(<DshDistribution/>);fill();verify();verify();expect(bridge.verifyDshPackage).toHaveBeenCalledTimes(1);
 if(action==='path')fireEvent.change(screen.getByLabelText('DSH 官方根归档绝对路径'),{target:{value:'/changed/dsh-0.2.2-rc.1.tgz'}});
 if(action==='clear')fireEvent.click(screen.getByRole('button',{name:'清除 DSH 根包选择与结果'}));if(action==='cancel')fireEvent.click(screen.getByRole('button',{name:'取消 DSH 根包结果'}));if(action==='unmount')view.unmount();
 expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(true);await act(async()=>work.resolve(verified));expect(screen.queryByRole('region',{name:'DSH 根归档校验结果'})).not.toBeInTheDocument();expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(false);
});
it('fixed errors clear success and never expose backend text',async()=>{render(<DshDistribution/>);fill();verify();await screen.findByRole('region',{name:'DSH 根归档校验结果'});vi.mocked(bridge.verifyDshPackage).mockRejectedValue(new Error('SECRET'));verify();expect(await screen.findByRole('alert')).not.toHaveTextContent('SECRET');expect(screen.queryByRole('region',{name:'DSH 根归档校验结果'})).not.toBeInTheDocument();});
it.each(['demo','unavailable'] as const)('has no IO in %s',mode=>{vi.mocked(bridge.runtimeMode).mockReturnValue(mode);render(<DshDistribution/>);verify();fireEvent.click(screen.getByRole('button',{name:'查看 DSH 根包计划'}));expect(bridge.verifyDshPackage).not.toHaveBeenCalled();expect(bridge.dshDistributionPlan).not.toHaveBeenCalled();});
it('coordinates both directions with old inventory and retains its read owner across unmount',async()=>{
 const inventory=deferred<bridge.DshInspectionResult>();vi.spyOn(bridge,'inspectDshProfile').mockReturnValue(inventory.promise);
 let view=render(<DshInventory/>);fireEvent.change(screen.getByLabelText('Profile 目录（绝对路径）'),{target:{value:'/profile'}});fireEvent.click(screen.getByRole('button',{name:'盘点所选声明'}));expect(screen.getByRole('button',{name:'查看 DSH 根包计划'})).toBeDisabled();view.unmount();expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(true);
 await act(async()=>inventory.resolve({report:dshDemoReport(false),synthetic:false}));expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(false);
 const root=deferred<DshRootVerification>();vi.mocked(bridge.verifyDshPackage).mockReturnValue(root.promise);view=render(<DshInventory/>);fill();verify();fireEvent.change(screen.getByLabelText('Profile 目录（绝对路径）'),{target:{value:'/profile'}});expect(screen.getByRole('button',{name:'盘点所选声明'})).toBeDisabled();fireEvent.click(screen.getByRole('button',{name:'盘点所选声明'}));expect(bridge.inspectDshProfile).toHaveBeenCalledTimes(1);await act(async()=>root.resolve(verified));
});
it('cancelled failure stays hidden and other workspace activity blocks native root reads',async()=>{
 const owner=Symbol();useWorkspaceActivityStore.getState().setActivity('qwen',owner,true);render(<DshDistribution/>);expect(screen.getByRole('button',{name:'查看 DSH 根包计划'})).toBeDisabled();verify();expect(bridge.verifyDshPackage).not.toHaveBeenCalled();act(()=>useWorkspaceActivityStore.getState().setActivity('qwen',owner,false));
 const work=deferred<DshRootVerification>();vi.mocked(bridge.verifyDshPackage).mockReturnValue(work.promise);fill();verify();fireEvent.click(screen.getByRole('button',{name:'取消 DSH 根包结果'}));expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(true);await act(async()=>work.reject(new Error('SECRET')));expect(screen.queryByRole('alert')).not.toBeInTheDocument();expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(false);
});
it.each(['resolve','reject'] as const)('remount blocks new root IPC until old %s settles without inheriting its outcome',async outcome=>{
 const old=deferred<DshRootVerification>();vi.mocked(bridge.verifyDshPackage).mockReturnValueOnce(old.promise);
 const first=render(<DshDistribution/>);fill();verify();first.unmount();
 render(<DshDistribution/>);
 const planButton=screen.getByRole('button',{name:'查看 DSH 根包计划'});
 const verifyButton=screen.getByRole('button',{name:'只读校验 DSH 根归档'});
 expect(planButton).toBeDisabled();expect(verifyButton).toBeDisabled();
 fireEvent.click(planButton);fireEvent.click(verifyButton);
 expect(bridge.dshDistributionPlan).not.toHaveBeenCalled();expect(bridge.verifyDshPackage).toHaveBeenCalledTimes(1);
 await act(async()=>{if(outcome==='resolve')old.resolve(verified);else old.reject(new Error('OLD_SECRET'));});
 expect(screen.queryByRole('region',{name:'DSH 根包计划结果'})).not.toBeInTheDocument();
 expect(screen.queryByRole('region',{name:'DSH 根归档校验结果'})).not.toBeInTheDocument();expect(screen.queryByRole('alert')).not.toBeInTheDocument();
 expect(useWorkspaceActivityStore.getState().busy.dsh).toBe(false);expect(planButton).not.toBeDisabled();
 fireEvent.click(planButton);await screen.findByRole('region',{name:'DSH 根包计划结果'});
 expect(bridge.dshDistributionPlan).toHaveBeenCalledTimes(1);
 fill();expect(verifyButton).not.toBeDisabled();verify();
 expect(await screen.findByRole('region',{name:'DSH 根归档校验结果'})).toHaveTextContent('已读取字节：25197');
 expect(bridge.verifyDshPackage).toHaveBeenCalledTimes(2);
});
