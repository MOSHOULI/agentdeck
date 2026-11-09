# Bounded NSIS build retries / 有界 NSIS 构建重试

## Reviewed upstream contract

This CI-only wrapper invokes the npm-lock-installed Tauri CLI **2.12.4** via its `tauri.js` entry with the same `build --bundles nsis --ci -- --locked` arguments. `npm ci` retains registry integrity verification. The wrapper checks the reviewed CLI version, registry URL, SHA512 integrity and native dependency versions, with the actual Windows x64 native package’s registry URL and SHA512 integrity additionally pinned; it rejects the Tauri GitHub mirror override variables.

Source: https://raw.githubusercontent.com/tauri-apps/tauri/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs

The exact evidence path is marked `-text` in `.gitattributes`; an offline test uses a real Git index/checkout with `core.autocrlf=true` and verifies byte-identical SHA256. The installed main and Windows x64 native package versions are checked before building.

The checked-in evidence `upstream/tauri/nsis-2.12.4.rs` was read on 2026-11-09. Its SHA256 is `ce129133be31063c51ee3649f69f600b9397f6af1e24075dec3b0d54ae637a2c`. It retains upstream copyright/SPDX notices. This snapshot documents the reviewed version, not an assertion that upstream tags can never change.

- DLL: `https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll`
- Upstream expected DLL SHA1: `75197FEE3C6A814FE035788D1C34EAD39349B860`.
- Official NSIS 3.13 zip SHA1: `DB14F8AF2EA346B786C6AE19343F61AAF5E09B8E`.
- Cached NSIS.exe SHA1: `6D45D4E8CCF18FE1FDAC9164058F356102A2ECF2`.

Tauri defaults to the platform cache directory plus `tauri/NSIS` (or its configured local-tools directory plus `.tauri/NSIS`). Missing required files or a mismatched NSIS.exe cause toolset recreation. A complete cache still has its NSIS.exe and DLL hash checked; a wrong DLL is downloaded and verified again. Merely populating a DLL into an empty directory does not work. We do not prefill, trust, bypass or add an Actions cache for this toolset. These are **upstream SHA1 checks**, not new SHA256 authenticity or signature guarantees for the DLL. The source snapshot SHA256 only protects our evidence. A direct official DLL GET during this work returned HTTP 504, so no new DLL hash or execution claim is made.

## Exact retry boundary

Only the build process's final three nonblank lines qualify: the exact pinned `Downloading` URL, `failed to bundle project: \`http status: N\``, and the same error prefixed `Error`. N must be 408, 429, 502, 503 or 504. Unknown formatting, a different URL, hash mismatch, compilation failure, permanent HTTP failure, or an error following an old transient failure fails closed. Success never triggers another build.

At most three whole build attempts occur, with 10 then 20 seconds between attempts. The entire workflow step has a 15-minute timeout; it is not 15 minutes per attempt. Output streams to the CI log while only the last 64 KiB is retained for classification. There is no new network reader, extraction routine or download size guarantee: upstream Tauri still performs all transfers and validation. A stalled transfer is bounded by the workflow step timeout, not an invented per-transfer watchdog. Exhaustion fails the existing gate; installer lifecycle and release gates are unchanged.

## 中文说明

本改动仅改善 CI 网络瞬断的容错，不修改产品、不发布安装包、不更换下载镜像。保留锁定 Tauri CLI 2.12.4 的官方 NSIS 下载与 SHA1 校验；上述 SHA256 是源码证据摘要，不是 DLL 的新增真实性保证。默认缓存为平台缓存目录下的 `tauri/NSIS`，完整缓存仍由上游检查 NSIS.exe 和 DLL 摘要，缺文件或摘要错误不会绕过检查。没有新增或盲目信任跨运行缓存。

仅构建进程最后三行精确对应固定官方 DLL 下载及 HTTP 408/429/502/503/504 才重试，最多三次，间隔 10/20 秒，整个构建步骤最多 15 分钟。摘要失败、非固定来源、旧错误后出现新错误或未知格式立即失败。日志流式输出，分类器只保留最后 64 KiB。没有新写下载器或承诺单独下载大小上限；网络读取仍由上游负责，总超时由 CI 步骤约束。离线测试覆盖成功、瞬断恢复、耗尽、永久错误、摘要错误、非固定 URL、混合旧日志及版本/来源契约。实际 Windows 重试仍须 CI 验证。

Windows x64 原生 CLI 的 registry URL 与 SHA512 integrity 同样固定检查，安装后的主 CLI/原生包版本也核对。源码证据使用精确路径 `-text` 属性，通过真实 Git `core.autocrlf=true` 索引/检出测试确认字节和 SHA256 不变。
