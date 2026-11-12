import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { CodexInstallRecovery } from './CodexInstallRecovery';
import * as bridge from '../bridge';
import { useAgentStore } from '../stores/agentStore';
import { type CodexInstallRecovery as Plan } from '../codexInstallRecoveryContract';
vi.mock('../bridge', async (original) => ({...await original<typeof import('../bridge')>(),
  previewCodexInstallRecovery:vi.fn(), confirmCodexInstallRecovery:vi.fn(), runtimeMode:vi.fn(()=>'native'),
}));
const plan: Plan = {schemaVersion:1,agentId:'codex',state:'eligible',prefix:'/synthetic/prefix',
  slots:[{slot:'package_directory',state:'missing'},{slot:'posix_launcher',state:'missing'}],
  revision:'a'.repeat(64),runtimeNotRun:true,cleanupSupported:false,limitations:'Metadata-only synthetic test.'};
const load = vi.fn();
const preview = async () => {
  fireEvent.click(screen.getByTestId('codex-recovery-preview'));
  await screen.findByTestId('codex-recovery-result');
};
describe('explicit Codex protection recovery',()=>{
  beforeEach(()=>{
    vi.clearAllMocks();load.mockResolvedValue(undefined);
    vi.mocked(bridge.runtimeMode).mockReturnValue('native');
    vi.mocked(bridge.previewCodexInstallRecovery).mockResolvedValue(plan);
    vi.mocked(bridge.confirmCodexInstallRecovery).mockResolvedValue(undefined);
    useAgentStore.setState({loading:false,previewing:false,error:null,notice:null,loadAgents:load});
  });
  it('only previews until separately confirmed, then refreshes without installing',async()=>{
    render(<CodexInstallRecovery active reviewContext="one"/>);await preview();
    expect(bridge.confirmCodexInstallRecovery).not.toHaveBeenCalled();expect(load).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('codex-recovery-confirm'));
    await waitFor(()=>expect(load).toHaveBeenCalledTimes(1));
    expect(bridge.confirmCodexInstallRecovery).toHaveBeenCalledExactlyOnceWith('a'.repeat(64));
    expect(screen.queryByTestId('codex-recovery-result')).not.toBeInTheDocument();
    expect(useAgentStore.getState().notice).toBe('已解除 Codex 探测保护，未安装或清理软件。');
  });
  it('cancel and changed review context invalidate confirmation without refresh',async()=>{
    const view=render(<CodexInstallRecovery active reviewContext="one"/>);await preview();
    fireEvent.click(screen.getByTestId('codex-recovery-cancel'));
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
    await preview();view.rerender(<CodexInstallRecovery active reviewContext="two"/>);
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
    expect(bridge.confirmCodexInstallRecovery).not.toHaveBeenCalled();expect(load).not.toHaveBeenCalled();
  });
  it('blocked and clear observations never offer confirmation',async()=>{
    vi.mocked(bridge.previewCodexInstallRecovery).mockResolvedValue({...plan,state:'blocked',revision:null});
    render(<CodexInstallRecovery active reviewContext="one"/>);await preview();
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
    expect(screen.getByText(/当前条件不允许解除保护/)).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('codex-recovery-cancel'));
    vi.mocked(bridge.previewCodexInstallRecovery).mockResolvedValue({...plan,state:'clear',prefix:null,slots:[],revision:null});
    await preview();expect(screen.getByText('当前没有待恢复的持久安装保护。')).toBeInTheDocument();
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
  });
  it('stale or uncertain confirmation consumes the token and never retries or refreshes',async()=>{
    vi.mocked(bridge.confirmCodexInstallRecovery).mockRejectedValue('private detail');
    render(<CodexInstallRecovery active reviewContext="one"/>);await preview();
    fireEvent.click(screen.getByTestId('codex-recovery-confirm'));
    expect(await screen.findByTestId('codex-recovery-error')).toHaveTextContent('保护状态未确认');
    expect(screen.queryByText('private detail')).not.toBeInTheDocument();
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();expect(load).not.toHaveBeenCalled();
  });
  it('repeated confirmation clicks dispatch once and cancellation cannot replay it',async()=>{
    let resolve!:()=>void;
    vi.mocked(bridge.confirmCodexInstallRecovery).mockReturnValue(new Promise(r=>{resolve=r;}));
    render(<CodexInstallRecovery active reviewContext="one"/>);await preview();
    const button=screen.getByTestId('codex-recovery-confirm');fireEvent.click(button);fireEvent.click(button);
    fireEvent.click(screen.getByTestId('codex-recovery-cancel'));
    expect(bridge.confirmCodexInstallRecovery).toHaveBeenCalledTimes(1);
    await act(async()=>resolve());await waitFor(()=>expect(load).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
  });
  it('keeps operation ownership through unmount/remount until IPC settles',async()=>{
    let resolve!:(p:Plan)=>void;
    vi.mocked(bridge.previewCodexInstallRecovery).mockReturnValue(new Promise(r=>{resolve=r;}));
    const first=render(<CodexInstallRecovery active reviewContext="one"/>);
    fireEvent.click(screen.getByTestId('codex-recovery-preview'));first.unmount();
    render(<CodexInstallRecovery active reviewContext="one"/>);
    expect(screen.getByTestId('codex-recovery-preview')).toBeDisabled();
    fireEvent.click(screen.getByTestId('codex-recovery-preview'));
    expect(bridge.previewCodexInstallRecovery).toHaveBeenCalledTimes(1);
    await act(async()=>resolve(plan));
    await waitFor(()=>expect(screen.getByTestId('codex-recovery-preview')).toBeEnabled());
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
  });
  it('invalidates eligible tokens on selection and stale-context changes',async()=>{
    const agentContext={id:'codex'};
    const view=render(<CodexInstallRecovery active reviewContext="one" agentContext={agentContext}/>);
    await preview();view.rerender(<CodexInstallRecovery active reviewContext="one" agentContext={agentContext} selected/>);
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
    await preview();view.rerender(<CodexInstallRecovery active reviewContext="one" agentContext={agentContext} selected dataStale/>);
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
    expect(screen.getByTestId('codex-recovery-preview')).toBeDisabled();
    expect(bridge.confirmCodexInstallRecovery).not.toHaveBeenCalled();
  });
  it('discards hidden or cancelled pending previews and blocks demo mutations',async()=>{
    let resolve!:(p:Plan)=>void;
    vi.mocked(bridge.previewCodexInstallRecovery).mockReturnValue(new Promise(r=>{resolve=r;}));
    const view=render(<CodexInstallRecovery active reviewContext="one"/>);
    fireEvent.click(screen.getByTestId('codex-recovery-preview'));
    view.rerender(<CodexInstallRecovery active={false} reviewContext="one"/>);
    await act(async()=>resolve(plan));view.rerender(<CodexInstallRecovery active reviewContext="one"/>);
    expect(screen.queryByTestId('codex-recovery-confirm')).not.toBeInTheDocument();
    vi.mocked(bridge.runtimeMode).mockReturnValue('demo');view.rerender(<CodexInstallRecovery active reviewContext="two"/>);
    expect(screen.getByTestId('codex-recovery-preview')).toBeDisabled();
  });
});
