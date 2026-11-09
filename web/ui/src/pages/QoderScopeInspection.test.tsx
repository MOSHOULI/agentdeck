import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import * as bridge from '../bridge';
import {QoderScopeInspection} from './QoderScopeInspection';
import {rawScopes,selectedScopes} from '../test-fixtures/qoderScopes';
import {validateScopeReport} from '../qoderScopeContract';
import {useWorkspaceActivityStore} from '../stores/workspaceActivity';
const report=validateScopeReport(rawScopes(),selectedScopes);
beforeEach(()=>{vi.spyOn(bridge,'runtimeMode').mockReturnValue('native');vi.spyOn(bridge,'inspectQoderScopes').mockResolvedValue(report);});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
function fill(){fireEvent.change(screen.getByLabelText('用户设置 settings.json'),{target:{value:selectedScopes.user_settings}});fireEvent.change(screen.getByLabelText('项目 MCP .mcp.json'),{target:{value:selectedScopes.project_mcp}});}
function inspect(){fireEvent.click(screen.getByRole('button',{name:'只读检查 Qoder 作用域'}));}
function deferred(){let resolve!:(v:typeof report)=>void;let reject!:(e:Error)=>void;const promise=new Promise<typeof report>((a,b)=>{resolve=a;reject=b;});return{resolve,reject,promise};}
it('requires selection, renders flags/counts not arbitrary values or conflicts',async()=>{
 render(<QoderScopeInspection/>);expect(screen.getByRole('button',{name:'只读检查 Qoder 作用域'})).toBeDisabled();fill();inspect();
 const result=await screen.findByRole('region',{name:'Qoder 作用域检查结果'});expect(result).toHaveTextContent('重复 MCP 标识数：1');expect(result).toHaveTextContent('不是冲突或生效优先级');expect(result).not.toHaveTextContent('SECRET');expect(bridge.inspectQoderScopes).toHaveBeenCalledExactlyOnceWith(selectedScopes);
});
it.each(['edit','clear','cancel','unmount'])('discards late scope result after %s and retains read owner until settled',async change=>{
 const work=deferred();vi.mocked(bridge.inspectQoderScopes).mockReturnValue(work.promise);const view=render(<QoderScopeInspection/>);fill();inspect();inspect();expect(bridge.inspectQoderScopes).toHaveBeenCalledTimes(1);
 if(change==='edit')fireEvent.change(screen.getByLabelText('用户设置 settings.json'),{target:{value:'/other/settings.json'}});
 if(change==='clear')fireEvent.click(screen.getByRole('button',{name:'清除 Qoder 作用域选择与结果'}));
 if(change==='cancel')fireEvent.click(screen.getByRole('button',{name:'取消 Qoder 作用域结果'}));
 if(change==='unmount')view.unmount();
 expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);await act(async()=>work.resolve(report));
 expect(screen.queryByRole('region',{name:'Qoder 作用域检查结果'})).not.toBeInTheDocument();expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false);
});
it('redacts failures and prevents a scope read while another Qoder write owner is active',async()=>{
 const owner=Symbol();useWorkspaceActivityStore.getState().setActivity('qoder',owner,true);render(<QoderScopeInspection/>);expect(screen.getByLabelText('用户设置 settings.json')).toBeDisabled();inspect();expect(bridge.inspectQoderScopes).not.toHaveBeenCalled();
 act(()=>useWorkspaceActivityStore.getState().setActivity('qoder',owner,false));fill();vi.mocked(bridge.inspectQoderScopes).mockRejectedValue(new Error('SECRET'));inspect();expect(await screen.findByRole('alert')).not.toHaveTextContent('SECRET');
});
it.each(['demo','unavailable'] as const)('never invokes in %s',mode=>{vi.mocked(bridge.runtimeMode).mockReturnValue(mode);render(<QoderScopeInspection/>);inspect();expect(bridge.inspectQoderScopes).not.toHaveBeenCalled();});
