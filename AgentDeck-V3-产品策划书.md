# AgentDeck（AI 中枢）V3 产品策划书

> 版本：v1.0　日期：2026-08-11　状态：已评审（grill-me 全分支收束）
> 仓库：shared-ai-config　分支：feature/v3　技术栈：Tauri 2 + Rust 全栈
> 甲方：龙喵　乙方：AI 全栈开发团队

---

## 一、产品概述

### 1.1 一句话定义

**AgentDeck 是一个 AI Agent 编排中枢——一站式发现、安装、配置、记忆管理本地 AI 代理工具。**

### 1.2 核心价值

| 痛点 | AgentDeck 解决方案 |
|------|-------------------|
| 17 款 agent 各自安装，命令各不相同 | Agent 广场一键安装（npm/pip/winget/VS Code 插件） |
| 每个 agent 的 skill/MCP/个性化要手动配置 | 装完自动注入——skill、MCP、个性化、规则记忆 |
| 各 agent 的对话记忆分散，无法跨工具检索 | 统一记忆采集 + SQLite FTS5 全文搜索 + 跨 agent 注入 |
| API Key 分散在十几个配置文件里 | vault AES-256-GCM 统一加密存储 |
| 换个电脑全部重来 | git 驱动备份回滚 + 在线注册表同步 |

### 1.3 与 CC Switch 的关系

互补共存：

| 能力 | CC Switch | AgentDeck |
|------|:---:|:---:|
| 模型供应商管理/一键切换 | ✅ 核心 | ❌ 不做（v3.0） |
| 本地协议代理（Responses↔Chat Completions） | ✅ 核心 | ❌ 不做（v3.0） |
| Agent 一键安装 | ❌ | ✅ 核心 |
| 安装安全闸（白名单/确认/回滚） | ❌ | ✅ |
| 配置自动注入（skill+MCP+个性化+记忆） | 部分 | ✅ 全流程 |
| 跨 agent 统一记忆采集/检索 | 部分 | ✅ 核心 |
| vault 密钥加密 | ❌ | ✅ |
| git 备份回滚 | ❌ | ✅ |

---

## 二、技术架构

### 2.1 整体架构

