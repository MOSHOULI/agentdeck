/**
 * Agent 卡片组件。
 * 展示单个 agent 的安装状态、热度星级、操作按钮。
 */

import React from 'react';
import type { AgentInfo } from '../bridge';

interface AgentCardProps {
  agent: AgentInfo;
  onInstall: (id: string) => void;
  onUninstall: (id: string) => void;
  onSelect: (id: string) => void;
  selected: boolean;
}

function formatStars(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(0)}k`;
  return String(n);
}

function getSourceBadge(source: 'open' | 'closed'): { text: string; color: string } {
  return source === 'open'
    ? { text: '开源', color: '#22c55e' }
    : { text: '闭源', color: '#f59e0b' };
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

export const AgentCard: React.FC<AgentCardProps> = ({
  agent,
  onInstall,
  onUninstall,
  onSelect,
  selected,
}) => {
  const { text: sourceText, color: sourceColor } = getSourceBadge(agent.source);

  return (
    <div
      onClick={() => onSelect(agent.id)}
      style={{
        border: selected ? '2px solid #6366f1' : '1px solid #374151',
        borderRadius: 12,
        padding: 20,
        background: selected ? '#1e1b4b' : '#1f2937',
        cursor: 'pointer',
        transition: 'all 0.2s',
        display: 'flex',
        flexDirection: 'column',
        gap: 12,
        minHeight: 220,
        position: 'relative',
      }}
    >
      {/* 顶部：图标 + 名称 + 热度 */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <div
          style={{
            width: 48,
            height: 48,
            borderRadius: 10,
            background: 'linear-gradient(135deg, #6366f1, #8b5cf6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 22,
            fontWeight: 700,
            color: '#fff',
          }}
        >
          {agent.name.charAt(0)}
        </div>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: 16, fontWeight: 600, color: '#f3f4f6' }}>
            {agent.name}
          </div>
          <div style={{ fontSize: 12, color: '#9ca3af', marginTop: 2 }}>
            {getUiTypeLabel(agent.uiType)}
          </div>
        </div>
        {agent.stars > 0 && (
          <div style={{ fontSize: 13, color: '#fbbf24', fontWeight: 600 }}>
            ★ {formatStars(agent.stars)}
          </div>
        )}
      </div>

      {/* 标签 */}
      <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
        <span
          style={{
            fontSize: 11,
            padding: '2px 8px',
            borderRadius: 99,
            background: `${sourceColor}20`,
            color: sourceColor,
            fontWeight: 500,
          }}
        >
          {sourceText}
        </span>
        <span
          style={{
            fontSize: 11,
            padding: '2px 8px',
            borderRadius: 99,
            background: '#374151',
            color: '#d1d5db',
          }}
        >
          {agent.category === 'ai-coding' ? 'AI 编程' : '个人代理'}
        </span>
      </div>

      {/* 描述 */}
      <p style={{ fontSize: 13, color: '#9ca3af', lineHeight: 1.5, flex: 1, margin: 0 }}>
        {agent.description}
      </p>

      {/* 状态 + 操作 */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginTop: 'auto',
        }}
      >
        <div style={{ fontSize: 12, color: '#6b7280' }}>
          {agent.installed ? (
            <span style={{ color: '#22c55e' }}>
              ✓ 已安装{agent.version ? ` v${agent.version}` : ''}
            </span>
          ) : (
            '未安装'
          )}
        </div>

        {agent.installed ? (
          <button
            onClick={(e) => {
              e.stopPropagation();
              if (window.confirm(`确定卸载 ${agent.name} 吗？`)) {
                onUninstall(agent.id);
              }
            }}
            style={{
              padding: '6px 14px',
              borderRadius: 8,
              border: '1px solid #ef4444',
              background: 'transparent',
              color: '#ef4444',
              fontSize: 12,
              cursor: 'pointer',
              fontWeight: 500,
            }}
          >
            卸载
          </button>
        ) : (
          <button
            onClick={(e) => {
              e.stopPropagation();
              if (window.confirm(`确定安装 ${agent.name} 吗？`)) {
                onInstall(agent.id);
              }
            }}
            style={{
              padding: '6px 14px',
              borderRadius: 8,
              border: 'none',
              background: 'linear-gradient(135deg, #6366f1, #8b5cf6)',
              color: '#fff',
              fontSize: 12,
              cursor: 'pointer',
              fontWeight: 500,
            }}
          >
            一键安装
          </button>
        )}
      </div>
    </div>
  );
};
