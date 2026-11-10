import React, { useEffect, useId, useRef, useState } from 'react';
import { installationEvidenceProblem, runtimeMode, type AgentInstallPreview, type AgentInfo, type InstallationInstance, type InstallationSource } from '../bridge';
import { Icon } from './Icon';

interface AgentCardProps {
  onOpenDshInventory?: () => void;
  agent: AgentInfo;
  onPrepareInstall: (id: string) => Promise<AgentInstallPreview | null>;
  onInstall: (id: string, revision: string) => void | Promise<void>;
  onUninstall: (id: string) => void | Promise<void>;
  onConfigure: (id: string) => void | Promise<void>;
  onSelect: (id: string) => void;
  selected: boolean;
  reviewContext?: string;
  busy?: boolean;
  dataStale?: boolean;
}

function getUiTypeLabel(uiType: string): string {
  const labels: Record<string, string> = {
    all: '全平台',
    'cli+desktop': 'CLI + 桌面',
    cli: '终端 CLI',
    desktop: '桌面应用',
    'vscode-extension': 'VS Code 插件',
    'ide-extension': 'IDE 插件',
    'cli-bot': 'CLI Bot',
  };
  return labels[uiType] ?? uiType;
}

function capabilityLabel(supported?: boolean): string {
  return supported === true ? '受支持' : supported === false ? '不受支持' : '能力未提供';
}

function officialHomepage(value?: string | null): string | null {
  if (!value) return null;
  try {
    const url = new URL(value);
    return url.protocol === 'https:' && !url.username && !url.password ? url.href : null;
  } catch {
    return null;
  }
}