```
┌─────────────────────────────────────────────────┐
│              AgentDeck Desktop                    │
│  ┌───────────────────────────────────────────┐  │
│  │          React 18 + Vite + Zustand         │  │
│  │   Agent 广场 | 记忆面板 | MCP 管理 | 设置  │  │
│  └──────────────────┬────────────────────────┘  │
│                     │ Tauri IPC (类型安全命令)     │
│  ┌──────────────────▼────────────────────────┐  │
│  │           Tauri 2 + Rust Backend           │  │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────┐ │  │
│  │  │  deploy  │ │  memory  │ │   vault   │ │  │
│  │  │  安装/卸  │ │ 采集/检索 │ │  加解密    │ │  │
│  │  ├──────────┤ ├──────────┤ ├───────────┤ │  │
│  │  │  inject  │ │   sync   │ │  backup   │ │  │
│  │  │ 配置注入  │ │ 五类同步 │ │ git 回滚   │ │  │
│  │  └──────────┘ └──────────┘ └───────────┘ │  │
│  └──────────────────┬────────────────────────┘  │
│                     │                            │
│  ┌──────────────────▼────────────────────────┐  │
│  │         SQLite + FTS5 (本地数据)            │  │
│  │  记忆索引 | Agent 状态 | 用户配置 | vault  │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

### 2.2 技术栈选型

| 层 | 技术 | 理由 |
|----|------|------|
| **桌面壳** | Tauri 2 | 单进程单二进制、三平台、体积小、Rust 原生安全 |
| **后端** | Rust（tokio + serde + rusqlite） | 类型安全、零成本抽象、与 Tauri 天然集成 |
| **前端** | React 18 + TypeScript + Vite + Zustand | 社区最大、中文资料最多、Zustand 极简状态管理 |
| **UI 组件** | shadcn/ui + TailwindCSS | 现代、可定制、暗色主题友好 |
| **数据存储** | SQLite + FTS5（rusqlite） | 零部署、全文搜索、v3.1 可加向量检索 |
| **密钥加密** | AES-256-GCM + DPAPI/Keychain/secret-service | V2 已验证 |
| **打包分发** | tauri-bundler → NSIS（Windows）/ dmg（macOS）/ AppImage（Linux） | Windows 优先 |
| **CI** | GitHub Actions（fmt/clippy/test/vitest/tauri build） | 三平台构建 |

### 2.3 项目结构

```
shared-ai-config/                    ← 仓库根目录
├── src-tauri/                       ← Tauri 2 应用
│   ├── Cargo.toml
│   ├── tauri.conf.json              ← 窗口/安全/更新配置
│   ├── capabilities/
│   └── src/
│       ├── main.rs                  ← 入口
│       └── commands/                ← Tauri 命令层
│           ├── mod.rs
│           ├── deploy.rs            ← Agent 安装/卸载/状态
│           ├── memory.rs            ← 记忆采集/检索/注入
│           ├── vault.rs             ← 密钥加解密
│           ├── sync.rs              ← skill/MCP/个性化同步
│           └── backup.rs            ← git 备份回滚
├── crates/
│   ├── aisync-core/                 ← 核心引擎库
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── deploy/              ← Agent 部署（安装/卸载/验证/白名单）
│   │       ├── memory/              ← 记忆引擎（采集/存储/检索/注入）
│   │       ├── vault/               ← 密钥管理（加解密/keystore/pbkdf2）
│   │       ├── sync/                ← 同步引擎（skill/插件/个性化/MCP）
│   │       ├── config/              ← 配置注入（适配器模式：TOML/JSONC/YAML Writer）
│   │       ├── git/                 ← Git 操作（pull/commit/backup/回滚）
│   │       └── registry/            ← 注册表管理（Agent/模型/MCP）
│   └── aisync-cli/                  ← CLI 二进制（可选）
│       ├── Cargo.toml
│       └── src/main.rs
├── web/ui/                          ← React 前端
│   ├── package.json
│   ├── vite.config.ts
│   ├── index.html
│   └── src/
│       ├── App.tsx                  ← 路由入口
│       ├── main.tsx
│       ├── bridge.ts                ← Tauri 命令调用封装层
│       ├── stores/                  ← Zustand stores
│       │   ├── agentStore.ts        ← Agent 状态管理
│       │   └── memoryStore.ts       ← 记忆面板状态
│       ├── pages/
│       │   ├── AgentSquare.tsx      ← Agent 广场（主界面）
│       │   ├── MemoryPanel.tsx      ← 记忆面板
│       │   ├── McpManager.tsx       ← MCP 管理
│       │   └── Settings.tsx         ← 设置
│       └── components/
│           ├── AgentCard.tsx        ← Agent 卡片组件
│           ├── InstallWizard.tsx    ← 安装向导
│           └── MemorySearch.tsx     ← 记忆搜索
├── agents/
│   ├── registry.json                ← V2 注册表（保留不动）
│   └── registry-v3.json            ← V3 注册表（17 款，9 核心 + 8 预留）
├── models/
│   └── registry.json               ← 模型供应商注册表（延续 V2）
├── mcp-configs/
│   └── registry.json               ← MCP 注册中心（延续 V2）
├── skills/                          ← 共享 Skill 定义（延续 V2，104 个）
├── personalization/                 ← 个性化模板（延续 V2）
│   ├── codex/
│   ├── claude/
│   ├── opencode/
│   ├── hermes/
│   └── ...
├── memory/                          ← 规则记忆文件（延续 V2）
├── docs/
│   └── v3-test-mapping.md           ← 183 条测试迁移映射表
├── desktop/                         ← V2 遗留（不参与 V3 构建）
├── web/app.py                       ← V2 遗留（不参与 V3 构建）
├── scripts/                         ← V2 sync.py（保留至 V3 发布）
├── Cargo.toml                       ← workspace 根
├── V3-开发需求与策划书.md            ← 旧版（保留为历史参考）
└── AgentDeck-V3-产品策划书.md        ← 本文档
```

---

## 三、数据层设计

### 3.1 SQLite 核心表

```sql
-- 记忆索引表
CREATE TABLE memory_entries (
    id TEXT PRIMARY KEY,              -- UUID
    agent_id TEXT NOT NULL,           -- 来源 agent id
    content TEXT NOT NULL,            -- 记忆内容（加密存储可选）
    content_hash TEXT NOT NULL,       -- SHA-256 去重
    collected_at INTEGER NOT NULL,    -- 采集时间戳
    source_session_id TEXT,           -- 来源会话 ID
    auto_inject INTEGER DEFAULT 0,    -- 0=被动 1=自动注入
    tags TEXT DEFAULT '[]',           -- JSON array
    created_at INTEGER NOT NULL       -- DEFAULT (unixepoch())
);

