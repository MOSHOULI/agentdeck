import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { AgentSquare } from './pages/AgentSquare';
import { QwenSettings } from './pages/QwenSettings';
import { DshInventory } from './pages/DshInventory';
import { MemoryPanel } from './components/MemoryPanel';
import { McpPanel } from './components/McpPanel';
import { VaultPanel } from './components/VaultPanel';
import { UpdatePanel } from './components/UpdatePanel';
import { Icon } from './components/Icon';
import { runtimeMode } from './bridge';
import { useAgentStore } from './stores/agentStore';
import { useWorkspaceActivity, useWorkspaceActivityStore } from './stores/workspaceActivity';
import type { WorkspaceId } from './stores/workspaceActivity';
import './App.css';

type Theme = 'system' | 'light' | 'dark';

const workspaces: { id: WorkspaceId; label: string; component: typeof AgentSquare }[] = [
  { id: 'agents', label: 'Agent 管理', component: AgentSquare },
  { id: 'memory', label: '记忆管理', component: MemoryPanel },
  { id: 'mcp', label: 'MCP 服务', component: McpPanel },
  { id: 'vault', label: '密钥与备份', component: VaultPanel },
  { id: 'qwen', label: 'Qwen 配置检查', component: QwenSettings },
  { id: 'dsh', label: 'DSH 声明盘点', component: DshInventory },
  { id: 'updates', label: '注册表更新', component: UpdatePanel },
];

const themes: { id: Theme; label: string; icon: string }[] = [
  { id: 'system', label: '跟随系统', icon: 'monitor' },
  { id: 'light', label: '浅色', icon: 'sun' },
  { id: 'dark', label: '深色', icon: 'moon' },
];

function readPreference(key: string): string | null {
  try { return window.localStorage.getItem(key); } catch { return null; }
}

function savePreference(key: string, value: string): void {
  try { window.localStorage.setItem(key, value); } catch { /* In-memory controls remain usable. */ }
}

function initialTheme(): Theme {
  const saved = readPreference('agentdeck.theme');
  return saved === 'light' || saved === 'dark' ? saved : 'system';
}

