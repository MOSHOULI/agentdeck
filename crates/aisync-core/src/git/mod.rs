//! Git 备份回滚模块（M4）。
//!
//! 事件驱动：每次安装/卸载/vault 变动前自动把应用管理的配置状态
//! （vault 密文、M2 注入备份）备份到 `~/.agentdeck/git-backup/`。
//! 只调用系统 git CLI；提交前做密钥特征扫描，疑似明文密钥一律中止。
//!
//! 真实备份文件（M2 注入备份）可能含明文 API Key，因此入库前用 vault
//! 主密钥 AES-256-GCM 加密为 `.gcm` 镜像（AAD=`b"backup"`），回滚时解密还原；
//! vault.json 本身已是密文，直接镜像。
//!
//! 回滚两种模式：
//! - 按操作回滚：还原某次备份 commit 变更的文件（默认）
//! - 按时间点整体恢复：还原到该 commit 的完整状态（`full=true`，UI 需确认）

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use zeroize::Zeroizing;

use serde::Serialize;

use crate::Error;

/// 与 V2 `gitutil._SECRET_VALUE_RE` 一致的密钥特征
/// 密钥特征扫描（V2 基线 + 主流 AI 厂商前缀）
const SECRET_RE: &str = r"(sk-[A-Za-z0-9]{8,}|sk-ant-[A-Za-z0-9]{8,}|sk-or-[A-Za-z0-9]{8,}|eyJ[A-Za-z0-9_-]{10,}|ghp_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|AIza[A-Za-z0-9_-]{30,}|hf_[A-Za-z0-9]{20,}|glpat-[A-Za-z0-9_-]{15,})";
const GITIGNORE: &str = "*.key\n.env\n*.log\nmemory.db\n";
const REPO_DIR: &str = ".agentdeck/git-backup";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub hash: String,
    /// 应用时间戳（纳秒字符串，跨 IPC 无损、每 commit 唯一）
    pub timestamp: String,
    pub message: String,
}

fn home_dir() -> crate::Result<PathBuf> {
    crate::home_dir().ok_or_else(|| Error::Other("无法定位用户主目录".into()))
}

pub fn default_repo_dir() -> crate::Result<PathBuf> {
    Ok(home_dir()?.join(REPO_DIR))
}

fn live_vault_dir() -> crate::Result<PathBuf> {
    Ok(home_dir()?.join(".agentdeck").join("vault"))
}

fn live_backups_dir() -> crate::Result<PathBuf> {
    Ok(home_dir()?.join(".agentdeck").join("backups"))
}

fn unix_now_nanos() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

fn run_git(repo: &Path, args: &[&str]) -> crate::Result<String> {
    Ok(String::from_utf8_lossy(&run_git_bytes(repo, args)?).to_string())
}

fn run_git_bytes(repo: &Path, args: &[&str]) -> crate::Result<Vec<u8>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .map_err(|e| Error::GitFailed(format!("git 执行失败（未安装？）: {}", e)))?;
    if !out.status.success() {
        return Err(Error::GitFailed(format!(
            "git {} 失败: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out.stdout)
}

fn ensure_repo(repo: &Path) -> crate::Result<()> {
    if !repo.join(".git").exists() {
        std::fs::create_dir_all(repo)?;
        run_git(repo, &["init", "-b", "main"])?;
        run_git(repo, &["config", "user.name", "aisync"])?;
        run_git(repo, &["config", "user.email", "aisync@local"])?;
    }
    let gi = repo.join(".gitignore");
    if !gi.exists() {
        std::fs::write(&gi, GITIGNORE)?;
    }
    Ok(())
}

/// 清空镜像目录（不存在视为成功）
fn clear_dir(path: &Path) -> crate::Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::GitFailed(format!(
            "清理仓库镜像目录失败 ({}): {}",
            path.display(),
            e
        ))),
    }
}