-- FTS5 全文搜索
CREATE VIRTUAL TABLE memory_fts USING fts5(
    content, tags,
    content=memory_entries,
    content_rowid=rowid
);

-- Agent 安装状态表
CREATE TABLE agent_state (
    agent_id TEXT PRIMARY KEY,
    installed INTEGER DEFAULT 0,      -- 0=未安装 1=已安装
    install_version TEXT,             -- 安装版本
    auth_verified INTEGER DEFAULT 0,  -- 认证是否完成
    last_config_sync INTEGER,         -- 最后配置同步时间戳
    last_memory_collect INTEGER,      -- 最后记忆采集时间戳
    updated_at INTEGER NOT NULL       -- DEFAULT (unixepoch())
);

-- 用户配置表（非敏感的持久化偏好）
CREATE TABLE user_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL       -- DEFAULT (unixepoch())
);
```

### 3.2 Agent 注册表字段规范

每条 agent 记录的标准字段（详见 `agents/registry-v3.json`）：

| 字段 | 类型 | 说明 | 示例 |
|------|------|------|------|
| `source` | enum | `"open"` / `"closed"` | 开闭源标识 |
| `license` | string? | 开源协议 | `"MIT"`, `"Apache-2.0"` |
| `github` | string? | GitHub 仓库 | `"anomalyco/opencode"` |
| `stars` | number? | GitHub 星数 | 194000 |
| `ui_type` | enum | `"cli"` / `"desktop"` / `"vscode-extension"` / `"ide-extension"` / `"all"` / `"cli-bot"` | 界面形态 |
| `install.method` | enum | `"npm"` / `"pip"` / `"download"` / `"winget"` / `"vscode_extension"` / `"manual"` | 安装方式 |
| `install.command` | string? | 安装命令 | `"npm install -g opencode-ai"` |
| `install.verify_cmd` | string? | 验证安装命令 | `"opencode --version"` |
| `install.dependencies` | string[] | 前置依赖 | `["node >= 18"]` |
| `auth_type` | enum | `"none"` / `"api_key"` / `"oauth_cli"` / `"account_login"` / `"license"` | 认证方式 |
| `auth_guide.steps` | string[] | 认证引导步骤 | 告诉用户怎么做 |
| `auth_guide.completion_check` | object? | 认证完成检测条件 | `{"file_exists": "~/.codex/auth.json"}` |
| `config_inject.skills` | bool | 是否注入 skill | — |
| `config_inject.mcp` | bool | 是否注入 MCP | — |
| `config_inject.memory` | bool | 是否注入记忆 | — |
| `config_inject.personalization` | object? | 个性化注入配置 | `{"template": "codex", "target": "~/.codex/AGENTS.md"}` |
| `config_inject.model_config` | object? | 模型配置注入 | `{"file": "...", "format": "json_merge"}` |
| `memory_collector` | string? | 记忆采集适配器标识 | `"codex_sqlite"` |
| `icon` | string | 图标标识 | `"codex"` |
| `popular` | bool | 热度标记 | `true` |

---

## 四、Agent 支持清单

### 4.1 核心 9 款（v0.1 交付）

| # | Agent | Stars | UI 类型 | 安装方式 | 认证方式 |
|---|-------|------:|---------|---------|---------|
| 1 | OpenCode | 194k | 全平台 | npm | API Key |
| 2 | Claude Code | 140k | 全平台 | npm | OAuth CLI |
| 3 | Codex | 104k | CLI+桌面 | npm | 账号登录 |
| 4 | Cline | 66k | VS Code | 插件 | API Key |
| 5 | ZCode | — | 桌面 | 下载 | 账号登录 |
| 6 | OpenClaw | 247k | CLI+Bot | 手动 | API Key |
| 7 | Hermes Agent | 218k | CLI | pip | API Key |
| 8 | Gemini CLI | 106k | CLI | npm | OAuth CLI |
| 9 | Pi | 83k | CLI | npm | API Key |

### 4.2 预留 8 款（v0.2+ 接入）

Goose (52k) / Aider (47k) / Continue.dev (35k) / Kilo Code (27k) / Crush (27k) / Roo Code (24k) / Plandex (16k) / Oh-My-Pi (12k)

接入一个新 agent 只需在 `agents/registry-v3.json` 的 `agents` 段加一条 JSON，补充 `install`/`config_inject`/`memory_collector` 字段即可激活。

---

## 五、开发里程碑

### M0 — 骨架搭建（Week 1-2）

**目标：** Cargo workspace + Tauri 脚手架 + 前端骨架 + CI 管道

```
[x] 确认技术栈决策（grill-me 完成）
[x] 创建 feature/v3 分支
[x] 编写 agents/registry-v3.json（9 核心 + 8 预留）
[x] 编写产品策划书（本文档）
[ ] 安装 Rust + Tauri CLI
[ ] cargo init workspace
[ ] npm create vite@latest web/ui -- --template react-ts
[ ] Tauri init（cargo tauri init）
[ ] CI: GitHub Actions（cargo fmt/clippy/test + vitest）
[ ] 验证：三套构建命令全绿
```

### M1 — 核心引擎 + 前端壳（Week 3-6）

**目标：** Agent 广场可操作 + 1 款 agent 完整部署链路

```
[ ] aisync-core 库骨架（deploy/vault/sync/memory 模块）
[ ] React 前端骨架（Agent 广场页面 + AgentCard 组件）
[ ] Tauri IPC 桥接层（bridge.ts + Rust commands）
[ ] Agent 注册表加载与解析
[ ] 首款 Agent（OpenCode）安装→验证→状态更新 闭环
[ ] 验证：OpenCode 一键安装成功 + Agent 广场卡片状态正确
```

### M2 — 配置注入引擎（Week 7-10）

**目标：** 全部 9 款核心 agent 的配置注入适配器

```
[ ] 适配器模式实现（TOML Writer / JSONC Merge / YAML Section / JSON Merge）
[ ] 个性化模板注入（codex/claude/opencode/hermes/gemini）
[ ] Agent 安装→注入 原子化流程（失败回滚）
[ ] 9 款核心 agent 配置注入全绿
[ ] 验证：每款 agent 安装后个性化/skill/MCP 配置文件已就位
```

### M3 — 记忆引擎（Week 11-14）

**目标：** SQLite FTS5 记忆存储 + 采集 + 检索 + 注入

```
[ ] SQLite schema 建表 + FTS5 索引
[ ] Collector 适配器接口 + codex_sqlite / claude_sessions 实现
[ ] 记忆采集（手动触发 + 定时 cron）
[ ] 记忆搜索（FTS5 全文搜索 + 前端搜索框）
[ ] 记忆注入（被动 + 自动标签双模式）
[ ] 验证：从 Codex 采集对话记忆 → 搜索 → 注入到 Claude Code
```

### M4 — vault + 备份（Week 15-17）

**目标：** V2 vault 移植 + git 备份回滚

```
[ ] vault 模块（AES-256-GCM + DPAPI/Keychain/secret-service）
[ ] V2 vault 导入兼容
[ ] API Key 管理 UI（添加/删除/查看）
[ ] git 备份（手动+自动）
[ ] git 回滚（按时间戳恢复）
[ ] 验证：V2 vault/备份测试 1:1 映射全绿
```

### M5 — MCP 管理 + 全功能集成（Week 18-20）

**目标：** MCP 跨工具面板 + 全流程联调

```
[ ] MCP 注册表加载
[ ] MCP 面板 UI（按 agent 独立开关）
[ ] MCP 配置注入（TOML section / JSON merge / YAML section）
[ ] 9 款 agent 端到端测试（安装→注入→记忆→MCP）
[ ] 前端 UX 打磨（引导页、空状态、错误提示）
[ ] 验证：全部 9 款 agent 端到端冒烟通过
```

### M6 — 发布（Week 21-24）

**目标：** Windows 安装包 + 注册表在线更新 + 发布 v3.0.0

```
[ ] tauri-bundler Windows NSIS 打包（可选安装目录）
[ ] 注册表在线更新机制（GitHub Releases → 本地合并）
[ ] ed25519 签名更新链
[ ] README/VERSION 同步
[ ] 冒烟测试 + SHA256 校验
[ ] tag v3.0.0 + 归档
```

---

## 六、开发规范

### 6.1 Rust 代码规范

- `cargo fmt` + `cargo clippy` 零告警作为提交门槛
- 所有 pub 函数带 docstring（`///` 注释）
- 错误处理用 `thiserror` 定义错误类型，禁止裸 `unwrap()`
- 数据库操作走 DAO 层，不裸写 SQL
- 模块边界：`aisync-core` 不依赖 Tauri，`src-tauri` 通过 commands 调用 core

