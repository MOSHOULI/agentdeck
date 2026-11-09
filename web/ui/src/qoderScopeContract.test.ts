import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {inspectQoderScopes} from './bridge';
import {validateScopeReport} from './qoderScopeContract';
import {rawScopes,selectedScopes} from './test-fixtures/qoderScopes';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>Object.assign(window,{__TAURI_INTERNALS__:{}}));
afterEach(()=>{delete(window as unknown as Record<string,unknown>).__TAURI_INTERNALS__;window.history.replaceState({},'','/');vi.clearAllMocks();});
it('projects only fixed flags/counts and invokes only explicit selected paths',async()=>{
 vi.mocked(invoke).mockResolvedValue(rawScopes());const r=await inspectQoderScopes(selectedScopes);
 expect(JSON.stringify(r)).not.toContain('SECRET');expect(r.overlap).toBe(1);
 expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_qoder_scopes',{userSettings:'/user/settings.json',projectSettings:null,localSettings:null,projectMcp:'/project/.mcp.json'});
});
it('rejects inconsistent identities, statuses, flags and limits',()=>{
 const changes=[(r:any)=>r.scopes.reverse(),(r:any)=>r.scopes.pop(),(r:any)=>r.scopes[1].status='read',(r:any)=>r.scopes[0].issue='missing',(r:any)=>r.scopes[0].summary.mcp_server_count=-1,(r:any)=>r.scopes[0].summary.model_present='true',(r:any)=>r.can_apply=true,(r:any)=>r.can_restore=true,(r:any)=>r.runtime_version='1.1.85',(r:any)=>r.effective_configuration='merged',(r:any)=>r.selection_issue='invalid_path',(r:any)=>r.limits.max_files=9,(r:any)=>r.repeated_mcp_identifier_count=3];
 for(const change of changes){const r=rawScopes();change(r);expect(()=>validateScopeReport(r,selectedScopes)).toThrow();}
});
it('missing or unsupported selected scope suppresses aggregate rather than claiming zero',()=>{
 for(const [status,issue] of [['missing','missing'],['unsupported','platform_unsupported'],['unreadable','file_changed']]){
  const r:any=rawScopes();r.scopes[0]={scope:'user_settings',status,issue,summary:null};r.repeated_mcp_identifier_count=null;
  expect(validateScopeReport(r,selectedScopes).overlap).toBeNull();r.repeated_mcp_identifier_count=0;expect(()=>validateScopeReport(r,selectedScopes)).toThrow();
 }
});
it('aggregate may exceed one-file limit but is bounded by observations',()=>{
 const paths={...selectedScopes,project_settings:'/p/settings.json',local_settings:'/p/settings.local.json'};const r=rawScopes(paths);
 for(const scope of r.scopes)scope.summary!.mcp_server_count=4096;
 r.repeated_mcp_identifier_count=8192;expect(validateScopeReport(r,paths).overlap).toBe(8192);
 r.repeated_mcp_identifier_count=8193;expect(()=>validateScopeReport(r,paths)).toThrow();
});
it('invalid inputs and unavailable/demo do not invoke',async()=>{
 for(const p of ['', '/bad\nsettings.json','/'+ '界'.repeat(1400)])await expect(inspectQoderScopes({...selectedScopes,user_settings:p,project_mcp:''})).rejects.toThrow();
 delete(window as unknown as Record<string,unknown>).__TAURI_INTERNALS__;await expect(inspectQoderScopes(selectedScopes)).rejects.toThrow();
 window.history.replaceState({},'','/?demo=1');await expect(inspectQoderScopes(selectedScopes)).rejects.toThrow();expect(invoke).not.toHaveBeenCalled();
});
