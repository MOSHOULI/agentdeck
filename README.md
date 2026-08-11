# AgentDeck（AI 中枢）

AgentDeck 是一个跨平台桌面应用，定位为 **Agent 编排中枢**——一站式发现、安装、配置、记忆管理本地 AI 代理工具。

与 CC Switch 互补共存：AgentDeck 负责「安装 → 注入 → 记忆」，CC Switch 负责「模型切换 → 协议代理」。

## 四大核心能力

1. **Agent 广场**：Steam 风格卡片展示 AI Agent，一键安装/卸载/配置注入
2. **统一记忆**：跨 agent 采集对话历史 → SQLite FTS5 全文搜索 → 跨 agent 注入
3. **vault 加密**：AES-256-GCM 统一管理所有 agent 的 API Key
4. **git 备份回滚**：每次变动前自动备份，可回滚

## 技术栈

| 层 | 技术 |
|----|------|
| 桌面壳 | Tauri 2（单进程单二进制） |
| 后端 | Rust（aisync-core 引擎库） |
| 前端 | React 18 + TypeScript + Vite + Zustand |
| 数据 | SQLite + FTS5（rusqlite bundled） |
| 打包 | tauri-bundler → NSIS（Windows 优先） |

## 当前状态（2026-08-11）

- ✅ **M0 完成**：`cargo tauri dev` 可弹出 1200×800 的 Agent 广场窗口；质量门全绿（cargo fmt/clippy/test、vitest、`npm run build`、`npx tsc --noEmit`）；GitHub Actions CI 已就位
- ✅ **M1 完成**：广场真实显示注册表 9 款 Agent；安装/卸载/验证闭环已接线（真实 npm 全局安装 opencode-ai 验证通过，测试后已还原）；安装/卸载前有确认弹窗
- ✅ **M2 完成**：配置注入引擎（JSON/JSONC/TOML/YAML/文本适配器 + 备份回滚）；codex/opencode 已真实注入成功（备份在 `~/.agentdeck/backups/`）；其余 7 款因数据源缺模板/特殊通道明确跳过（逐项报告）
- ✅ **M3 核心完成**：记忆引擎（SQLite FTS5 存储/搜索，Codex/Claude/OpenCode 采集器，记忆注入文件）；前端记忆管理面板（采集/搜索/自动注入/注入）；真实采集验证 384 条；opencode 代码审查发现的 8 项问题已修复
- ⏳ **M3 剩余**：定时采集（cron）、自动标签注入；之后 M4（vault 加密 + git 备份回滚）

## 环境要求（本机已就绪）

- Windows + PowerShell 7
- Rust 1.97.1（MSVC 工具链）+ VS2022 Build Tools
- Node 24 LTS + npm 11
- tauri-cli 2.11（`cargo tauri`）
- cargo 已配置 USTC 镜像与关闭证书吊销检查（`C:\Users\CLUSTER\.cargo\config.toml`）

## 常用命令

在仓库根目录 `D:\BASED CODE\AGENTDECK\agentdeck` 执行：

```powershell
cargo tauri dev              # 启动开发窗口
cargo check --workspace      # 编译检查
cargo test --workspace       # Rust 测试
cargo clippy --workspace -- -D warnings   # 静态检查
cargo fmt --check            # 格式检查
```

前端（`web/ui` 目录下）：

```powershell
npm run build                # 生产构建
npm test                     # vitest 测试
npx tsc --noEmit             # 类型检查
```

## 目录结构

```
agentdeck/
├── Cargo.toml                       ← workspace 根（3 个 member）
├── AgentDeck-V3-产品策划书.md        ← 完整产品方案（必读）
├── agents/registry-v3.json          ← Agent 注册表（9 款核心）
├── crates/
│   ├── aisync-core/                 ← 核心引擎（deploy/memory/vault/sync/config/git/registry）
│   └── aisync-cli/                  ← CLI 入口
├── src-tauri/                       ← Tauri 2 桌面壳
│   ├── tauri.conf.json              ← 窗口/安全/打包配置
│   ├── icons/                       ← 应用图标（本地生成，构建必需）
│   └── src/commands/                ← 5 类 Tauri 命令
└── web/ui/                          ← React 前端
    ├── tsconfig.json
    └── src/
        ├── bridge.ts                ← Tauri IPC 桥接层
        ├── stores/agentStore.ts     ← Zustand 状态管理
        ├── pages/AgentSquare.tsx    ← Agent 广场主界面
        ├── components/AgentCard.tsx ← Agent 卡片组件
        ├── App.tsx / App.css
        └── main.jsx / styles.css
```

## 关键文档与数据源

- `AgentDeck-V3-产品策划书.md`：完整产品方案（7 章、6 个里程碑 M0-M6），**必读**
- `agents/registry-v3.json`：9 款核心 Agent 定义（8 款预留接口规划中）
- 外部数据源：`D:\BASED CODE\SHARED\shared-ai-config`（skills / personalization / mcp-configs / memory 在此定义）

## 提交约定

- 中文 Conventional Commits（`feat:` / `fix:` / `chore:` / `docs:` / `test:` / `ci:` / `style:`）
- 里程碑推进时同步更新本文档「当前状态」与「下一步」

---

## 给 Codex 的启动提示

你是 Codex，角色为乙方全栈开发团队；甲方是龙喵（老大），零基础、不写代码，所有开发由你完成。

协作规则：

- 称呼「老大」，不客套，先自查再问
- 涉及方案/计划先走 grill-me 审查，确认后再动手
- 修改代码前先读相关依赖，不重复造轮子
- 每个阶段结束做 code review 与验证，提交用中文 Conventional Commits
- 文档同步：功能、接口、数据结构变化时同步更新 README 与产品文档

开始任务前先读：

1. `AgentDeck-V3-产品策划书.md` 全文
2. `agents/registry-v3.json`（Agent 注册表结构）
3. `README.md` 当前状态与常用命令

当前下一步：M0 收尾（质量门 + CI + vitest）→ M1（广场显示 9 款 Agent + OpenCode 安装闭环）。