### 6.2 前端代码规范

- 组件：一个组件一个文件，props 带 TypeScript 类型
- 状态管理：Zustand store 按领域拆分（agentStore / memoryStore / vaultStore）
- Tauri 命令调用：统一走 `bridge.ts` 封装，不从组件直接 `invoke()`
- 所有用户可见文案使用简体中文

### 6.3 安全红线（继承 Codex AGENTS.md）

1. 凭据不进仓库——vault 加密存储，占位符化后入库
2. 安装命令白名单校验——仅允许注册表声明的 `install.command`
3. 破坏性操作确认——删除 agent、回滚配置、git reset 前弹窗确认
4. 日志脱敏——不输出 API Key、token、密码到日志

### 6.4 Git 规范

- 分支：`feature/v3` 开发 → `main` 仅发布
- Commit：中文 Conventional Commits（`feat: xxx` / `fix: xxx` / `docs: xxx`）
- 不跳过 hooks（`--no-verify` 禁止）

---

## 七、附录

### A. 决策记录（grill-me 全分支收束）

详见开发前准备清单 `D:\WORKBUDDY\2026-08-11-14-57-30\AgentDeck-开发前准备清单.md`

### B. 测试迁移映射

V2 的 141 pytest + 21 前端 + 21 桌面测试需 1:1 映射到 Rust 单测/集成/E2E，映射表维护在 `docs/v3-test-mapping.md`。

### C. 第三方依赖审计

引入新 crate 前需确认：license 兼容（MIT/Apache-2.0/BSD）、维护活跃（近 3 月有 commit）、安全无已知漏洞（cargo audit）。