/// 把 src 目录内容镜像复制到 dst（返回复制文件数）
fn copy_tree(src: &Path, dst: &Path) -> crate::Result<usize> {
    if !src.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in walkdir::WalkDir::new(src).follow_links(false).into_iter() {
        let entry = entry.map_err(|e| Error::GitFailed(format!("备份目录读取失败: {e}")))?;
        let path = entry.path();
        if entry.file_type().is_symlink() {
            return Err(Error::GitFailed(format!(
                "备份不允许符号链接: {}",
                path.display()
            )));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        // master.key 等密钥文件绝不进仓库（即使被 .gitignore 兜底）
        if protected_path(path) {
            continue;
        }
        let rel = path
            .strip_prefix(src)
            .map_err(|_| Error::GitFailed("备份路径解析失败".into()))?;
        let target = dst.join(rel);
        check_no_symlinks(&target)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, &target)?;
        count += 1;
    }
    Ok(count)
}

/// 把 live backups 目录加密镜像到 repo（每个文件 → `*.gcm` = nonce(12)||密文）
fn encrypt_backups_tree(src: &Path, dst: &Path, master: &[u8; 32]) -> crate::Result<usize> {
    if !src.exists() {
        clear_dir(dst)?;
        return Ok(0);
    }
    let mut count = 0usize;
    let mut retained = HashSet::new();
    for entry in walkdir::WalkDir::new(src).follow_links(false).into_iter() {
        let entry = entry.map_err(|e| Error::GitFailed(format!("备份目录读取失败: {e}")))?;
        let path = entry.path();
        if entry.file_type().is_symlink() {
            return Err(Error::GitFailed(format!(
                "备份不允许符号链接: {}",
                path.display()
            )));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(src)
            .map_err(|_| Error::GitFailed("备份路径解析失败".into()))?;
        let mut file_name = rel
            .file_name()
            .ok_or_else(|| Error::GitFailed("备份文件名非法".into()))?
            .to_os_string();
        file_name.push(".gcm");
        let target = dst
            .join(rel.parent().unwrap_or_else(|| Path::new("")))
            .join(file_name);
        check_no_symlinks(&target)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = Zeroizing::new(std::fs::read(path)?);
        retained.insert(target.clone());
        // Reuse authenticated ciphertext, never a public hash-derived nonce.
        // Legacy deterministic ciphertext is migrated on the next backup.
        if let Ok(existing) = std::fs::read(&target) {
            use sha2::{Digest, Sha256};
            let legacy_nonce = Sha256::digest(&*raw);
            if existing.len() >= 28
                && existing[..12] != legacy_nonce[..12]
                && let Ok(plain) = decrypt_backup_file(master, &existing)
                && Zeroizing::new(plain).as_slice() == raw.as_slice()
            {
                count += 1;
                continue;
            }
        }
        let (nonce, ct) = encrypt_bytes(master, &raw)?;
        let mut data = Vec::with_capacity(nonce.len() + ct.len());
        data.extend_from_slice(&nonce);
        data.extend_from_slice(&ct);
        crate::vault::keystore::atomic_write_bytes(&target, &data)?;
        count += 1;
    }
    if dst.exists() {
        for entry in walkdir::WalkDir::new(dst).follow_links(false) {
            let entry = entry.map_err(|e| Error::GitFailed(e.to_string()))?;
            if entry.file_type().is_file() && !retained.contains(entry.path()) {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(count)
}

fn encrypt_bytes(master: &[u8; 32], raw: &[u8]) -> crate::Result<([u8; 12], Vec<u8>)> {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master));
    let mut nonce = [0u8; 12];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let ct = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: raw,
                aad: b"backup",
            },
        )
        .map_err(|_| Error::GitFailed("备份加密失败".into()))?;
    Ok((nonce, ct))
}

fn decrypt_backup_file(master: &[u8; 32], data: &[u8]) -> crate::Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    use aes_gcm::{Aes256Gcm, Key, Nonce};

    if data.len() <= 12 {
        return Err(Error::GitFailed("备份密文长度异常".into()));
    }
    let (nonce, ct) = data.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master));
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ct,
                aad: b"backup",
            },
        )
        .map_err(|_| Error::GitFailed("备份解密失败（主密钥不匹配或数据损坏）".into()))
}

