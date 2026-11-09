# DSH dependency evidence / DSH 依赖证据

```sh
aisync dsh dependency-evidence
```

## Scope and interpretation

This command emits schema version **1**, a fixed **2026-11-09** review snapshot
for `@deepseek-ai/dsh@0.2.2-rc.1`. It takes no path, version, URL, checksum,
platform, install or other override. It hashes and parses only embedded fixtures;
it does not read local manifests, discover a runtime, contact npm, extract an
archive, execute package scripts or write files. Exit 0 means the report was
produced, not that DSH is installable. Invalid arguments exit 2; embedded evidence
validation and output errors fail nonzero without a successful complete report.
Output goes to stdout; a caller may redirect it to a file.

- `root.recorded_archive_state: root_archive_verified_at_review` records the
  previous root-archive review. `archive_verified_during_this_command: false`
  prevents confusing it with a current local-file check. Use the separate
  [root `verify-package` command](DSH-DISTRIBUTION.md) to check a selected tgz.
- `root.declarations.dependencies` preserves **all 90 original direct edges**,
  keyed by dependency name, under the identified root package/version. **82**
  declarations are the exact root version and **8** are unresolved ranges,
  listed unchanged in `direct_unresolved_ranges`. No version is chosen for a
  range, and an exact declaration is not a resolved or verified dependency.
- `illustrative_exact_metadata` contains exactly three manually selected,
  independently pinned registry responses, **not an exhaustive inventory or a
  dependency closure**. The samples do not establish reachability from the root.
  `dsh-subprocess-local` is not a direct root dependency. Its own metadata does
  declare exact edges to the sampled `node-pty` and `koffi` versions.
- Each sample says `registry_metadata_observed`,
  `dependency_archive_verified: false` and
  `metadata_archive_agreement: unchecked`. Its `dist` hashes and tarball URL are
  registry declarations, not evidence that the dependency archive was fetched,
  authenticated or compared with its manifest. Other dependency metadata remains
  unobserved by this report.
- Original `dependencies`, `optionalDependencies`, `peerDependencies` and
  `peerDependenciesMeta` are retained, alongside `scripts`, `engines`, `os`,
  `cpu`, `libc`, `gypfile` and the two bundled-dependency field spellings.
  `null` means undeclared in these snapshots, not absent at runtime. Development
  dependencies are not inventoried. Peer contexts and optional/platform choices
  are unresolved; no platform is inferred from package names or missing fields.
- `declared_lifecycle_scripts` highlights the preinstall/install/postinstall,
  prepublish and preprepare/prepare/postprepare declarations when present. All
  original scripts remain in `declarations.scripts`, including other lifecycle
  events. Highlighting does not predict which hooks a package manager will run
  or assess their behavior. `full_implicit_lifecycle_assessed: false` applies to
  every package, including the root: native defaults, unobserved archive content
  and dependency hooks remain unknown.

The selected observations include `dsh-subprocess-local`'s `postinstall`,
`node-pty`'s `install` and `postinstall`, and `koffi`'s `install` plus **15 optional
platform-package declarations**. They are neither blanket “safe” nor “unsafe”
verdicts, and none were executed. Every report keeps `closure_complete`,
`install_ready`, `dependencies_resolved`, `runtime_verified` and
`signature_verified` **false**. Platform compatibility remains `unverified` and
peer resolution remains `unresolved`.

## Provenance and freshness

The [fixture directory](upstream/dsh/dependency-evidence) contains unmodified
bytes, not regenerated JSON. Per-fixture source URLs, byte lengths and SHA256s
are embedded in the core and emitted in the report. The three version-specific
registry responses were retrieved over the official registry on 2026-11-09:

