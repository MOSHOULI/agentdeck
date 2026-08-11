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

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    dirs::home_dir().ok_or_else(|| Error::Other("无法定位用户主目录".into()))
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
    for entry in walkdir::WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // master.key 等密钥文件绝不进仓库（即使被 .gitignore 兜底）
        if path.extension().and_then(|e| e.to_str()) == Some("key") {
            continue;
        }
        let rel = path
            .strip_prefix(src)
            .map_err(|_| Error::GitFailed("备份路径解析失败".into()))?;
        let target = dst.join(rel);
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
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in walkdir::WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
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
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = std::fs::read(path)?;
        let (nonce, ct) = encrypt_bytes(master, &raw)?;
        let mut data = Vec::with_capacity(nonce.len() + ct.len());
        data.extend_from_slice(&nonce);
        data.extend_from_slice(&ct);
        crate::vault::keystore::atomic_write_bytes(&target, &data)?;
        count += 1;
    }
    Ok(count)
}

fn encrypt_bytes(master: &[u8; 32], raw: &[u8]) -> crate::Result<([u8; 12], Vec<u8>)> {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    use sha2::{Digest, Sha256};

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master));
    // 确定性 nonce = SHA-256(内容) 前 12 字节。
    // 同一主密钥下：相同内容 → 相同 nonce → 相同密文（备份去重）；
    // nonce 相同且内容相同不构成 GCM nonce 重用风险（仅暴露内容相等性，本就已知）。
    let digest = Sha256::digest(raw);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&digest[..12]);
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
    ensure_repo(repo)?;
    let master = crate::vault::keystore::load_or_create_master(live_vault)?;
    // 重建镜像（git 历史兜底，删除可恢复）；防止上次残留的明文镜像被再次暂存
    clear_dir(&repo.join("vault"))?;
    clear_dir(&repo.join("backups"))?;
    copy_tree(live_vault, &repo.join("vault"))?;
    encrypt_backups_tree(live_backups, &repo.join("backups"), &master)?;
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
    // 回滚前先拍当前状态快照，防止选错时间戳导致数据不可逆丢失
    backup("restore: 回滚前快照")?;
    restore_at(
        &default_repo_dir()?,
        &live_vault_dir()?,
        &live_backups_dir()?,
        timestamp,
        full,
    )
}

fn find_commit(repo: &Path, timestamp: &str) -> crate::Result<String> {
    let log = list_backups_at(repo)?;
    for info in log {
        if info.timestamp == timestamp {
            return Ok(info.hash);
        }
    }
    Err(Error::GitFailed(format!(
        "未找到时间戳 {} 的备份（可用 list_backups 查看）",
        timestamp
    )))
}

/// 仓库相对路径 → 实际文件位置（`.gcm` 后缀还原为原文件名）
fn map_path(repo_rel: &str, live_vault: &Path, live_backups: &Path) -> crate::Result<PathBuf> {
    let safe = |rel: &str| -> crate::Result<PathBuf> {
        let p = Path::new(rel);
        if p.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) {
            return Err(Error::GitFailed(format!("非法备份路径: {}", rel)));
        }
        Ok(p.to_path_buf())
    };
    if let Some(rel) = repo_rel.strip_prefix("vault/") {
        Ok(live_vault.join(safe(rel)?))
    } else if let Some(rel) = repo_rel.strip_prefix("backups/") {
        let rel_path = safe(rel)?;
        let name = rel_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::GitFailed("备份文件名非法".into()))?
            .strip_suffix(".gcm")
            .ok_or_else(|| Error::GitFailed("备份文件缺少 .gcm 后缀".into()))?;
        let mut plain = rel_path.clone();
        plain.set_file_name(name);
        Ok(live_backups.join(plain))
    } else {
        Err(Error::GitFailed(format!("未知仓库路径: {}", repo_rel)))
    }
}

fn restore_at(
    repo: &Path,
    live_vault: &Path,
    live_backups: &Path,
    timestamp: &str,
    full: bool,
) -> crate::Result<Vec<String>> {
    let commit = find_commit(repo, timestamp)?;
    let paths: Vec<String> = if full {
        run_git(repo, &["ls-tree", "-r", "--name-only", &commit])?
            .lines()
            .map(|l| l.to_string())
            .collect()
    } else {
        run_git(
            repo,
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--name-only",
                "-r",
                &commit,
            ],
        )?
        .lines()
        .map(|l| l.to_string())
        .collect()
    };

    // 主密钥只加载一次（DPAPI 解密开销大）
    let master = if paths.iter().any(|p| p.starts_with("backups/")) {
        Some(crate::vault::keystore::load_or_create_master(live_vault)?)
    } else {
        None
    };

    let mut restored = Vec::new();
    let mut tracked = HashSet::new();
    for path in &paths {
        if path == ".gitignore" {
            continue;
        }
        let live = map_path(path, live_vault, live_backups)?;
        let repo_content = run_git_bytes(repo, &["show", &format!("{}:{}", commit, path)])?;
        let content = if path.starts_with("backups/") {
            let m = master
                .as_ref()
                .ok_or_else(|| Error::GitFailed("备份解密缺少主密钥".into()))?;
            decrypt_backup_file(m, &repo_content)?
        } else {
            repo_content
        };
        if let Some(parent) = live.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&live, content)?;
        // 用于整体恢复时与真实文件比对（去掉 .gcm 后缀）
        let tracked_key = path
            .strip_suffix(".gcm")
            .unwrap_or(path.as_str())
            .to_string();
        tracked.insert(tracked_key);
        restored.push(path.clone());
    }

    // 整体恢复：删除目标时间点之后新增的文件，回到该时间点状态
    if full {
        for (live_dir, prefix) in [(live_vault, "vault"), (live_backups, "backups")] {
            if !live_dir.exists() {
                continue;
            }
            for entry in walkdir::WalkDir::new(live_dir)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.path().is_file() {
                    continue;
                }
                let rel = entry
                    .path()
                    .strip_prefix(live_dir)
                    .map_err(|_| Error::GitFailed("整体恢复路径解析失败".into()))?;
                let key = format!("{}/{}", prefix, rel.display());
                if !tracked.contains(&key) {
                    let _ = std::fs::remove_file(entry.path());
                    restored.push(format!("-{}", key));
                }
            }
        }
    }
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