/// 备份当前状态到 git 仓库；无变更返回 0，否则返回本次提交的文件数
pub fn backup(message: &str) -> crate::Result<usize> {
    backup_at(
        &default_repo_dir()?,
        &live_vault_dir()?,
        &live_backups_dir()?,
        message,
    )
}

fn backup_at(
    repo: &Path,
    live_vault: &Path,
    live_backups: &Path,
    message: &str,
) -> crate::Result<usize> {
    crate::vault::keystore::with_file_lock(live_vault, || {
        refuse_key_rotation(repo, live_vault)?;
        let master = Zeroizing::new(crate::vault::keystore::load_or_create_master_unlocked(
            live_vault,
        )?);
        backup_with_master(repo, live_vault, live_backups, message, &master)
    })
}

fn backup_with_master(
    repo: &Path,
    live_vault: &Path,
    live_backups: &Path,
    message: &str,
    master: &[u8; 32],
) -> crate::Result<usize> {
    check_no_symlinks(repo)?;
    check_no_symlinks(live_vault)?;
    check_no_symlinks(live_backups)?;
    ensure_repo(repo)?;
    // 重建镜像（git 历史兜底，删除可恢复）；防止上次残留的明文镜像被再次暂存
    clear_dir(&repo.join("vault"))?;
    copy_tree(live_vault, &repo.join("vault"))?;
    encrypt_backups_tree(live_backups, &repo.join("backups"), master)?;
    // 仓库只存放应用管理的文件（vault 密文 + 注入备份），.gitignore 已排除密钥类文件
    run_git(repo, &["add", "-A"])?;

    if run_git(repo, &["diff", "--cached", "--quiet"]).is_ok() {
        return Ok(0);
    }

    // 密钥扫描：疑似明文密钥一律中止提交
    let staged = run_git(repo, &["diff", "--cached"])?;
    let re = regex::Regex::new(SECRET_RE).expect("密钥正则应为合法正则");
    if re.is_match(&staged) {
        return Err(Error::GitFailed(
            "检测到疑似明文密钥内容，已中止备份提交；请检查 backups/ 与 vault 目录后重试".into(),
        ));
    }

    let staged_names = run_git(repo, &["diff", "--cached", "--name-only"])?;
    let count = staged_names
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    let msg = format!("{} {}", unix_now_nanos(), message);
    run_git(repo, &["commit", "-m", &msg])?;
    Ok(count.max(1))
}

/// 列出最近 100 条备份
pub fn list_backups() -> crate::Result<Vec<BackupInfo>> {
    list_backups_at(&default_repo_dir()?)
}

fn list_backups_at(repo: &Path) -> crate::Result<Vec<BackupInfo>> {
    if !repo.join(".git").exists() {
        return Ok(Vec::new());
    }
    if run_git(repo, &["rev-parse", "--verify", "HEAD"]).is_err() {
        return Ok(Vec::new());
    }
    let log = run_git(repo, &["log", "--format=%H|%ct|%s", "--max-count=100"])?;
    let mut out = Vec::new();
    for line in log.lines() {
        let mut parts = line.splitn(3, '|');
        let (Some(hash), Some(ts), Some(message)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let timestamp = message.split_whitespace().next().unwrap_or(ts).to_string();
        out.push(BackupInfo {
            hash: hash.to_string(),
            timestamp,
            message: message.to_string(),
        });
    }
    Ok(out)
}

/// 按时间戳回滚：`full=false` 只还原该次备份变更的文件，`full=true` 还原到该时间点完整状态
pub fn restore(timestamp: &str, full: bool) -> crate::Result<Vec<String>> {
    let repo = default_repo_dir()?;
    let vault = live_vault_dir()?;
    let backups = live_backups_dir()?;
    // One lock spans the safety snapshot and restore; otherwise a concurrent
    // vault save could be overwritten without ever reaching the safety snapshot.
    crate::vault::keystore::with_file_lock(&vault, || {
        find_commit(&repo, timestamp)?;
        let master = Zeroizing::new(crate::vault::keystore::load_master(&vault)?);
        backup_with_master(&repo, &vault, &backups, "restore: 回滚前快照", &master)?;
        restore_with_master(&repo, &vault, &backups, timestamp, full, &master)
    })
}

fn refuse_key_rotation(repo: &Path, vault: &Path) -> crate::Result<()> {
    if !vault.join("master.key").exists()
        && (repo.join("backups").exists()
            || (repo.join(".git").exists()
                && run_git(repo, &["rev-parse", "--verify", "HEAD"]).is_ok()))
    {
        return Err(Error::GitFailed(
            "备份历史存在但主密钥丢失；拒绝生成替代密钥，请恢复原 master.key".into(),
        ));
    }
    Ok(())
}

fn find_commit(repo: &Path, timestamp: &str) -> crate::Result<String> {
    // The pre-restore snapshot may push the requested item beyond the UI's 100-item window.
    let log = run_git(repo, &["log", "--format=%H %s"])?;
    for line in log.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(hash), Some(ts)) = (fields.next(), fields.next())
            && ts == timestamp
        {
            return Ok(hash.to_owned());
        }
    }
    Err(Error::GitFailed(format!(
        "未找到时间戳 {} 的备份（可用 list_backups 查看）",
        timestamp
    )))
}

