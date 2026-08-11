//! 密钥库（vault）模块（M4）。
//!
//! AES-256-GCM 加密存储 API Key，密文格式与 V2 完全一致：
//! `vault.json` = `{"keys": {"<name>": {"provider", "nonce", "ct"}}}`，
//! 主密钥由 DPAPI 保护（`master.key`），明文密钥绝不落盘。
//!
//! 目录：`~/.agentdeck/vault/`；V2 数据在 `~/.aisync-vault/`，可一键导入。

pub mod keystore;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::Error;

const AAD: &[u8] = b"key";
const NONCE_LEN: usize = 12;
const VAULT_DIR: &str = ".agentdeck/vault";
const V2_DIR: &str = ".aisync-vault";

/// 密钥列表项（脱敏展示用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub name: String,
    pub provider: String,
    pub masked: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct VaultFile {
    keys: BTreeMap<String, KeyEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct KeyEntry {
    provider: String,
    nonce: String,
    ct: String,
}

fn home_dir() -> crate::Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| Error::Other("无法定位用户主目录".into()))
}

fn default_vault_dir() -> crate::Result<PathBuf> {
    Ok(home_dir()?.join(VAULT_DIR))
}

fn v2_vault_dir() -> crate::Result<PathBuf> {
    Ok(home_dir()?.join(V2_DIR))
}

fn validate_name(name: &str) -> crate::Result<()> {
    if name.trim().is_empty() || name.chars().any(|c| c.is_control()) {
        return Err(Error::VaultError("密钥名称不能为空或含控制字符".into()));
    }
    Ok(())
}

/// 脱敏：sk-abc12345def → sk-a***def；过短或非 ASCII 一律 `****`
fn mask_key(key: &str) -> String {
    if !key.is_ascii() || key.len() <= 8 {
        return "****".into();
    }
    format!("{}***{}", &key[..4], &key[key.len() - 4..])
}

fn encrypt(master: &[u8; 32], value: &str) -> crate::Result<(String, String)> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_bytes);
    let ct = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: value.as_bytes(),
                aad: AAD,
            },
        )
        .map_err(|_| Error::VaultError("AES-256-GCM 加密失败".into()))?;
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    Ok((B64.encode(nonce_bytes), B64.encode(ct)))
}

fn decrypt(master: &[u8; 32], nonce_b64: &str, ct_b64: &str) -> crate::Result<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    let nonce_bytes = B64
        .decode(nonce_b64.trim())
        .map_err(|e| Error::VaultError(format!("nonce 解码失败: {}", e)))?;
    let ct = B64
        .decode(ct_b64.trim())
        .map_err(|e| Error::VaultError(format!("密文解码失败: {}", e)))?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(Error::VaultError(format!(
            "nonce 长度异常: {}",
            nonce_bytes.len()
        )));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master));
    let mut plain = cipher
        .decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload { msg: &ct, aad: AAD },
        )
        .map_err(|_| Error::VaultError("解密失败（主密钥不匹配或数据损坏）".into()))?;
    let text = String::from_utf8(plain.clone())
        .map_err(|_| Error::VaultError("解密结果非 UTF-8".into()))?;
    plain.zeroize();
    Ok(text)
}

fn load_file(dir: &Path) -> crate::Result<VaultFile> {
    let path = dir.join("vault.json");
    if !path.exists() {
        return Ok(VaultFile::default());
    }
    let text = std::fs::read_to_string(&path)?;
    serde_json::from_str(&text)
        .map_err(|e| Error::VaultError(format!("vault.json 解析失败: {}", e)))
}

/// vault 句柄（目录可注入，便于测试）
pub struct Vault {
    dir: PathBuf,
}

