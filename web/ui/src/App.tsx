/**
 * AgentDeck 应用入口。
 * 暗色主题，Agent 广场为主界面。
 */

import React from 'react';
import { AgentSquare } from './pages/AgentSquare';
import './App.css';
import { runtimeMode } from './bridge';

const App: React.FC = () => {
  return (
    <div className="app">
      {runtimeMode() !== 'native' && (
        <div role="status" className="runtime-banner">
          {runtimeMode() === 'demo'
            ? '只读演示模式 · 所有数据均为示例，不代表本机状态；操作不会执行。'
            : '未连接桌面后端 · 请在 AgentDeck 桌面应用中运行。'}
        </div>
      )}
      <AgentSquare />
    </div>
  );
};

export default App;