// Files deliberately excluded from backup must never be restored or removed.
fn protected_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(extension.as_str(), "key" | "log")
        || matches!(name.as_str(), ".env" | "memory.db" | ".vault.lock")
        || name.starts_with(".tmp-")
}

/// Parse Git's portable slash-separated paths, rejecting Windows special names too.
fn map_path(repo_rel: &str, live_vault: &Path, live_backups: &Path) -> crate::Result<PathBuf> {
    let (root, rel, encrypted) = if let Some(rel) = repo_rel.strip_prefix("vault/") {
        (live_vault, rel, false)
    } else if let Some(rel) = repo_rel.strip_prefix("backups/") {
        (live_backups, rel, true)
    } else {
        return Err(Error::GitFailed(format!("未知仓库路径: {repo_rel}")));
    };
    let rel = if encrypted {
        rel.strip_suffix(".gcm")
            .ok_or_else(|| Error::GitFailed("备份文件缺少 .gcm 后缀".into()))?
    } else {
        rel
    };
    let mut path = root.to_path_buf();
    for part in rel.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err(Error::GitFailed(format!("非法备份路径: {repo_rel}")));
        }
        path.push(part);
    }
    if !encrypted && protected_path(&path) {
        return Err(Error::GitFailed(format!("备份包含受保护文件: {repo_rel}")));
    }
    check_no_symlinks(&path)?;
    Ok(path)
}

fn check_no_symlinks(path: &Path) -> crate::Result<()> {
    for ancestor in path.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(Error::GitFailed(format!(
                    "路径不允许符号链接: {}",
                    ancestor.display()
                )));
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn git_paths(repo: &Path, args: &[&str]) -> crate::Result<Vec<String>> {
    run_git_bytes(repo, args)?
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| {
            String::from_utf8(p.to_vec()).map_err(|_| Error::GitFailed("非 UTF-8 备份路径".into()))
        })
        .collect()
}

#[cfg(all(test, windows))]
fn restore_at(
    repo: &Path,
    live_vault: &Path,
    live_backups: &Path,
    timestamp: &str,
    full: bool,
) -> crate::Result<Vec<String>> {
    // Never create a replacement key during restore: that would hide key loss.
    crate::vault::keystore::with_file_lock(live_vault, || {
        let master = Zeroizing::new(crate::vault::keystore::load_master(live_vault)?);
        restore_with_master(repo, live_vault, live_backups, timestamp, full, &master)
    })
}

