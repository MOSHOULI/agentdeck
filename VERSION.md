# AgentDeck 版本记录

## v1.0.0（2026-08-11）

首个正式发布版本（版本号重新开始计数）。

- M0：Tauri 桌面壳 + 质量门（fmt/clippy/test/vitest/build/tsc）+ CI
- M1：Agent 广场（9 款注册表 + 安装/卸载/验证闭环 + 确认弹窗）
- M2：配置注入引擎（JSON/JSONC/TOML/YAML/文本适配器 + 备份回滚）
- M3：记忆引擎（SQLite FTS5 + 三源采集 + 定时采集 + 自动标签注入）
- M4：vault 加密（DPAPI + AES-256-GCM + V2 导入）+ git 事件驱动备份回滚
- M5：MCP 管理（内置注册表 + 按 agent 开关实时写回 + 前端面板）
- M6：Windows NSIS 打包 + 注册表在线更新（ed25519 签名）

验证：六项质量门全绿（cargo fmt/clippy/test、vitest、build、tsc）；真实端到端（安装/注入/记忆/MCP/vault）通过。