function nonemptyText(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function sourceEvidence(value: unknown, resolvedPath: string | null, entrypointPath: string | null): InstallationSource {
  if (record(value)) {
    if (value.kind === 'unknown' && nonemptyText(value.reason)) return { kind: 'unknown', reason: value.reason };
    if (value.kind === 'npm' && resolvedPath && entrypointPath && nonemptyText(value.package)
      && nonemptyText(value.packageRoot) && nonemptyText(value.globalRoot)
      && nonemptyText(value.prefix) && nonemptyText(value.managerPath)) {
      return {
        kind: 'npm', package: value.package, packageRoot: value.packageRoot,
        globalRoot: value.globalRoot, prefix: value.prefix, managerPath: value.managerPath,
      };
    }
  }
  return { kind: 'unknown', reason: '来源证据缺失或格式不完整，无法确认当前目录与包的关联。' };
}

// Treat new IPC fields as observations. Incomplete or future source variants
// must never become ownership, version, or mutation authorization.
function installationEvidence(agent: AgentInfo) {
  const details: unknown = agent.installationDetails;
  const hasDetails = record(details);
  const rawInstances = hasDetails && Array.isArray(details.instances) ? details.instances : [];
  const instances: InstallationInstance[] = rawInstances.filter((value) => record(value) && nonemptyText(value.path)).map((value) => {
    const resolvedPath = nonemptyText(value.resolvedPath) ? value.resolvedPath : null;
    const entrypointPath = nonemptyText(value.entrypointPath) ? value.entrypointPath : null;
    return {
      path: value.path,
      resolvedPath,
      entrypointPath,
      version: nonemptyText(value.version) ? value.version : null,
      active: value.active === true,
      source: sourceEvidence(value.source, resolvedPath, entrypointPath),
    };
  });
  const activeInstances = instances.filter((instance) => instance.active);
  const active = activeInstances.length === 1 ? activeInstances[0] : null;
  const problem = installationEvidenceProblem(agent);
  const invalid = problem === 'invalid';
  const ambiguous = problem !== null;
  const probe = hasDetails && record(details.probe) ? details.probe : null;
  const measuredVersion = probe?.status === 'installed' && !invalid ? active?.version ?? null : null;
  return {
    hasDetails, instances, active, ambiguous, invalid, measuredVersion,
    message: hasDetails && nonemptyText(details.message) ? details.message : null,
    probeMessage: probe && nonemptyText(probe.message) ? probe.message : null,
  };
}

function SourceFacts({ source }: { source: InstallationSource }) {
  return source.kind === 'npm' ? <>
    <div className="agent-evidence-wide"><dt>来源判断</dt><dd>npm 全局目录匹配，安装历史未知<span className="agent-fact-note">仅证明当前目录与包入口的关联，不证明最初安装方式、安装时间或由 AgentDeck 安装。</span></dd></div>
    <div><dt>npm 包</dt><dd>{source.package}</dd></div>
    <div><dt>包目录</dt><dd className="agent-path">{source.packageRoot}</dd></div>
    <div><dt>npm 全局目录</dt><dd className="agent-path">{source.globalRoot}</dd></div>
    <div><dt>npm prefix</dt><dd className="agent-path">{source.prefix}</dd></div>
    <div><dt>npm 命令</dt><dd className="agent-path">{source.managerPath}</dd></div>
  </> : <div className="agent-evidence-wide"><dt>来源判断</dt><dd>来源未知<span className="agent-fact-note">{source.reason}</span></dd></div>;
}

/** A compact row with separate native controls for selection, details and actions. */
export const AgentCard: React.FC<AgentCardProps> = ({
  agent,
  onOpenDshInventory,
  onInstall,
  onPrepareInstall,
  onConfigure,
  onSelect,
  selected,
  reviewContext,
  busy = false,
  dataStale = true,
}) => {
  const [expanded, setExpanded] = useState(false);
  const [pending, setPending] = useState<'install' | 'configure' | null>(null);
  const [plan, setPlan] = useState<AgentInstallPreview | null>(null);
  const generation = useRef(0);
  useEffect(() => {
    generation.current++;
    setPlan(null);
    return () => { generation.current++; };
  }, [agent, selected, dataStale, reviewContext]);
  const [operationError, setOperationError] = useState<string | null>(null);
  const operationLock = useRef(false);
  const detailButton = useRef<HTMLButtonElement>(null);
  const detailsId = useId();
  const configurationNoteId = useId();
  const mode = runtimeMode();
  const readOnly = mode !== 'native';
  const homepage = officialHomepage(agent.homepage);
  const installed = agent.installed && agent.installationStatus === 'installed';
  const notDetected = !agent.installed && agent.installationStatus === 'not_detected';
  const installUnknown = !installed && !notDetected;
  const evidence = installationEvidence(agent);
  const canInstall = notDetected && agent.canInstall === true && !evidence.ambiguous;
  const supportsConfigure = installed && agent.canConfigure === true;
  const canConfigure = supportsConfigure && !evidence.ambiguous;
  const configurationReason = evidence.invalid
    ? '安装检测证据不完整或状态不一致，已暂停安装与应用配置。请刷新列表。'
    : evidence.ambiguous ? 'PATH 候选优先级存在歧义或当前实例未确认，已暂停安装与应用配置。请先确认实际使用的实例并刷新列表。' : undefined;
  const version = installed ? evidence.measuredVersion : null;
  const versionLabel = version ? `v${version.replace(/^v/i, '')}` : evidence.hasDetails ? '版本未知' : '版本未验证';
  // Current source linkage never enables uninstall, even if a backend advertises it.
  const manual = agent.installed || !canInstall;
  const status = evidence.invalid ? '安装状态未知' : installed ? '已安装' : notDetected ? 'PATH 未检测到'
    : agent.installationStatus === 'error' ? '检测出错' : '安装状态未知';
  const manualHelp = agent.installed
    ? '自动卸载功能尚未提供；来源目录匹配不代表安装历史或管理权限。'
    : evidence.ambiguous ? '检测证据待确认，自动安装已暂停。'
    : installUnknown
      ? [agent.installationMessage || '无法确认本地安装状态，自动安装已暂停。请刷新列表或查看官方说明。', agent.installHelp].filter(Boolean).join(' ')
      : agent.installHelp || '此 Agent 暂不支持自动安装，请参阅官方安装说明。';
  const blockedReason = readOnly
    ? mode === 'demo' ? '只读演示：请在桌面应用中管理安装与配置。' : '未连接桌面后端，请通过桌面应用管理安装与配置。'
    : busy || pending ? '请等待当前操作完成。'
      : dataStale ? '当前状态待刷新，请成功刷新列表后再操作。' : undefined;
  const disabledReason = blockedReason || configurationReason || (manual ? manualHelp : undefined);

  const cancelPreview = () => {
    generation.current++;
    setPlan(null);
  };

  const runOperation = async (operation: 'install' | 'configure') => {
    if (operationLock.current || busy || readOnly || dataStale || plan
      || (operation === 'install' ? !canInstall : !canConfigure)) return;
    operationLock.current = true;
    const token = generation.current;
    try {
      if (operation === 'configure' && !window.confirm(`确定为 ${agent.name} 应用默认 profile 吗？将更新个性化指令与支持的模型配置，写入前会备份相关文件。`)) return;
      setPending(operation);
      setOperationError(null);
      if (operation === 'configure') await onConfigure(agent.id);
      else {
        const result = await onPrepareInstall(agent.id);
        if (token === generation.current) setPlan(result);
      }
    } catch {
      if (token === generation.current) setOperationError('操作未完成，请刷新列表确认当前状态。');
    } finally {
      operationLock.current = false;
      setPending(null);
    }
  };

  const confirmInstall = async () => {
    if (!plan || operationLock.current || busy || readOnly || dataStale || !canInstall) return;
    operationLock.current = true;
    const revision = plan.revision;
    setPlan(null);
    setPending('install');
    try {
      await onInstall(agent.id, revision);
    } catch {
      setOperationError('安装未确认完成，软件可能已安装。请刷新列表确认；不会自动重试、回滚或卸载。');
    } finally {
      operationLock.current = false;
      setPending(null);
    }
  };

  const closeDetails = () => {
    setExpanded(false);
    detailButton.current?.focus();
  };

  return (
    <article
      className={`agent-card${selected ? ' agent-card-selected' : ''}`}
      aria-label={agent.name}
      onKeyDown={(event) => {
        if ((plan || pending === 'install') && event.key === 'Escape') {
          event.stopPropagation();
          cancelPreview();
        } else if (expanded && event.key === 'Escape') {
          event.stopPropagation();
          closeDetails();
        }
      }}
    >
      <div className="agent-row">
        <button
          type="button"
          className="agent-select"
          aria-label={`选择 ${agent.name}`}
          aria-pressed={selected}
          onClick={() => onSelect(agent.id)}
        >
          <span className="agent-avatar" aria-hidden="true">{agent.name.charAt(0)}</span>
          <span className="agent-identity">
            <span className="agent-name-line">
              <span className="agent-name">{agent.name}</span>
              {selected && <span className="agent-selected-mark"><Icon name="check" size={13} />当前选择</span>}
            </span>
            <span className="agent-description">{agent.description || '暂无描述'}</span>
            <span className="agent-meta">
              <span>{getUiTypeLabel(agent.uiType)}</span>
              <span aria-hidden="true">·</span>
              <span>{agent.source === 'open' ? '开源' : '闭源'}</span>
            </span>
          </span>
        </button>

        <div className="agent-state">
          <span className={`status-badge agent-status${evidence.invalid ? ' agent-status-unknown' : installed ? ' agent-status-installed' : installUnknown ? ' agent-status-unknown' : ''}`}>
            <Icon name={evidence.invalid ? 'info' : installed ? 'check' : installUnknown ? 'info' : 'download'} size={13} />
            {status}
          </span>
          {installed && <span className="agent-version">{versionLabel}</span>}
        </div>

        <div className="agent-row-actions">
          {agent.id === 'dsh' && onOpenDshInventory && <button type="button" className="button" onClick={onOpenDshInventory}>打开 DSH 声明盘点</button>}
          <button
            type="button"
            className="button agent-detail-button"
            ref={detailButton}
            aria-label={`${expanded ? '收起' : '查看'} ${agent.name} 详情`}
            aria-expanded={expanded}
            aria-controls={detailsId}
            onClick={() => setExpanded(!expanded)}
          >
            详情 <Icon name="chevron-down" size={14} className={expanded ? 'agent-chevron-open' : undefined} />
          </button>
          <button
            type="button"
            className={`button agent-operation ${agent.installed ? 'button-danger' : 'button-primary'}`}
            disabled={Boolean(blockedReason) || manual}
            title={disabledReason}
            aria-describedby={!agent.installed && configurationReason ? configurationNoteId : undefined}
            data-testid={`install-preview-${agent.id}`}
            aria-label={`${agent.installed ? '卸载' : '安装'} ${agent.name}`}
            onClick={agent.installed ? undefined : () => { void runOperation('install'); }}
          >
            <Icon name={agent.installed ? 'trash' : 'download'} size={14} />
            {pending === 'install' ? '处理中…' : agent.installed ? '手动卸载' : installUnknown || evidence.ambiguous ? '安装不可用' : manual ? '手动安装' : '安装'}
          </button>
          {supportsConfigure && <button
            type="button"
            className="button agent-operation"
            disabled={Boolean(blockedReason) || !canConfigure}
            title={blockedReason || configurationReason || '单独应用默认 profile 的个性化指令与支持的模型配置（写前备份）'}
            aria-describedby={configurationReason ? configurationNoteId : undefined}
            aria-label={`应用默认配置 ${agent.name}`}
            onClick={() => { void runOperation('configure'); }}
          >{pending === 'configure' ? '应用中…' : '应用默认配置'}</button>}
        </div>
      </div>

      {pending === 'install' && !plan && <p role="status">安装操作处理中，请等待当前请求完成。<button type="button" className="button" onClick={cancelPreview}>取消预览展示</button></p>}
      {plan && <section role="dialog" data-testid="install-review-dialog" aria-label={`${agent.name} 安装计划`} className="agent-details">
        <h3>检查 npm 安装计划</h3>
        <dl className="agent-facts">
          {([
            ['npm 包', plan.package], ['启动命令', plan.executable], ['npm prefix', plan.prefix],
            ['npm 全局目录', plan.globalRoot], ['包目录', plan.packageRoot],
            ['启动文件目录', plan.launcherDirectory], ['npm 规范入口', plan.managerPath],
            ['安装器 Node', plan.nodePath],
          ] as const).map(([label, value]) => <div key={label}><dt>{label}</dt><dd className="agent-path">{value}</dd></div>)}
        </dl>
        <p>版本由当前 npm 配置选择，未固定或解析版本；此预览不验证 npm registry 的可信性。</p>
        <p>安装器 Node 仅用于此次安装，不保证 Agent 以后使用相同 Node。</p>
        <p>{plan.scriptPolicy === 'disabled' ? '生命周期脚本：禁用（ignore-scripts）。' : '生命周期脚本：遵循当前 npm 配置，安装时可能执行脚本。'}安装会下载软件，依赖或包的行为取决于 npm 配置。</p>
        <p>仅安装软件，不应用 profile 或模型配置，不创建配置备份。不自动回滚或卸载；失败后可能已有部分软件写入。</p>
        <p>预览仅进行有限的 npm 路径查询，不安装软件、不初始化 profile、vault 或备份；npm 可能读取配置或更新缓存。执行前将重新核对计划，变化后需重新预览。</p>
        <button type="button" className="button" data-testid="install-review-cancel" onClick={cancelPreview}>取消</button>
        <button type="button" className="button button-primary" data-testid="install-review-confirm" disabled={busy || dataStale || readOnly || !canInstall} onClick={() => { void confirmInstall(); }}>确认安装软件</button>
      </section>}

      {manual && (
        <div className="agent-manual-note">
          <Icon name="info" size={14} />
          <span>{manualHelp}</span>
          {agent.installed && <span>{agent.uninstallHelp || '请使用原安装方式或官方卸载说明。'}</span>}
          {homepage && <a href={homepage} target="_blank" rel="noopener noreferrer">官方说明<Icon name="external" size={12} /></a>}
        </div>
      )}
      {configurationReason && <p id={configurationNoteId} className="notice notice-warning agent-ambiguity-note"><Icon name="info" size={14} />{configurationReason}</p>}
      {operationError && <div role="alert" className="notice notice-error agent-operation-error"><Icon name="alert" size={16} />{operationError}</div>}

      {expanded && (
        <section
          id={detailsId}
          className="agent-details"
          aria-label={`${agent.name} 详情`}
        >
          <div className="agent-details-heading">
            <h3>{agent.name} 详情</h3>
            <button type="button" className="button agent-close-details" onClick={closeDetails} aria-label={`关闭 ${agent.name} 详情`}><Icon name="x" size={15} /></button>
          </div>
          <p className="agent-full-description">{agent.description || '暂无描述'}</p>
          <dl className="agent-facts">
            <div><dt>类型</dt><dd>{getUiTypeLabel(agent.uiType)} · {agent.category === 'ai-coding' ? 'AI 编程' : agent.category === 'personal-agent' ? '个人代理' : agent.category || '未提供'}</dd></div>
            <div><dt>安装状态</dt><dd>{status}{installed && ` · ${versionLabel}`}</dd></div>
            <div><dt>自动管理</dt><dd>安装{evidence.ambiguous ? '已暂停（检测证据待确认）' : capabilityLabel(agent.canInstall)} · 配置{evidence.ambiguous ? '已暂停（检测证据待确认）' : capabilityLabel(agent.canConfigure)} · 自动卸载功能尚未提供</dd></div>
            {agent.installationMessage && <div><dt>检测说明</dt><dd>{agent.installationMessage}</dd></div>}
            <div><dt>来源证据</dt><dd>{evidence.active?.source.kind === 'npm' ? '当前 npm 目录关联已匹配，详见实例证据' : '安装来源未验证'}</dd></div>
            <div><dt>本地认证线索</dt><dd>{agent.authVerified ? '检测到本地认证配置' : '未验证'}</dd></div>
            <div><dt>Agent ID</dt><dd>{agent.id}</dd></div>
          </dl>
          <section className="agent-installation-evidence" aria-label={`${agent.name} 安装实例与来源证据`}>
            <h4>安装实例与来源证据</h4>
            <p className="agent-evidence-note">只展示本次检测到的 PATH 候选，不是全盘安装清单。仅当前候选执行版本检测，其他候选不据此推断版本。</p>
            {evidence.probeMessage && evidence.probeMessage !== agent.installationMessage && <p className="agent-evidence-note">检测说明：{evidence.probeMessage}</p>}
            {evidence.message && <p className="agent-evidence-note">{evidence.message}</p>}
            {evidence.instances.length === 0 ? <p className="agent-evidence-note">未提供安装实例证据；路径、来源与实例版本均无法确认。</p> : <>
              <p className="agent-evidence-note">检测到 {evidence.instances.length} 个 PATH 候选{evidence.instances.length > 1 ? `，其中 ${evidence.instances.filter((instance) => !instance.active).length} 个为其他候选` : ''}。{!evidence.active && '未确认唯一的当前候选。'}</p>
              <ol className="agent-instance-list" aria-label="PATH 候选实例">
                {evidence.instances.map((instance, index) => {
                  const current = instance === evidence.active;
                  return <li key={`${instance.path}-${index}`} aria-label={`${current ? '当前' : '其他'} PATH 候选 ${index + 1}`}>
                    <h5>{current ? '当前 PATH 候选' : instance.active ? '候选标记冲突，当前实例未确认' : '其他 PATH 候选（仅观察，未执行）'}</h5>
                    <dl className="agent-facts agent-instance-facts">
                      <div className="agent-evidence-wide"><dt>PATH 路径</dt><dd className="agent-path">{instance.path}</dd></div>
                      <div className="agent-evidence-wide"><dt>规范文件路径</dt><dd className="agent-path">{instance.resolvedPath || '未提供或无法解析'}</dd></div>
                      <div className="agent-evidence-wide"><dt>已证实入口</dt><dd className="agent-path">{instance.entrypointPath || '未证实实际入口，不能由启动文件名推断'}</dd></div>
                      <div className="agent-evidence-wide"><dt>版本检测</dt><dd>{current ? version ? `实测版本 ${version}` : '版本未知（未取得有效版本结果）' : '未检测（其他候选未执行）'}</dd></div>
                      <SourceFacts source={instance.source} />
                    </dl>
                  </li>;
                })}
              </ol>
            </>}
          </section>
          <div className="agent-details-footer">
            <p className="muted">认证线索仅反映本地配置，不代表远端凭据仍有效。{installUnknown && '当前安装状态无法自动检测。'}</p>
            {homepage
              ? <a className="button" href={homepage} target="_blank" rel="noopener noreferrer">官方网站<Icon name="external" size={14} /></a>
              : <span className="muted">未提供 HTTPS 官方链接</span>}
          </div>
          {readOnly && <p className="notice notice-warning agent-readonly-note"><Icon name="info" size={15} />{disabledReason}</p>}
        </section>
      )}
    </article>
  );
};
