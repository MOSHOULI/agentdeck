# DSH exact-metadata platform planning

The developer-only module `scripts/dsh_platform_plan.py` supplies a bounded,
read-only **declaration planner** for the next DSH dependency review. It is not
wired into installation, the product CLI or the desktop UI. Run its offline
regressions with:

```sh
python -B scripts/test_dsh_platform_plan.py
```

## Input and result

`plan(document)` takes an already loaded object with exactly these top-level
fields:

```json
{
  "schemaVersion": 1,
  "root": "example@1.0.0",
  "target": {"os": "linux", "cpu": "x64", "libc": "glibc"},
  "packages": [{"name": "example", "version": "1.0.0"}]
}
```

Package objects use npm-style `name`, `version`, `dependencies`,
`optionalDependencies`, `peerDependencies`, `peerDependenciesMeta`, `os`, `cpu`
and `libc` fields. Other metadata fields are not interpreted or certified. The
caller must retain the original metadata bytes and provenance separately. This
module does not load paths, deserialize a lockfile, read a package manager cache,
query registries, inspect the host, write output, extract archives or run scripts.
The caller is responsible for bounded JSON parsing before invoking it.

The subset is deliberately narrow: at most 2,048 unique package identities and
16,384 declarations, bounded strings and platform lists. Identity versions must
be exact semver. Duplicate identities and malformed relevant fields fail without
a partial report, including malformed nodes outside the root's reachable graph.
A package identity is **not** an installed location or peer context.

Every supplied node and original dependency declaration is retained in the
result, even when it is unreachable, shadowed or platform-excluded. Output is
deterministic for reordered input nodes and dependency maps. Platform declarations
are normalized to lists. Missing declarations are reported as `undeclared`, not
as evidence of runtime support. No input object is changed.

## Explicit target rules

- `os` and `cpu` must be explicit concrete tokens. `libc` must be explicit too;
  `null` means unknown. The planner never substitutes its host platform or libc.
- Allow/deny lists use the npm-install-checks comparison rule: an exact singleton
  `any` matches, matching negative entries veto, and at least one positive must
  match when positives exist. String declarations and empty lists are supported.
- If a package declares libc and target libc is unknown, the result stays
  `unknown`, including for `any` and negative-only lists. There is no assumption
  that all Linux targets are glibc, nor that absent libc in an npm v3 lock means
  the package has no libc declaration.
- A known mismatch in any dimension is a mismatch even if another is unknown.
  All dimension results and declarations are retained for inspection.

These are declaration comparisons, not a promise that a package runs on the
named system. Node/npm engines, native ABIs, operating-system versions, lifecycle
requirements, bundled dependencies and archive agreement are outside this step.

## Graph and path rules

Only literal exact-version dependency declarations identify another metadata
node. Ranges, tags, npm aliases, workspace links, file paths and URLs remain
`spec_unresolved`; no version selection or string-to-version guessing occurs.
A missing exact node remains `metadata_missing`, even for an optional dependency,
since absent metadata cannot prove an incompatible platform.

Production-declaration reachability is computed before target exclusions.
`optionalDependencies` override same-name `dependencies`; the overridden original
edge is still emitted with `shadowed_by_optional`. Each node separately records
its target paths as `mandatory`, `optional`, both or neither. A mandatory edge
under an optional ancestor remains an optional *path*. A shared mandatory path
cannot be removed because an optional path encounters a mismatch. Cycles terminate
using a bounded node/path-kind work queue.

Known incompatible optional paths are recorded in `excluded_optional_paths`,
without deleting their nodes or descendant declarations. An incompatible required
child inside an optional subtree also emits `optional_subtree_platform_mismatch`.
The planner does **not** simulate npm's backward removal of the optional ancestor,
reify a package tree or claim that the surviving optional parent is installable.
Mandatory mismatches remain reachable and emit `mandatory_platform_mismatch`.
Unknown relevant constraints emit `platform_unknown` instead of silently pruning.

Peer declarations are retained separately and always marked
`peer_context_unresolved`, including optional peers and exact versions with
matching metadata present. Peers do not become ordinary production paths. The
planner does not perform peer placement or claim an optional peer must be installed.
`unreachable_metadata` means not reached by the supported exact production edges;
it may include peer-only packages or packages whose incoming ranges are unresolved.

Findings cover target-reachable nodes; the complete edge list still exposes
missing/unresolved declarations on excluded or unreachable branches. An empty
findings list is **not a completion or safety verdict**.

## Evidence boundary and next gates

All reports hard-code `closure_complete`, `install_ready`,
`dependencies_resolved`, `runtime_verified`, `archive_verified`,
`signature_verified` and `lockfile_validated` to **false**. `npm_invoked` is false
and `npm_version` is null because this operation does not run npm. The input graph
is not pruned. This step cannot turn metadata observations into a verified lock.

Tests use authored small graphs plus the existing checked-in koffi metadata to
show that its 15 absent optional nodes stay missing. They do not recreate the
previous 493-response research checkpoint, fetch new metadata, validate a real
DSH installation or establish dependency closure. No install capability or
network isolation policy is changed.

A future npm-generated candidate still needs independent lock/node/edge/context
validation, original platform metadata including libc, archive and manifest
agreement, lifecycle policy and supported isolated execution gates. Retain the
full graph **before** any package-manager cleanup, the exact npm version, producer
host, explicit target and original evidence provenance. npm's mandatory and
optional checks can use different environment inputs; a successful npm run alone
must not substitute for an explicit-target audit. A v3 lock omitting libc is not
sufficient input to reconstruct it. Synthetic success does not authorize or imply
that live resolution or installation has resumed.

## 中文摘要

这是离线开发预审模块，不是安装器、npm 解析器或完整锁验证器。它比较显式目标的
os/cpu/libc 声明，保留完整输入节点和原始依赖边，区分必须路径、可选路径及尚未解析
的 peer 上下文。共享节点的必须路径优先，不因另一条可选路径不兼容而整体删除。
缺失元数据、范围、别名及 peer 均不猜测为已解析；libc 未知也不按宿主补齐。

合成测试及已有 koffi 元数据只验证这些边界。完整闭包、安装就绪、运行验证、归档
核验、签名验证与锁文件验证始终为 false。后续真实候选必须保留清理前图和 npm
版本，另行完成锁图、原始平台元数据、归档一致性及隔离运行审查。
