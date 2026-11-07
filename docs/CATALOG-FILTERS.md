# Agent catalog filters / Agent 目录筛选

## 简体中文

Agent 广场的「支持能力与状态」可与「全部 / 已安装」及搜索组合使用，所有条件需要同时满足。筛选只处理当前列表，不执行 Agent、安装器或新的后端请求；不会更改安装、配置或卸载权限。

- **当前可自动安装**：仅在已连接桌面后端、检测已成功刷新且无操作进行时，展示明确支持自动安装、当前 PATH 未检测到、检测证据无冲突的条目。仍需用户点击并确认安装。PATH 未检测到不代表设备上完全不存在。
- **仅手动安装**：目录明确不支持自动安装的条目。这是能力分类，不代表尚未安装，也不保证官方指引已经运行验证。旧后端缺失能力字段时，不猜测为手动支持。
- **待确认状态**：检测出错、仅被动观察/无法自动验证、缺失安装状态或安装能力字段、状态矛盾或已提供的详细证据格式不完整、多实例歧义，以及刷新中或刷新失败后的旧结果。它不等于软件损坏。保持既有旧后端兼容规则：仅缺少 `installationDetails` 不会单独触发此分类，也不会额外阻止原本允许的操作；已提供的详细证据仍需通过现有校验。
- Qwen 等被动 PATH 候选不会进入「已安装」或「当前可自动安装」。安装状态与详细证据矛盾的条目也不进入「已安装」；已测安装但实例优先级有歧义的条目仍可查看，其操作保持禁用。

只读演示不会显示当前可自动安装项。刷新失败时可切回「全部」查看旧结果，成功刷新前写入仍被锁定。空列表中的「查看全部 Agent」清除搜索及两组筛选，但保留当前 Agent 选择。

## English

The capability/status selector combines with All / Installed and search using AND semantics. Filtering is local to the current list: it runs no agent, installer, or extra backend request, and grants no installation, configuration, or uninstall permission.

- **Currently available for automatic installation** requires native desktop mode, a successful fresh observation, no operation in progress, explicit automatic-install support, NotDetected with installed=false, and consistent/unambiguous evidence. Clicking and confirming are still required. Absence from PATH does not establish absence from the device.
- **Manual installation only** means the catalog explicitly does not support automatic installation. It does not mean uninstalled or independently runtime-verified. Missing legacy capability fields remain unknown.
- **Needs confirmation** includes probe errors, passive/unsupported verification, missing installation-status or installation-capability fields, inconsistent status or malformed supplied details, instance ambiguity, and stale or refreshing observations. It does not mean the software is broken. Existing legacy DTO compatibility is retained: absent `installationDetails` alone does not trigger this category or add a new operation block; supplied details still undergo the existing validation.
- Passive Qwen candidates are not Installed or automatically installable. Contradictory installation details are excluded from Installed. Detected installations with priority ambiguity remain inspectable with mutations blocked.

Read-only previews do not advertise currently actionable installation. After refresh failure, All still exposes the previous observations without enabling writes. The empty-state reset clears search and both selectors while preserving the selected agent.

Validation uses synthetic UI/DTO fixtures. It does not establish real-agent installation, native desktop rendering, or manual GUI acceptance.
