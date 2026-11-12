import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as bridge from '../bridge';
import { QoderDeploymentInspection } from './QoderDeploymentInspection';
import { useWorkspaceActivityStore } from '../stores/workspaceActivity';
vi.mock('../bridge', () => ({runtimeMode:vi.fn(()=> 'native'),inspectQoderDeployment:vi.fn()}));
const fill = (value='/private/deployed') => fireEvent.change(screen.getByLabelText('Qoder 已部署绝对目录'),{target:{value}});
const run = () => fireEvent.click(screen.getByRole('button',{name:'只读检查 Qoder 私有部署'}));
const pending = () => { let resolve!: (v:{state:'payload_verified'})=>void; const promise=new Promise<{state:'payload_verified'}>(r=>resolve=r); return {promise,resolve}; };
beforeEach(()=>{vi.clearAllMocks();vi.mocked(bridge.runtimeMode).mockReturnValue('native');vi.mocked(bridge.inspectQoderDeployment).mockResolvedValue({state:'payload_verified'}); const s=useWorkspaceActivityStore.getState();for(const id of Object.keys(s.owners) as (keyof typeof s.owners)[])for(const owner of s.owners[id])s.setActivity(id,owner,false);});
afterEach(cleanup);
it('requires explicit request and displays fixed labels, not dynamic prose',async()=>{
 render(<QoderDeploymentInspection active release="1.1.85"/>); fill();expect(bridge.inspectQoderDeployment).not.toHaveBeenCalled();run();await screen.findByText('私有部署载荷与固定版本匹配（未运行）');expect(bridge.inspectQoderDeployment).toHaveBeenCalledExactlyOnceWith('/private/deployed','1.1.85');fill('/other');expect(screen.queryByTestId('qoder-deployment-inspection-result')).not.toBeInTheDocument();
});
it.each(['edit','cancel','clear','navigate','release','unmount'])('discards late results on %s while retaining busy ownership',async(mode)=>{
 const p=pending();vi.mocked(bridge.inspectQoderDeployment).mockReturnValue(p.promise);const view=render(<QoderDeploymentInspection active release="1.1.85"/>);fill();run();
 if(mode==='edit')fill('/other');if(mode==='cancel')fireEvent.click(screen.getByText('取消 Qoder 部署检查结果'));if(mode==='clear')fireEvent.click(screen.getByText('清除 Qoder 部署检查'));if(mode==='navigate'){view.rerender(<QoderDeploymentInspection active={false} release="1.1.85"/>);view.rerender(<QoderDeploymentInspection active release="1.1.85"/>);}if(mode==='release')view.rerender(<QoderDeploymentInspection active release="1.1.86"/>);if(mode==='unmount')view.unmount();
 expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(true);await act(async()=>p.resolve({state:'payload_verified'}));expect(screen.queryByTestId('qoder-deployment-inspection-result')).not.toBeInTheDocument();expect(useWorkspaceActivityStore.getState().busy.qoder).toBe(false);expect(bridge.inspectQoderDeployment).toHaveBeenCalledTimes(1);
});
it('blocks concurrent work and demo invocation, projects error to fixed message',async()=>{
 const owner=Symbol();useWorkspaceActivityStore.getState().setActivity('agents',owner,true);render(<QoderDeploymentInspection active release="1.1.85"/>);fill();run();expect(bridge.inspectQoderDeployment).not.toHaveBeenCalled();act(()=>useWorkspaceActivityStore.getState().setActivity('agents',owner,false));vi.mocked(bridge.inspectQoderDeployment).mockRejectedValue('SECRET_BACKEND_PATH');run();await screen.findByRole('alert');expect(screen.queryByText('SECRET_BACKEND_PATH')).not.toBeInTheDocument();
});
it('demo does not read or fake success',()=>{vi.mocked(bridge.runtimeMode).mockReturnValue('demo');render(<QoderDeploymentInspection active release="1.1.85"/>);run();expect(bridge.inspectQoderDeployment).not.toHaveBeenCalled();});
