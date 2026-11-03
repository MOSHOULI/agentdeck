import React, { useId, useRef, useState } from 'react';
import { runtimeMode, type AgentInfo } from '../bridge';
import { Icon } from './Icon';

interface AgentCardProps {
  agent: AgentInfo;
  onInstall: (id: string) => void | Promise<void>;
  onUninstall: (id: string) => void | Promise<void>;
  onSelect: (id: string) => void;
  selected: boolean;
  busy?: boolean;
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
  onUninstall,
  onSelect,
  selected,
  busy = false,
}) => {
  const [expanded, setExpanded] = useState(false);
  const [pending, setPending] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);
  const operationLock = useRef(false);
  const detailButton = useRef<HTMLButtonElement>(null);
  const detailsId = useId();
  const mode = runtimeMode();
  const readOnly = mode !== 'native';
  const homepage = officialHomepage(agent.homepage);
  const manual = agent.installed ? agent.canUninstall !== true : agent.canInstall !== true;
  const installUnknown = !agent.installed && agent.canVerifyInstall !== true;
  const status = agent.installed ? '已安装' : installUnknown ? '安装状态未知' : '未安装';
  const manualHelp = agent.installed
    ? '此 Agent 不支持自动卸载，请使用官方卸载方式。'
    : agent.installHelp || '此 Agent 暂不支持自动安装，请参阅官方安装说明。';
  const disabledReason = readOnly
    ? mode === 'demo' ? '只读演示：请在桌面应用中管理安装。' : '未连接桌面后端，请通过桌面应用管理安装。'
    : busy || pending ? '请等待当前操作完成。' : manual ? manualHelp : undefined;

  const runOperation = async () => {
    if (operationLock.current || busy || readOnly || manual) return;
    operationLock.current = true;
    const question = agent.installed
      ? `确定卸载 ${agent.name} 吗？`
      : `确定安装 ${agent.name} 吗？安装后会应用默认 profile，更新个性化指令与支持的模型配置（写前备份）。`;
    try {
      if (!window.confirm(question)) return;
      setPending(true);
      setOperationError(null);
      await (agent.installed ? onUninstall(agent.id) : onInstall(agent.id));
    } catch (error) {
      setOperationError(`操作未完成：${error instanceof Error ? error.message : String(error)}。请刷新列表确认当前状态。`);
    } finally {
      operationLock.current = false;
      setPending(false);
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
          <span className={`status-badge agent-status${agent.installed ? ' agent-status-installed' : installUnknown ? ' agent-status-unknown' : ''}`}>
            <Icon name={agent.installed ? 'check' : installUnknown ? 'info' : 'download'} size={13} />
            {status}
          </span>
          {agent.installed && <span className="agent-version">{agent.version ? `v${agent.version.replace(/^v/i, '')}` : '版本未知'}</span>}
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
            disabled={busy || pending || readOnly || manual}
            title={disabledReason}
            aria-label={`${agent.installed ? '卸载' : '安装'} ${agent.name}`}
            onClick={() => { void runOperation(); }}
          >
            <Icon name={agent.installed ? 'trash' : 'download'} size={14} />
            {pending ? '处理中…' : manual ? agent.installed ? '手动卸载' : '手动安装' : agent.installed ? '卸载' : '安装'}
          </button>
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
            <div><dt>安装状态</dt><dd>{status}{agent.installed && ` · ${agent.version ? `版本 ${agent.version}` : '版本未知'}`}</dd></div>
            <div><dt>自动管理</dt><dd>安装{capabilityLabel(agent.canInstall)} · 卸载{capabilityLabel(agent.canUninstall)}</dd></div>
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
