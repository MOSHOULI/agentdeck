import React, { useId, useRef, useState } from 'react';
import { runtimeMode, type AgentInfo } from '../bridge';
import { Icon } from './Icon';

interface AgentCardProps {
  agent: AgentInfo;
  onInstall: (id: string) => void | Promise<void>;
  onUninstall: (id: string) => void | Promise<void>;
  onConfigure: (id: string) => void | Promise<void>;
  onSelect: (id: string) => void;
  selected: boolean;
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

/** A compact row with separate native controls for selection, details and actions. */
export const AgentCard: React.FC<AgentCardProps> = ({
  agent,
  onInstall,
  onConfigure,
  onSelect,
  selected,
  busy = false,
  dataStale = true,
}) => {
  const [expanded, setExpanded] = useState(false);
  const [pending, setPending] = useState<'install' | 'configure' | null>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const operationLock = useRef(false);
  const detailButton = useRef<HTMLButtonElement>(null);
  const detailsId = useId();
  const mode = runtimeMode();
  const readOnly = mode !== 'native';
  const homepage = officialHomepage(agent.homepage);
  const installed = agent.installed && agent.installationStatus === 'installed';
  const notDetected = !agent.installed && agent.installationStatus === 'not_detected';
  const installUnknown = !installed && !notDetected;
  const canInstall = notDetected && agent.canInstall === true;
  const canConfigure = installed && agent.canConfigure === true;
  // Current sources are all unverified, even if a legacy backend advertises
  // canUninstall. Never treat PATH detection as proof of ownership.
  const manual = agent.installed || !canInstall;
  const status = installed ? '已安装' : notDetected ? 'PATH 未检测到'
    : agent.installationStatus === 'error' ? '检测出错' : '安装状态未知';
  const manualHelp = agent.installed
    ? agent.uninstallHelp || '安装来源未验证，无法自动卸载。请使用原安装方式或官方卸载说明。'
    : installUnknown
      ? [agent.installationMessage || '无法确认本地安装状态，自动安装已暂停。请刷新列表或查看官方说明。', agent.installHelp].filter(Boolean).join(' ')
      : agent.installHelp || '此 Agent 暂不支持自动安装，请参阅官方安装说明。';
  const blockedReason = readOnly
    ? mode === 'demo' ? '只读演示：请在桌面应用中管理安装与配置。' : '未连接桌面后端，请通过桌面应用管理安装与配置。'
    : busy || pending ? '请等待当前操作完成。'
      : dataStale ? '当前状态待刷新，请成功刷新列表后再操作。' : undefined;
  const disabledReason = blockedReason || (manual ? manualHelp : undefined);

  const runOperation = async (operation: 'install' | 'configure') => {
    if (operationLock.current || busy || readOnly || dataStale
      || (operation === 'install' ? !canInstall : !canConfigure)) return;
    operationLock.current = true;
    const question = operation === 'configure'
      ? `确定为 ${agent.name} 应用默认 profile 吗？将更新个性化指令与支持的模型配置，写入前会备份相关文件。`
      : `确定安装 ${agent.name} 吗？此操作仅安装软件，不应用默认 profile 或修改模型配置。`;
    try {
      if (!window.confirm(question)) return;
      setPending(operation);
      setOperationError(null);
      await (operation === 'configure' ? onConfigure(agent.id) : onInstall(agent.id));
    } catch (error) {
      setOperationError(`操作未完成：${error instanceof Error ? error.message : String(error)}。请刷新列表确认当前状态。`);
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
        if (expanded && event.key === 'Escape') {
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
              {agent.popular && <><span aria-hidden="true">·</span><span>热度推荐</span></>}
            </span>
          </span>
        </button>

        <div className="agent-state">
          <span className={`status-badge agent-status${installed ? ' agent-status-installed' : installUnknown ? ' agent-status-unknown' : ''}`}>
            <Icon name={installed ? 'check' : installUnknown ? 'info' : 'download'} size={13} />
            {status}
          </span>
          {installed && <span className="agent-version">{agent.version ? `v${agent.version.replace(/^v/i, '')}` : '版本未知'}</span>}
        </div>

        <div className="agent-row-actions">
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
            aria-label={`${agent.installed ? '卸载' : '安装'} ${agent.name}`}
            onClick={agent.installed ? undefined : () => { void runOperation('install'); }}
          >
            <Icon name={agent.installed ? 'trash' : 'download'} size={14} />
            {pending === 'install' ? '安装中…' : agent.installed ? '手动卸载' : installUnknown ? '安装不可用' : manual ? '手动安装' : '安装'}
          </button>
          {canConfigure && <button
            type="button"
            className="button agent-operation"
            disabled={Boolean(blockedReason)}
            title={blockedReason || '单独应用默认 profile 的个性化指令与支持的模型配置（写前备份）'}
            aria-label={`应用默认配置 ${agent.name}`}
            onClick={() => { void runOperation('configure'); }}
          >{pending === 'configure' ? '应用中…' : '应用默认配置'}</button>}
        </div>
      </div>

      {manual && (
        <div className="agent-manual-note">
          <Icon name="info" size={14} />
          <span>{manualHelp}</span>
          {homepage && <a href={homepage} target="_blank" rel="noopener noreferrer">官方说明<Icon name="external" size={12} /></a>}
        </div>
      )}
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
            <div><dt>安装状态</dt><dd>{status}{installed && ` · ${agent.version ? `版本 ${agent.version}` : '版本未知'}`}</dd></div>
            <div><dt>自动管理</dt><dd>安装{capabilityLabel(agent.canInstall)} · 配置{capabilityLabel(agent.canConfigure)} · 自动卸载不可用（来源未验证）</dd></div>
            {agent.installationMessage && <div><dt>检测说明</dt><dd>{agent.installationMessage}</dd></div>}
            <div><dt>安装来源</dt><dd>未验证，自动卸载已停用</dd></div>
            <div><dt>本地认证线索</dt><dd>{agent.authVerified ? '检测到本地认证配置' : '未验证'}</dd></div>
            <div><dt>注册表热度</dt><dd>{agent.stars > 0 ? `${agent.stars.toLocaleString()} stars` : '未提供'}<span className="agent-fact-note">注册表记录，非实时数据</span></dd></div>
            <div><dt>Agent ID</dt><dd>{agent.id}</dd></div>
          </dl>
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