impl Vault {
    pub fn open(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    pub fn open_default() -> crate::Result<Self> {
        Ok(Self::open(&default_vault_dir()?))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 保存/覆盖密钥（读改写带文件锁，防止并发丢数据）
    pub fn save(&self, name: &str, provider: &str, key: &str) -> crate::Result<()> {
        validate_name(name)?;
        keystore::with_file_lock(&self.dir, || {
            let master = keystore::load_or_create_master_unlocked(&self.dir)?;
            let (nonce, ct) = encrypt(&master, key)?;
            let mut file = load_file(&self.dir)?;
            file.keys.insert(
                name.to_string(),
                KeyEntry {
                    provider: provider.to_string(),
                    nonce,
                    ct,
                },
            );
            let text = serde_json::to_string_pretty(&file)
                .map_err(|e| Error::VaultError(format!("序列化失败: {}", e)))?;
            keystore::atomic_write(&self.dir.join("vault.json"), &text)
        })
    }

    /// 取明文密钥（复制用；主密钥不匹配时明确报错）
    pub fn get(&self, name: &str) -> crate::Result<Option<String>> {
        let master = keystore::load_or_create_master(&self.dir)?;
        let file = load_file(&self.dir)?;
        match file.keys.get(name) {
            None => Ok(None),
            Some(entry) => decrypt(&master, &entry.nonce, &entry.ct).map(Some),
        }
    }

    /// 列出密钥（脱敏）
    pub fn list(&self) -> crate::Result<Vec<KeyInfo>> {
        let master = keystore::load_or_create_master(&self.dir)?;
        let file = load_file(&self.dir)?;
        let mut out = Vec::new();
        for (name, entry) in file.keys {
            let plain = decrypt(&master, &entry.nonce, &entry.ct)?;
            out.push(KeyInfo {
                name,
                provider: entry.provider,
                masked: mask_key(&plain),
            });
        }
        Ok(out)
    }

    /// 删除密钥，返回是否存在
    pub fn delete(&self, name: &str) -> crate::Result<bool> {
        keystore::with_file_lock(&self.dir, || {
            let mut file = load_file(&self.dir)?;
            if file.keys.remove(name).is_none() {
                return Ok(false);
            }
            let text = serde_json::to_string_pretty(&file)
                .map_err(|e| Error::VaultError(format!("序列化失败: {}", e)))?;
            keystore::atomic_write(&self.dir.join("vault.json"), &text)?;
            Ok(true)
        })
    }
}

/// 保存 API Key（加密存入 vault）
pub fn save(agent_id: &str, provider: &str, key: &str) -> crate::Result<()> {
    Vault::open_default()?.save(agent_id, provider, key)
}

/// 列出已存储的 Key（脱敏）
pub fn list() -> crate::Result<Vec<KeyInfo>> {
    Vault::open_default()?.list()
}

/// 取明文 Key（复制用）
pub fn get(agent_id: &str) -> crate::Result<Option<String>> {
    Vault::open_default()?.get(agent_id)
}

/// 删除 Key
pub fn delete(agent_id: &str) -> crate::Result<bool> {
    Vault::open_default()?.delete(agent_id)
}

/// 从 V2 vault（`~/.aisync-vault`）导入密钥到当前 vault；
/// 已存在的名称跳过，返回导入条数。
pub fn import_from_v2() -> crate::Result<usize> {
    import_from_v2_at(&v2_vault_dir()?, &default_vault_dir()?)
}

fn import_from_v2_at(v2_dir: &Path, v3_dir: &Path) -> crate::Result<usize> {
    let v2_file = v2_dir.join("vault.json");
    if !v2_file.exists() {
        return Ok(0);
    }
    // V2 master.key 与 V3 同为 DPAPI 保护，可直接解密
    let v2_master = keystore::load_master(v2_dir)?;
    let text = std::fs::read_to_string(&v2_file)?;
    let file: VaultFile = serde_json::from_str(&text)
        .map_err(|e| Error::VaultError(format!("V2 vault.json 解析失败: {}", e)))?;

    let v3 = Vault::open(v3_dir);
    let mut count = 0usize;
    for (name, entry) in file.keys {
        if v3.get(&name)?.is_some() {
            continue;
        }
        let plain = decrypt(&v2_master, &entry.nonce, &entry.ct)?;
        v3.save(&name, &entry.provider, &plain)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentdeck-vault-{}-{}", tag, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn aes_gcm_roundtrip_and_tamper() {
        let master = [7u8; 32];
        let (nonce, ct) = encrypt(&master, "sk-秘密值-123456").unwrap();
        assert_eq!(decrypt(&master, &nonce, &ct).unwrap(), "sk-秘密值-123456");

        let other = [8u8; 32];
        assert!(
            decrypt(&other, &nonce, &ct).is_err(),
            "错误主密钥应解密失败"
        );
        let tampered = format!("{}1", ct);
        assert!(
            decrypt(&master, &nonce, &tampered).is_err(),
            "篡改密文应失败"
        );
    }

    #[test]
    fn mask_key_cases() {
        assert_eq!(mask_key("sk-abcdefgh12345678"), "sk-a***5678");
        assert_eq!(mask_key("short"), "****");
        assert_eq!(mask_key("密钥中文"), "****");
    }

    #[cfg(windows)]
    #[test]
    fn vault_save_get_list_delete_roundtrip() {
        let dir = temp_dir("crud");
        let v = Vault::open(&dir);
        v.save("codex", "manual", "sk-codex-abc-12345678").unwrap();
        v.save("opencode", "env", "opencode-token-999").unwrap();

        assert_eq!(
            v.get("codex").unwrap().as_deref(),
            Some("sk-codex-abc-12345678")
        );
        assert_eq!(v.get("nonexistent").unwrap(), None);

        let list = v.list().unwrap();
        assert_eq!(list.len(), 2);
        let codex = list.iter().find(|k| k.name == "codex").unwrap();
        assert_eq!(codex.provider, "manual");
        assert_eq!(codex.masked, "sk-c***5678");
        assert!(list.iter().all(|k| !k.masked.contains("token-999")));

        assert!(v.delete("codex").unwrap());
        assert!(!v.delete("codex").unwrap());
        assert_eq!(v.get("codex").unwrap(), None);
        assert_eq!(v.list().unwrap().len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn vault_json_never_plaintext() {
        let dir = temp_dir("plaintext");
        let v = Vault::open(&dir);
        v.save("secret", "manual", "plain-secret-value-123")
            .unwrap();
        let text = std::fs::read_to_string(dir.join("vault.json")).unwrap();
        assert!(
            !text.contains("plain-secret-value-123"),
            "vault.json 不应含明文"
        );
        assert!(text.contains("nonce") && text.contains("ct"));
    }

    #[cfg(windows)]
    #[test]
    fn concurrent_saves_no_loss() {
        let dir = temp_dir("concurrent");
        let n = 8;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(n));
        let handles: Vec<_> = (0..n)
            .map(|i| {
                let dir = dir.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    Vault::open(&dir).save(&format!("k{}", i), "t", &format!("v{}", i))
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap().unwrap();
        }
        let v = Vault::open(&dir);
        for i in 0..n {
            assert_eq!(
                v.get(&format!("k{}", i)).unwrap().as_deref(),
                Some(format!("v{}", i).as_str())
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn import_from_v2_merges_skipping_existing() {
        let v2 = temp_dir("v2");
        let v3 = temp_dir("v3");

        // 构造 V2 格式目录：DPAPI master.key + V2 vault.json
        let v2_vault = Vault::open(&v2);
        v2_vault.save("OLD_KEY", "env", "old-value-123456").unwrap();
        v2_vault.save("SHARED", "env", "v2-value").unwrap();

        // V3 已有同名 SHARED，导入时应跳过
        let v3_vault = Vault::open(&v3);
        v3_vault.save("SHARED", "manual", "v3-value").unwrap();

        let count = import_from_v2_at(&v2, &v3).unwrap();
        assert_eq!(count, 1, "只应导入 OLD_KEY");
        assert_eq!(
            v3_vault.get("OLD_KEY").unwrap().as_deref(),
            Some("old-value-123456")
        );
        assert_eq!(
            v3_vault.get("SHARED").unwrap().as_deref(),
            Some("v3-value"),
            "已存在密钥不应被覆盖"
        );
    }

    #[cfg(windows)]
    #[test]
    fn import_from_v2_returns_zero_when_missing() {
        let v2 = temp_dir("v2missing");
        let v3 = temp_dir("v3empty");
        assert_eq!(import_from_v2_at(&v2, &v3).unwrap(), 0);
    }

    /// 真实环境验证：真实 vault 增删改查 + git 备份 + V2 导入（手动运行）
    #[test]
    #[ignore = "真实环境验证：写入 ~/.agentdeck/vault 与 ~/.agentdeck/git-backup"]
    fn real_vault_and_backup_smoke() {
        let v = Vault::open_default().unwrap();
        v.save("smoke-test", "manual", "sk-smoke-12345678").unwrap();
        assert_eq!(
            v.get("smoke-test").unwrap().as_deref(),
            Some("sk-smoke-12345678")
        );
        assert!(v.list().unwrap().iter().any(|k| k.name == "smoke-test"));
        assert!(v.delete("smoke-test").unwrap());
        println!("vault 真实增删改查通过");

        let n = crate::git::backup("vault: real smoke test").unwrap();
        println!("git 备份提交文件数: {}", n);

        let imported = import_from_v2().unwrap();
        println!("V2 导入条数: {}", imported);
    }
}
