import { useState } from 'react';
import { CodexResidualCheck } from './CodexResidualCheck';
import { useAgentStore } from '../stores/agentStore';

/** Read-only, in-memory request history. Never an ownership or recovery receipt. */
export function InstallSessionNote({ agentId }: { agentId: string }) {
  const [open, setOpen] = useState(false);
  const session = useAgentStore(state => state.installSessions[agentId]);
  if (!session) return null;
  const labels = {
    submitted: '安装请求处理中',
    unconfirmed: '安装完成状态未确认',
    completed: '安装请求已完成',
  };
  return <details onToggle={event => setOpen(event.currentTarget.open)} className="agent-details" data-testid="install-session-record" data-state={session.state}>
    <summary>本会话安装记录：{labels[session.state]}</summary>
    <p>这是本会话已确认请求的历史目标快照，不是当前磁盘检查、安装所有权或恢复凭据。关闭或重启应用后记录丢失。</p>
    {session.state === 'unconfirmed' && <p>请求可能在内部预检阶段被拒绝，也可能已执行或留下部分软件；仅凭失败不能判断。没有自动重试、回滚、清理或卸载。</p>}
    {session.state === 'completed' && <p>安装请求已返回成功；当前检测状态以列表刷新为准，刷新失败不会推翻请求已成功返回的结果。未应用 profile 或模型配置。</p>}
    {session.target ? <dl className="agent-facts">{([
      ['npm 包', session.target.package], ['启动命令', session.target.executable],
      ['已审核 npm prefix', session.target.prefix], ['已审核全局目录', session.target.globalRoot],
      ['已审核包目录', session.target.packageRoot], ['已审核启动文件目录', session.target.launcherDirectory],
      ['已审核 npm 入口', session.target.managerPath], ['已审核安装器 Node', session.target.nodePath],
    ] as const).map(([label, value]) => <div key={label}><dt>{label}</dt><dd className="agent-path">{value}</dd></div>)}</dl>
      : <p>本会话没有可关联的已审核路径快照；不会从错误内容猜测目标路径。</p>}
    <p>先刷新列表确认检测结果，再在上述已审核位置手动核对实际文件，并参考该 Agent 的官方安装说明。不要直接删除整段 prefix 或共享 node_modules；它们可能包含其他软件。残留路径会阻止新的安装计划，路径匹配不授予清理权限。</p>
    <p>再次预览不会清理残留或自动重试。此记录不提供可重放令牌或恢复命令。</p>
    {open && agentId === 'codex' && session.state === 'unconfirmed' && session.sessionRef && <CodexResidualCheck key={session.sessionRef} sessionRef={session.sessionRef} />}
  </details>;
}
