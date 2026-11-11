# Independent npm v3 lock-location audit

`scripts/dsh_lock_graph.py` is a developer-only prerequisite for a future DSH
consumer-lock review. It adds independent **location and declaration checks** to
the [explicit-target metadata planner](DSH-PLATFORM-PLAN.md). It does not call npm,
read files, request metadata, install anything, or change product capabilities.

```sh
python -B scripts/test_dsh_platform_plan.py
python -B scripts/test_dsh_lock_graph.py
```

## Call contract

`audit(lock, metadata_graph, npm_version=None)` accepts already parsed objects:

- `lock` must be a v3 npm lock with a `packages` root at `""`. Only canonical
  `node_modules/name` and nested/scoped equivalents are supported. Symlink,
  workspace and alias layouts require another review and are rejected. Every
  nested location must have its containing package locations in the lock.
- `metadata_graph` uses the existing planner's explicit root, target os/cpu/libc
  and original exact-version metadata list. Source root metadata is mandatory,
  and its identity must agree with the lock root. Missing dependency metadata is
  emitted as a finding; it is not silently filled from a lock entry.
- `npm_version` is the producer version recorded by the caller, not the version
  installed on the current machine. If absent, `producer_npm_version_missing`
  remains a finding. Supplied text must be an exact version, but the auditor
  cannot attest its truth and keeps `producer_version_verified: false`.

Callers must bound parsing and preserve original bytes, hashes, source URLs,
retrieval times, producer environment and the complete candidate **before** npm
cleanup. This function cannot recover omitted nodes or prove provenance. Neither
an input field nor a syntactically valid report authenticates that evidence.

Bounds cover 2,048 supplied lock locations, 16,384 lock declarations, separately
16,384 normalized lock peer hints, and 16,384 authoritative output declarations.
The existing metadata-graph bounds also apply. Counts are checked before
traversing oversized maps. Repeated copies of an identity at different locations
count independently toward the output limit. Malformed supported fields reject
the whole call without returning partial approval.

## Independent checks

The auditor keeps **locations**, not just name/version identities. Two copies of
the same package may have different parents, resolved dependency locations and
mandatory/optional paths. Ordinary edges search inside the declaring package,
then its ancestor package locations. The nearest installed candidate is retained
and compared; an incompatible nearer version cannot be skipped in favor of a
compatible hoisted one. Scoped names remain atomic package names throughout this
lookup. Peer candidates start beside the declaring package, not in its own
`node_modules`, and peer contexts remain unverified even when an exact candidate
exists in that search scope.

When source metadata exists, its original dependencies, optionalDependencies and
peerDependencies determine the edges. The corresponding lock declarations are
compared separately. A required source edge deleted from the lock is still
examined and reported. Extra lock-only declarations remain visible in each
node's `lock_declarations`, even when source metadata does not declare that edge.
If source metadata is missing, declarations from the lock are explicitly marked
`declaration_basis: lock_only`, with a `source_metadata_missing` finding.

Only literal exact version strings are compared for equality. A selected
candidate does not establish semver range, tag, alias, workspace, URL or file-spec
satisfaction; these are `spec_unverified`. Missing candidates remain
`target_missing`. Peer findings are context observations, not assertions that
optional peers must be installed. `optional_peer` preserves the source hint, or
the explicitly labelled lock-only hint when source metadata is unavailable.

Peer optional metadata is normalized and compared separately. Hints with no
corresponding peer declaration are legal observations, reported as
`peer_hint_without_declaration`; they never create a dependency edge. The
checked-in UI lock's debug/supports-color entry exercises this real format case.
Other peer metadata fields are outside this partial gate.

Candidate reachability and platform-filtered path modes are derived from edges,
never from `dev`, `optional`, `devOptional` or `peer` lock flags. Those flags remain
visible as observations. Same-name optional declarations override ordinary ones,
without erasing the original edge. A shared mandatory path remains mandatory even
if another optional path is excluded. Required failures inside an optional
subtree are still findings; backward npm optional-subtree cleanup is not simulated.
Cycles terminate using location/path-kind work sets.

Platform comparisons use **source metadata only**, including libc. Lock os/cpu/libc
fields cannot substitute for the original declaration or justify dropping a node.
Missing source metadata or unknown target libc remains unverified. In particular,
a v3 lock that omits libc cannot convert a source musl constraint into an
unrestricted or host-glibc package. Nodes and original lock declarations remain
in the output even when their candidate paths are excluded or unreachable.

## What a successful call does not establish

Findings are observations, not a single safe/unsafe or accepted/rejected verdict.
An empty findings list still leaves `lockfile_validated`, `closure_complete`,
`dependencies_resolved`, `install_ready`, `runtime_verified`, `archive_verified`,
`signature_verified` and `source_provenance_verified` **false**.

This gate does not validate resolved URLs, integrity hashes, tarball identity or
contents, registry signatures, semver satisfaction, complete peer sets, bundled
content, engines, native ABIs, lifecycle policy or real execution. Bundled flags
are exposed as `bundled_content_unverified`. Development dependency declarations
are not production roots; dev-only locations can appear as unreachable while
remaining available for structural inspection. The full input graph is not pruned.

Platform/path modes describe the supplied **candidate**, including unresolved or
mismatched edges, and are not an installable selection. Findings are also retained
on unreachable or excluded candidates so their evidence gaps cannot disappear.
No stale cache, historical research count or author-provided readiness flag is
accepted as proof of completion.

## Verification and remaining work

Offline regressions cover nested/hoisted and scoped lookup, nearest-version
mismatch, missing nodes, deleted source edges, missing metadata, peer placement,
legal unbound peer hints, optional override, distinct placements of one identity,
shared mandatory precedence, libc absent from locks, cycles, bounds and malformed
locations. They also inspect the repository's real checked-in UI lock: all 260
current locations are retained, while missing source metadata, unknown producer
version and unverified ranges remain explicit. This does **not** validate that
lock's full dependency semantics or establish a real DSH consumer lock.

Before any DSH lock can be accepted, the remaining gates need reviewed original
metadata and provenance, range/peer-context verification, archive/manifest
agreement, lifecycle policy and supported isolated runtime evidence. This change
does not recreate the missing 493-response checkpoint, resume live collection,
perform a DSH install, bypass #22 isolation, or publish an installer.

## 中文摘要

本模块独立核对 npm v3 候选锁中的安装位置和原始声明：正确区分嵌套与提升、同一包
的不同位置、必须与可选路径，并发现锁里被删掉的原始依赖。不能靠 lock.optional
或缺失的 libc 字段把问题隐藏掉。合法但没有 peer 声明的元数据提示会保留，不凭空
生成依赖。真实 UI lock 的结构检查也保持缺失元数据、范围及生产 npm 版本未核实。

这仍是前置证据检查。完整锁、完整闭包、可安装、归档、签名与运行验证全部为 false；
不代表真实 DSH 支持，也不改变安装或隔离边界。
