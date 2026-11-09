import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {dshRootPlan,validateDshRootPlan,validateDshRootVerification} from './dshDistributionContract';
import * as bridge from './bridge';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>Object.assign(window,{__TAURI_INTERNALS__:{}}));
afterEach(()=>{delete(window as unknown as Record<string,unknown>).__TAURI_INTERNALS__;window.history.replaceState({},'','/');vi.clearAllMocks();});
const raw=()=>({state:'root_archive_verified',bytes:25197,dependencies_resolved:false,installed:false,runtime_verified:false,signature_verified:false,platform_compatibility:'unverified',plan:{...dshRootPlan,limitations:'SECRET'},path:'SECRET'});
it('projects fixed plan and root-only verification without arbitrary values',async()=>{
 vi.mocked(invoke).mockResolvedValue({...dshRootPlan,limitations:'SECRET'});expect(await bridge.dshDistributionPlan()).toEqual(dshRootPlan);expect(invoke).toHaveBeenCalledExactlyOnceWith('dsh_distribution_plan');vi.mocked(invoke).mockClear();
 vi.mocked(invoke).mockResolvedValue(raw());const report=await bridge.verifyDshPackage('/selected/dsh-0.2.2-rc.1.tgz');expect(JSON.stringify(report)).not.toContain('SECRET');expect(invoke).toHaveBeenCalledExactlyOnceWith('verify_dsh_package',{archivePath:'/selected/dsh-0.2.2-rc.1.tgz'});
});
it('every fixed plan field and verification capability is checked',()=>{
 for(const key of Object.keys(dshRootPlan)){expect(()=>validateDshRootPlan({...dshRootPlan,[key]:'wrong'})).toThrow();expect(()=>validateDshRootVerification({...raw(),plan:{...dshRootPlan,[key]:'wrong'}})).toThrow();}
 for(const key of ['state','bytes','dependencies_resolved','installed','runtime_verified','signature_verified','platform_compatibility'])expect(()=>validateDshRootVerification({...raw(),[key]:'wrong'})).toThrow();
});
it('rejects unbounded or invalid archive input before IPC',async()=>{
 for(const path of ['', 'relative/dsh-0.2.2-rc.1.tgz','/bad.tgz','/secret\n/dsh-0.2.2-rc.1.tgz','/'+ '界'.repeat(1400)+'/dsh-0.2.2-rc.1.tgz','/a/../dsh-0.2.2-rc.1.tgz','//server/dsh-0.2.2-rc.1.tgz'])await expect(bridge.verifyDshPackage(path)).rejects.toThrow();expect(invoke).not.toHaveBeenCalled();
});
it.each(['unavailable','demo'])('does not invoke plan or verify in %s',async mode=>{delete(window as unknown as Record<string,unknown>).__TAURI_INTERNALS__;if(mode==='demo')window.history.replaceState({},'','/?demo=1');await expect(bridge.dshDistributionPlan()).rejects.toThrow();await expect(bridge.verifyDshPackage('/selected/dsh-0.2.2-rc.1.tgz')).rejects.toThrow();expect(invoke).not.toHaveBeenCalled();});