fn restore_with_master(
    repo: &Path,
    live_vault: &Path,
    live_backups: &Path,
    timestamp: &str,
    full: bool,
    master: &[u8; 32],
) -> crate::Result<Vec<String>> {
    let commit = find_commit(repo, timestamp)?;
    let tree: HashSet<String> = git_paths(repo, &["ls-tree", "-r", "-z", "--name-only", &commit])?
        .into_iter()
        .collect();
    let paths = if full {
        tree.iter().cloned().collect()
    } else {
        git_paths(
            repo,
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--name-only",
                "--no-renames",
                "-z",
                "-r",
                &commit,
            ],
        )?
    };
    type PlannedFile = (String, Option<Zeroizing<Vec<u8>>>);
    let mut changes: HashMap<PathBuf, PlannedFile> = HashMap::new();
    // Preflight every path and decrypt every blob before changing live state.
    for path in paths {
        if path == ".gitignore" {
            continue;
        }
        let live = map_path(&path, live_vault, live_backups)?;
        let content = if tree.contains(&path) {
            let blob = run_git_bytes(repo, &["show", &format!("{commit}:{path}")])?;
            Some(Zeroizing::new(if path.starts_with("backups/") {
                decrypt_backup_file(master, &blob)?
            } else {
                blob
            }))
        } else {
            None
        };
        changes.insert(live, (path, content));
    }
    if full {
        // Only remove previously managed files. Unknown local files and all key /
        // lock files are preserved, even if absent from the selected snapshot.
        for path in git_paths(repo, &["ls-tree", "-r", "-z", "--name-only", "HEAD"])? {
            if path == ".gitignore" || tree.contains(&path) {
                continue;
            }
            let live = map_path(&path, live_vault, live_backups)?;
            changes.entry(live).or_insert((path, None));
        }
    }
    let mut changes: Vec<_> = changes.into_iter().collect();
    changes.sort_by(|a, b| a.0.cmp(&b.0));
    let mut originals = Vec::new();
    for (path, _) in &changes {
        check_no_symlinks(path)?;
        let old = match std::fs::read(path) {
            Ok(bytes) => Some(Zeroizing::new(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        originals.push(old);
    }
    for (index, (path, (_, content))) in changes.iter().enumerate() {
        let apply = apply_file(path, content.as_deref().map(|v| v.as_slice()));
        if let Err(error) = apply {
            let mut rollback_errors = Vec::new();
            for i in (0..=index).rev() {
                if let Err(e) =
                    apply_file(&changes[i].0, originals[i].as_deref().map(|v| v.as_slice()))
                {
                    rollback_errors.push(e.to_string());
                }
            }
            return Err(Error::GitFailed(format!(
                "恢复失败: {error}; 回滚错误: {}",
                rollback_errors.join("; ")
            )));
        }
    }
    Ok(changes
        .into_iter()
        .map(|(_, (name, content))| {
            if content.is_some() {
                name
            } else {
                format!("-{name}")
            }
        })
        .collect())
}

fn apply_file(path: &Path, content: Option<&[u8]>) -> crate::Result<()> {
    check_no_symlinks(path)?;
    if let Some(bytes) = content {
        crate::vault::keystore::atomic_write_bytes(path, bytes)
    } else {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Explicit test-only key injection; production non-Windows storage stays fail-closed.
    fn backup_at(repo: &Path, vault: &Path, backups: &Path, message: &str) -> crate::Result<usize> {
        backup_with_master(repo, vault, backups, message, &[42; 32])
    }
    fn restore_at(
        repo: &Path,
        vault: &Path,
        backups: &Path,
        timestamp: &str,
        full: bool,
    ) -> crate::Result<Vec<String>> {
        restore_with_master(repo, vault, backups, timestamp, full, &[42; 32])
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentdeck-git-{}-{}", tag, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(p: &Path, content: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn backup_and_restore_roundtrip() {
        let root = temp_dir("roundtrip");
        let repo = root.join("repo");
        let vault = root.join("vault");
        let backups = root.join("backups");

        write(&vault.join("vault.json"), r#"{"keys":{}}"#);
        write(&backups.join("inject-1").join("config.toml"), "old config");
        let first = backup_at(&repo, &vault, &backups, "inject codex").unwrap();
        assert!(first >= 1, "首次备份应有变更");

        // 第二次无变更 → 0
        assert_eq!(backup_at(&repo, &vault, &backups, "noop").unwrap(), 0);

        // 变更后第二次备份
        write(&vault.join("vault.json"), r#"{"keys":{"a":1}}"#);
        write(&backups.join("inject-2").join("config.toml"), "new config");
        backup_at(&repo, &vault, &backups, "inject opencode").unwrap();

        let list = list_backups_at(&repo).unwrap();
        assert_eq!(list.len(), 2);
        let ts1 = list[1].timestamp.clone();

        // 按操作回滚：只还原第一个 commit 变更的文件
        let restored = restore_at(&repo, &vault, &backups, &ts1, false).unwrap();
        assert!(restored.contains(&"vault/vault.json".to_string()));
        assert_eq!(
            std::fs::read_to_string(vault.join("vault.json")).unwrap(),
            r#"{"keys":{}}"#
        );
    }

    #[test]
    fn full_restore_removes_newer_files() {
        let root = temp_dir("full");
        let repo = root.join("repo");
        let vault = root.join("vault");
        let backups = root.join("backups");

        write(&vault.join("vault.json"), "v1");
        backup_at(&repo, &vault, &backups, "first").unwrap();
        let list = list_backups_at(&repo).unwrap();
        let ts1 = list[0].timestamp.clone();

        write(&vault.join("vault.json"), "v2");
        write(&vault.join("extra.txt"), "extra");
        backup_at(&repo, &vault, &backups, "second").unwrap();

        let restored = restore_at(&repo, &vault, &backups, &ts1, true).unwrap();
        assert_eq!(
            std::fs::read_to_string(vault.join("vault.json")).unwrap(),
            "v1"
        );
        assert!(
            !vault.join("extra.txt").exists(),
            "整体恢复应删除之后新增的文件"
        );
        assert!(restored.iter().any(|r| r.starts_with('-')), "应有删除记录");
    }

    #[test]
    fn secret_scan_aborts_commit() {
        let root = temp_dir("secret");
        let repo = root.join("repo");
        let vault = root.join("vault");
        let backups = root.join("backups");

        write(&vault.join("vault.json"), "ok");
        backup_at(&repo, &vault, &backups, "base").unwrap();

        // 意外出现在仓库内的明文密钥（绕过加密镜像），扫描必须拦截
        write(
            &repo.join("leak.txt"),
            "api_key = sk-abcdefghijklmnopqrstuvwxyz",
        );
        let err = backup_at(&repo, &vault, &backups, "bad").unwrap_err();
        assert!(
            err.to_string().contains("疑似明文密钥"),
            "应中止提交: {}",
            err
        );
        assert_eq!(list_backups_at(&repo).unwrap().len(), 1, "不应产生新提交");
    }

    #[test]
    fn encrypted_backups_never_plaintext_and_restore_roundtrip() {
        let root = temp_dir("enc");
        let repo = root.join("repo");
        let vault = root.join("vault");
        let backups = root.join("backups");

        write(&vault.join("vault.json"), r#"{"keys":{}}"#);
        // 真实备份可能含明文密钥 → 入库必须加密
        write(
            &backups.join("inject-x").join("config.toml"),
            "experimental_bearer_token = \"sk-29ce80d25d8548cb9702251a8810abcd\"",
        );
        backup_at(&repo, &vault, &backups, "inject codex").unwrap();

        // 仓库内不得出现明文
        let staged = run_git(&repo, &["show", "HEAD:backups/inject-x/config.toml.gcm"]).unwrap();
        assert!(
            !staged.contains("sk-29ce80d25d8548cb9702251a8810abcd"),
            "仓库内不应出现明文密钥"
        );

        // 回滚后应还原明文
        write(&backups.join("inject-x").join("config.toml"), "被覆盖");
        let list = list_backups_at(&repo).unwrap();
        let ts = list[0].timestamp.clone();
        restore_at(&repo, &vault, &backups, &ts, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(backups.join("inject-x").join("config.toml")).unwrap(),
            "experimental_bearer_token = \"sk-29ce80d25d8548cb9702251a8810abcd\""
        );
    }

    #[test]
    fn restore_unknown_timestamp_errors() {
        let root = temp_dir("unknown");
        let repo = root.join("repo");
        let vault = root.join("vault");
        let backups = root.join("backups");
        write(&vault.join("vault.json"), "v1");
        backup_at(&repo, &vault, &backups, "first").unwrap();
        assert!(restore_at(&repo, &vault, &backups, "9999999999", false).is_err());
    }
    #[test]
    fn full_restore_preserves_keys_unmanaged_files_and_nested_paths() {
        let root = temp_dir("preserve");
        let (repo, vault, backups) = (root.join("repo"), root.join("vault"), root.join("backups"));
        write(&vault.join("master.key"), "irreplaceable wrapped key");
        write(&vault.join(".vault.lock"), "");
        write(&vault.join("nested/data.gcm"), "vault bytes");
        write(&backups.join("nested/配置.toml"), "secret");
        backup_at(&repo, &vault, &backups, "first").unwrap();
        let timestamp = list_backups_at(&repo).unwrap()[0].timestamp.clone();
        write(&vault.join("new.txt"), "managed later");
        backup_at(&repo, &vault, &backups, "second").unwrap();
        write(&vault.join("unmanaged.txt"), "local only");
        restore_at(&repo, &vault, &backups, &timestamp, true).unwrap();
        assert_eq!(
            std::fs::read_to_string(vault.join("master.key")).unwrap(),
            "irreplaceable wrapped key"
        );
        assert!(vault.join(".vault.lock").exists());
        assert!(vault.join("unmanaged.txt").exists());
        assert!(!vault.join("new.txt").exists());
        assert_eq!(
            std::fs::read_to_string(vault.join("nested/data.gcm")).unwrap(),
            "vault bytes"
        );
        assert_eq!(
            std::fs::read_to_string(backups.join("nested/配置.toml")).unwrap(),
            "secret"
        );
    }

    #[test]
    fn partial_restore_applies_deleted_paths() {
        let root = temp_dir("deleted");
        let (repo, vault, backups) = (root.join("repo"), root.join("vault"), root.join("backups"));
        write(&backups.join("deleted.txt"), "old");
        backup_at(&repo, &vault, &backups, "first").unwrap();
        std::fs::remove_file(backups.join("deleted.txt")).unwrap();
        backup_at(&repo, &vault, &backups, "delete").unwrap();
        let timestamp = list_backups_at(&repo).unwrap()[0].timestamp.clone();
        write(&backups.join("deleted.txt"), "recreated");
        let restored = restore_at(&repo, &vault, &backups, &timestamp, false).unwrap();
        assert!(!backups.join("deleted.txt").exists());
        assert!(restored.contains(&"-backups/deleted.txt.gcm".to_owned()));
    }

    #[test]
    fn restore_auth_failure_does_not_change_live_files() {
        let root = temp_dir("preflight");
        let (repo, vault, backups) = (root.join("repo"), root.join("vault"), root.join("backups"));
        write(&vault.join("vault.json"), "old");
        write(&backups.join("a.txt"), "secret");
        backup_at(&repo, &vault, &backups, "first").unwrap();
        let timestamp = list_backups_at(&repo).unwrap()[0].timestamp.clone();
        write(&vault.join("vault.json"), "current");
        write(&backups.join("a.txt"), "current secret");
        assert!(restore_with_master(&repo, &vault, &backups, &timestamp, true, &[99; 32]).is_err());
        assert_eq!(
            std::fs::read_to_string(vault.join("vault.json")).unwrap(),
            "current"
        );
        assert_eq!(
            std::fs::read_to_string(backups.join("a.txt")).unwrap(),
            "current secret"
        );
    }

    #[test]
    fn backup_nonce_is_random_and_legacy_ciphertext_still_decrypts() {
        use aes_gcm::aead::{Aead, KeyInit, Payload};
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        use sha2::{Digest, Sha256};
        let key = [42; 32];
        let raw = b"guessable secret";
        let (first, _) = encrypt_bytes(&key, raw).unwrap();
        let (second, _) = encrypt_bytes(&key, raw).unwrap();
        assert_ne!(first, second);
        let digest = Sha256::digest(raw);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let mut old = digest[..12].to_vec();
        old.extend(
            cipher
                .encrypt(
                    Nonce::from_slice(&digest[..12]),
                    Payload {
                        msg: raw,
                        aad: b"backup",
                    },
                )
                .unwrap(),
        );
        assert_eq!(decrypt_backup_file(&key, &old).unwrap(), raw);
        let root = temp_dir("migration");
        write(&root.join("live/file"), "guessable secret");
        std::fs::create_dir_all(root.join("mirror")).unwrap();
        std::fs::write(root.join("mirror/file.gcm"), &old).unwrap();
        encrypt_backups_tree(&root.join("live"), &root.join("mirror"), &key).unwrap();
        let migrated = std::fs::read(root.join("mirror/file.gcm")).unwrap();
        assert_ne!(&migrated[..12], &digest[..12]);
        encrypt_backups_tree(&root.join("live"), &root.join("mirror"), &key).unwrap();
        assert_eq!(
            std::fs::read(root.join("mirror/file.gcm")).unwrap(),
            migrated
        );
    }

    #[test]
    fn restore_rejects_portable_traversal_and_protected_paths() {
        let root = temp_dir("paths");
        for path in [
            "vault/../outside",
            "vault/master.key",
            "vault/.vault.lock",
            "backups/../x.gcm",
            "backups/.gcm",
            "vault/C:stream",
            "vault/a\\..\\outside",
            "vault/CON",
            "vault/a./x",
            "vault//x",
            "vault/./x",
        ] {
            assert!(
                map_path(path, &root.join("vault"), &root.join("backups")).is_err(),
                "{path}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn restore_and_backup_reject_symlink_escape() {
        use std::os::unix::fs::symlink;
        let root = temp_dir("symlink");
        std::fs::create_dir_all(root.join("vault")).unwrap();
        std::fs::create_dir_all(root.join("outside")).unwrap();
        write(&root.join("outside/secret"), "untouched");
        symlink(root.join("outside"), root.join("vault/link")).unwrap();
        assert!(
            map_path(
                "vault/link/secret",
                &root.join("vault"),
                &root.join("backups")
            )
            .is_err()
        );
        assert!(
            backup_at(
                &root.join("repo"),
                &root.join("vault"),
                &root.join("backups"),
                "symlink"
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(root.join("outside/secret")).unwrap(),
            "untouched"
        );
    }
    #[cfg(windows)]
    #[test]
    fn full_restore_keeps_real_vault_decryptable() {
        let root = temp_dir("dpapi-restore");
        let (repo, vault, backups) = (root.join("repo"), root.join("vault"), root.join("backups"));
        let handle = crate::vault::Vault::open(&vault);
        handle.save("test", "test", "old-secret").unwrap();
        super::backup_at(&repo, &vault, &backups, "first").unwrap();
        let timestamp = list_backups_at(&repo).unwrap()[0].timestamp.clone();
        let key_before = std::fs::read(vault.join("master.key")).unwrap();
        handle.save("test", "test", "new-secret").unwrap();
        super::backup_at(&repo, &vault, &backups, "second").unwrap();
        super::restore_at(&repo, &vault, &backups, &timestamp, true).unwrap();
        assert_eq!(std::fs::read(vault.join("master.key")).unwrap(), key_before);
        assert_eq!(handle.get("test").unwrap().as_deref(), Some("old-secret"));
    }
    #[test]
    fn backup_history_without_master_refuses_rotation() {
        let root = temp_dir("lost-master-history");
        let (repo, vault, backups) = (root.join("repo"), root.join("vault"), root.join("backups"));
        write(&backups.join("config"), "secret");
        backup_at(&repo, &vault, &backups, "config-only").unwrap();
        assert!(!vault.join("vault.json").exists());
        assert!(refuse_key_rotation(&repo, &vault).is_err());
        assert!(!vault.join("master.key").exists());
        write(&vault.join("master.key"), "present");
        refuse_key_rotation(&repo, &vault).unwrap();
    }
}
