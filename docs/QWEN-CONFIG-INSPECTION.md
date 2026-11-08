# Qwen settings inspection / Qwen 设置检查

## English

`aisync qwen inspect --settings /absolute/path/settings.json --runtime-manifest /absolute/path/package.json`

This command is a read-only foundation for a future versioned adapter, not automatic configuration or restoration. Select both files yourself. It does not discover HOME/project settings, read `.env` or separate authentication files, execute Qwen/Node/npm, initialize profiles, create backups/locks, or write configuration. It does read the selected settings bytes, which may contain secrets; the JSON report excludes their values, arbitrary keys, paths and raw parser/OS errors. CLI argument errors may echo the input path; do not put secrets in command-line paths.

The only reviewed declaration is `@qwen-code/qwen-code` version **0.27.0**, settings **$version 4**. Missing or different declarations do not enable settings inspection. A matching manifest is merely a file declaration, not proof of package authenticity, installation, runtime selection, or association with the selected settings. Unknown versions must be reviewed before extending the contract.

The report shows a fixed subset of top-level section presence only. It does not validate all field types or nested schemas. Unknown sections are not printed or treated as invalid. Strict JSON is supported in this slice; Qwen itself also accepts JSON comments and BOM. An unsupported parsing result therefore does not mean Qwen rejects that file. No migration, environment-variable expansion, imports or effective-layer merging occurs.

Exit 0 means a report was serialized, even when evidence is missing or unsupported. Invalid path syntax is rejected by argument parsing. Output failure is an error. Only audited Windows and Linux reader platforms are supported; other targets fail closed. Symlinks/reparse points, special files and oversized input are rejected. Handle and component checks narrow, but do not eliminate, same-user concurrent replacement races. Reads may update access times; this is not an OS sandbox.

Settings can be layered across defaults, system defaults, user, project, system overrides, environment and CLI parameters. This one-file summary cannot tell which configuration is active, whether authentication works, whether a change is safe, or whether restoration is possible. Existing mutation capabilities remain disabled. The next step requires field ownership, original-value capture, drift checks and explicit apply/restore approval rather than copying another manager's whole file.

## 中文

此命令是后续版本化适配器的只读基础，不是自动配置或恢复功能。必须明确选择 settings.json 和 package.json，不自动寻找用户或项目目录，不读取 .env 或独立认证文件，不执行 Qwen/Node/npm，不初始化配置、创建备份或锁，也不写文件。选定设置的字节可能包含密钥，因此 JSON 报告不返回配置值、任意键名、路径或原始解析/系统错误。CLI 参数错误可能回显输入路径，请勿把密钥放进命令行路径。

本批仅研究包声明 **@qwen-code/qwen-code 0.27.0** 与设置 **$version 4**。没有匹配的包声明时不读取设置内容；未来版本不能自动视为兼容。声明匹配不证明软件来源可信、已安装、正在运行或与所选设置存在关联。

报告只显示固定的顶层分区是否存在，不是完整字段类型、嵌套 schema 或生效配置校验。本批只支持严格 JSON；Qwen 官方还支持注释和 BOM，所以“本检查器无法解析”不等于官方配置无效。不会迁移旧格式、展开环境变量、导入其他文件或合并设置层级。

退出码 0 只表示生成了报告，缺失或不支持也可产生报告。链接、重解析点、特殊文件及过大输入会被拒绝；未审查平台关闭读取。文件检查不能完全消除同权限进程的并发替换竞态，读取也可能更新访问时间。

此摘要不验证登录、实际安装、恢复资格或应用安全性，现有写入能力保持关闭。后续恢复能力需要记录字段归属、原值、外部漂移，并明确确认后才能应用，不能整文件覆盖其他管理器或用户的设置。

## Audited official sources / 已核查官方来源

All sources pinned to Qwen commit `6c3d5355de2be05b8d1ab61f4699f52c02c42068`, reviewed 2026-11-08. Source version is not a verified npm release.

- [Package declaration](https://github.com/QwenLM/qwen-code/blob/6c3d5355de2be05b8d1ab61f4699f52c02c42068/packages/cli/package.json)
- [Settings loading and migrations](https://github.com/QwenLM/qwen-code/blob/6c3d5355de2be05b8d1ab61f4699f52c02c42068/packages/cli/src/config/settings.ts)
- [Settings schema](https://github.com/QwenLM/qwen-code/blob/6c3d5355de2be05b8d1ab61f4699f52c02c42068/packages/cli/src/config/settingsSchema.ts)
- [Settings layers](https://github.com/QwenLM/qwen-code/blob/6c3d5355de2be05b8d1ab61f4699f52c02c42068/docs/users/configuration/settings.md)
