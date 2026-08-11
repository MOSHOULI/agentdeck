/**
 * AgentDeck 应用入口。
 * 暗色主题，Agent 广场为主界面。
 */

import React from 'react';
import { AgentSquare } from './pages/AgentSquare';
import './App.css';

const App: React.FC = () => {
  return (
    <div className="app">
      <AgentSquare />
    </div>
  );
};

export default App;