| Fixture | Exact source | Bytes | SHA256 |
| --- | --- | ---: | --- |
| `dsh-subprocess-local-0.2.2-rc.1.json` | [registry](https://registry.npmjs.org/@deepseek-ai%2fdsh-subprocess-local/0.2.2-rc.1) | 2939 | `4f7bf72dcfdc4769c0c63648d5c1c7276089e39d016a9af305455c816dae7de8` |
| `node-pty-1.2.0-beta.15.json` | [registry](https://registry.npmjs.org/node-pty/1.2.0-beta.15) | 2921 | `371717f3c9568b63681e11555eeb100e65dc51f02ee6e30e13f8444773bf13aa` |
| `koffi-3.1.1.json` | [registry](https://registry.npmjs.org/koffi/3.1.1) | 2623 | `10daa36c7aba172d929aa632773419a72b575dfaed8dc680cbe757e1a5e843c7` |
| `root-package-0.2.2-rc.1.json` | `package/package.json` in the [fixed root tgz](https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.2.2-rc.1.tgz) | 8329 | `3e14837ceb3bbc0ba419ed08e9b5ba164338369b1268b93c3c25d89f7b1c9953` |

Root manifest bytes were read statically during review, not by running a package
or by the product command. Their digest and 90 dependency declarations agree
with the existing [root review](upstream/dsh/root-0.2.2-rc.1.json), whose own
fixture SHA256 is
`e23a5772276c9a7709ffb2daeaa5b2d6099a431a4bb9f48cf8c87c5a2517539e`.
That root tgz contained no `npm-shrinkwrap.json`. The
[source workspace lockfile at 66491b05e3f05ad35bcdd4f6cd7ed26041d9e3db](https://github.com/deepseek-ai/deepseek-harness/blob/66491b05e3f05ad35bcdd4f6cd7ed26041d9e3db/pnpm-lock.yaml)
contains workspace links, overrides and patches, not a verified lock of the
published npm consumer tree. Workspace build permissions likewise do not
establish a lifecycle policy for consumer installation. No source lock is
silently converted into a resolved npm graph.

Hashes detect changes to reviewed bytes. They do **not** establish independent
authenticity: npm metadata and archive hashes share a source, and any registry
signature fields in the raw fixtures remain **unverified**. A pinned version
does not guarantee its registry response will never change. This command does
not refresh, check current dist-tags or choose newer versions. A refresh needs
an explicit new review of exact sources, bytes, identities, declared edges and
hash constants, with the date, tests and documentation updated together.

Git attributes mark all five hash-pinned JSON fixtures as `-text`, including the
existing root review. Windows `core.autocrlf` must not rewrite their line endings:
the embedded digests validate the original evidence bytes, not normalized JSON.
No alternate platform digest or runtime newline normalization is accepted.

## Verification

Core regressions verify fixture hashes, identities, parsed field shapes, root
declaration agreement, exact/range counts, original scripts/peer/optional edges,
deterministic JSON and permanently false readiness claims. CLI regressions run
with a private application home and empty PATH, ignore a deliberately invalid
local manifest, check unchanged fixture bytes/mtime and reject all overrides.
This is evidence-report verification, not package execution, install testing or
platform acceptance. Existing root verification, passive inspection and disabled
DSH mutation capabilities remain unchanged.

An offline Git regression checks out all five fixtures with `core.autocrlf=true`
and `core.eol=crlf`, verifies their original bytes and hashes, and proves conversion
is active using an unprotected control file. A negative case removes the attributes
and reproduces digest changes in both multiline root fixtures. Linux and Windows
CI run this regression before compilation.

```sh
python -B scripts/test_dsh_evidence_checkout.py
python scripts/run-isolated.py cargo test --locked -p aisync-core dependency_evidence
python scripts/run-isolated.py cargo test --locked -p aisync-cli --test dsh_dependency_evidence
```

## 中文

此命令只输出 **2026-11-09** 核查的固定证据，schema 为 **1**。没有路径、版本、
来源或平台覆盖参数；只校验并解析编译时嵌入的材料，不访问网络或本地配置，
不解包、不启动 Node/npm/包脚本，也不安装或写入文件。退出码 0 仅代表报告生成。

根归档的成功核验是**历史记录**，不是本次验证了某个本地文件。90 条原始直接依赖
声明完整保留：82 条固定版本声明，8 条未解析范围。固定版本声明也不代表对应包
已经解析、下载或校验。三个示例是手动选定的准确版本元数据，**不是完整传递闭包**，
也没有建立它们从根包可达的证据；subprocess-local 不属于根包的直接依赖。

示例保留原始依赖、可选依赖、peer 及其元信息、脚本、引擎和平台字段。subprocess-local
声明 postinstall，node-pty 声明 install/postinstall，koffi 声明 install 和 15 条可选平台
包依赖。脚本未执行，字段未声明不等于没有运行时行为或隐式构建；没有作统一安全/不安全
判断。所有示例依赖归档均未核验，元数据与包内容是否一致未检查，peer 和平台选择未解析。

闭包完整、可安装、依赖已解析、运行已验证、签名已验证均为 **false**。同源摘要只能
核对材料变化，不能代替独立签名认证。上游源码 workspace 锁文件和构建许可不能直接
当作 npm 发布包的安装锁或生命周期许可。快照不会自动刷新、追随 latest 或选择新版本；
更新必须重新核查准确来源与原始字节，并同步更新摘要、日期、测试和文档。

五份固定摘要 JSON（含既有根包核查记录）均使用 Git `-text` 属性保留原始字节，
不接受 Windows 自动 CRLF 转换后的替代摘要，也不在运行时归一化。离线回归以真实
Git checkout 验证 autocrlf 开启时摘要不变，并用无保护文件和移除属性的反例确认测试有效。