export default function App() {
  const [workspace, setWorkspace] = useState<WorkspaceId>('agents');
  const [collapsed, setCollapsed] = useState(() => readPreference('agentdeck.sidebar-collapsed') === 'true');
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const navRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const busy = useWorkspaceActivityStore((state) => state.busy);
  const agentsLoading = useAgentStore((state) => state.loading);
  useWorkspaceActivity('agents', agentsLoading);

  const mode = runtimeMode();
  const current = workspaces.find((item) => item.id === workspace)!;
  const activeWorkspaces = workspaces.filter((item) => busy[item.id]);
  const activityLabel = activeWorkspaces.map((item) => item.label).join('、');
  const runtimeLabel = mode === 'native' ? '本地桌面模式' : mode === 'demo' ? '只读演示' : '桌面后端未连接';

  useEffect(() => {
    const media = window.matchMedia?.('(prefers-color-scheme: dark)');
    const applyTheme = () => {
      document.documentElement.dataset.theme = theme === 'system' ? (media?.matches ? 'dark' : 'light') : theme;
    };
    applyTheme();
    media?.addEventListener?.('change', applyTheme);
    return () => {
      media?.removeEventListener?.('change', applyTheme);
      delete document.documentElement.dataset.theme;
    };
  }, [theme]);

  const navigateWithKeyboard = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    let next: number;
    if (event.key === 'ArrowDown') next = (index + 1) % workspaces.length;
    else if (event.key === 'ArrowUp') next = (index + workspaces.length - 1) % workspaces.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = workspaces.length - 1;
    else return;
    event.preventDefault();
    setWorkspace(workspaces[next].id);
    navRefs.current[next]?.focus();
  };

  const toggleSidebar = () => {
    const next = !collapsed;
    setCollapsed(next);
    savePreference('agentdeck.sidebar-collapsed', String(next));
  };

  return (
    <div className={`app${collapsed ? ' app-sidebar-collapsed' : ''}`}>
      <a className="skip-link" href="#workspace-content">跳到工作区内容</a>
      <aside className="app-sidebar" aria-label="应用导航">
        <div className="app-brand">
          <span className="app-brand-mark"><Icon name="agents" size={22} /></span>
          <span className="app-brand-copy">AgentDeck<span>本地 AI 工作台</span></span>
        </div>

        <div className="app-nav-section-label" aria-hidden="true">工作空间</div>
        <div className="app-workspace-nav" role="tablist" aria-label="工作空间" aria-orientation="vertical">
          {workspaces.map((item, index) => (
            <button
              key={item.id}
              ref={(node) => { navRefs.current[index] = node; }}
              id={`workspace-tab-${item.id}`}
              className={`app-nav-item${workspace === item.id ? ' is-active' : ''}`}
              type="button"
              role="tab"
              aria-selected={workspace === item.id}
              aria-controls={`workspace-panel-${item.id}`}
              aria-label={`${item.label}${busy[item.id] ? '，操作进行中' : ''}`}
              tabIndex={workspace === item.id ? 0 : -1}
              title={collapsed ? `${item.label}${busy[item.id] ? ' · 进行中' : ''}` : undefined}
              onClick={() => setWorkspace(item.id)}
              onKeyDown={(event) => navigateWithKeyboard(event, index)}
            >
              <Icon name={(item.id === 'dsh' || item.id === 'qwen') ? 'search' : item.id} size={19} />
              <span className="app-nav-label">{item.label}</span>
              {busy[item.id] && <span className="app-nav-activity" aria-hidden="true"><Icon name="refresh" size={13} className="activity-spinner" /></span>}
            </button>
          ))}
        </div>

        <div className="app-sidebar-footer">
          <div className="app-runtime-state" title={runtimeLabel}>
            <Icon name={mode === 'native' ? 'monitor' : 'info'} size={16} />
            <span>{runtimeLabel}</span>
          </div>
          <div className="app-theme-control" role="group" aria-label="外观主题">
            {themes.map((item) => (
              <button
                key={item.id}
                type="button"
                className="app-theme-button"
                aria-label={item.label}
                aria-pressed={theme === item.id}
                title={item.label}
                onClick={() => { setTheme(item.id); savePreference('agentdeck.theme', item.id); }}
              >
                <Icon name={item.icon} size={17} />
                <span className="app-theme-label">{item.id === 'system' ? '系统' : item.label}</span>
              </button>
            ))}
          </div>
          <button className="app-collapse-button" type="button" onClick={toggleSidebar} aria-label={collapsed ? '展开侧栏' : '收起侧栏'} aria-expanded={!collapsed}>
            <Icon name={collapsed ? 'chevron-right' : 'chevron-left'} size={17} />
            <span>收起侧栏</span>
          </button>
        </div>
      </aside>

      <div className="app-main">
        <header className="app-header">
          <div className="app-breadcrumb"><span>工作空间</span><Icon name="chevron-right" size={13} /><strong>{current.label}</strong></div>
          <div className="app-activity-status" role="status" aria-live="polite" aria-atomic="true">
            {activeWorkspaces.length > 0 && <><Icon name="refresh" size={14} className="activity-spinner" /><span title={`进行中：${activityLabel}`}>进行中：{activityLabel}</span></>}
          </div>
        </header>
        {mode !== 'native' && (
          <div role="status" className="runtime-banner">
            <Icon name="info" size={16} />
            <span>{mode === 'demo'
              ? '只读演示模式 · 所有数据均为示例，不代表本机状态；操作不会执行。'
              : '未连接桌面后端 · 请在 AgentDeck 桌面应用中运行。'}</span>
          </div>
        )}
        <main id="workspace-content" className="app-content" tabIndex={-1}>
          {workspaces.map(({ id, component: Workspace }) => (
            // Keep mounted: navigation must never reset in-flight operation locks or form state.
            <section
              key={id}
              id={`workspace-panel-${id}`}
              role="tabpanel"
              aria-labelledby={`workspace-tab-${id}`}
              hidden={workspace !== id}
              tabIndex={0}
              className="app-workspace"
            >
              <Workspace />
            </section>
          ))}
        </main>
      </div>
    </div>
  );
}
