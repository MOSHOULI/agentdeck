# SQLite dependency security / SQLite 依赖安全

AgentDeck uses rusqlite's `bundled` feature for its memory database and read-only
session collection. RUSTSEC-2026-0459 / CVE-2026-119375 affects bundled SQLite
before 3.54.0: opening a crafted database with a sibling WAL having a mismatched
page size can expose uninitialized page-cache memory. Read-only opening is not
proof that the affected parser is safe.

AgentDeck 的记忆数据库和只读会话采集使用 bundled SQLite。恶意数据库及页大小不匹配
的同名 WAL 可能导致旧版本泄露未初始化内存；只读打开不能代替依赖修复。

The dependency is upgraded from rusqlite 0.34 to 0.41 (libsqlite3-sys >=0.39,
SQLite >=3.54.0). Keep Cargo.lock committed and use `--locked`. Upstream requires
Rust >=1.89; CI uses current stable. No advisory is ignored and no schema or
production permission changes are introduced.

升级 rusqlite 0.34 至 0.41，锁定修复后的 libsqlite3-sys。保留审计门禁，不增加忽略项，
不改变数据库结构或生产权限。上游最低 Rust 版本为 1.89。

Regression tests assert the actual linked SQLite version, exercise a synthetic
MemoryStore WAL with a concurrent read-only reader, verify writes are rejected
by that reader, check FTS queries, and reopen the database. They do not claim to
reproduce the vulnerability or validate every third-party session database.

回归验证实际链接版本、合成数据库 WAL、只读写入拒绝、FTS 和重开兼容性；不声称重现
漏洞或验证了全部第三方会话数据库。

Sources / 来源:
- https://rustsec.org/advisories/RUSTSEC-2026-0459.html
- https://github.com/RustSec/advisory-db/blob/main/crates/libsqlite3-sys/RUSTSEC-2026-0459.md
- https://github.com/rusqlite/rusqlite/releases/tag/v0.41.0
