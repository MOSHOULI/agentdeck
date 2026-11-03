//! 发布签名模块（M6）。
//!
//! ed25519 签名更新链（与 V2 设计一致）：
//! - 密钥对：ring Ed25519，私钥（PKCS8）用 DPAPI 加密存 `~/.agentdeck/release/signing-key.enc`，不入仓库
//! - manifest：`{version, min_version, released, files[{name,url,sha256,size}], pubkey_fp}`
//! - 签名：对 canonical JSON（sort_keys）签 ed25519，输出 `.sig`（base64）
//! - 客户端：公钥内置验签 + min_version 防降级

use std::path::{Path, PathBuf};

use ring::rand::SystemRandom;
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
use serde::{Deserialize, Serialize};

use crate::Error;

const RELEASE_DIR: &str = ".agentdeck/release";
const KEY_FILE: &str = "signing-key.enc";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseManifest {
    pub version: String,
    pub min_version: String,
    pub released: String,
    pub files: Vec<ManifestFile>,
    pub pubkey_fp: String,
}

fn home_dir() -> crate::Result<PathBuf> {
    crate::home_dir().ok_or_else(|| Error::Other("无法定位用户主目录".into()))
}

pub fn release_dir() -> crate::Result<PathBuf> {
    Ok(home_dir()?.join(RELEASE_DIR))
}

pub fn signing_key_path() -> crate::Result<PathBuf> {
    Ok(release_dir()?.join(KEY_FILE))
}

/// 应用内置的发布公钥（base64）——v1.0.0 发布密钥对（2026-08-11 生成，DPAPI 存本机）
pub const EMBEDDED_PUBKEY_B64: &str = "oQ7ynIBaiXY/4Z4i+ltgmDcdKIbFjPNsMW6PptkaXxE=";

/// 解码内置公钥；未配置时返回 None（未发布版本无更新能力）
pub fn embedded_public_key() -> Option<Vec<u8>> {
    if EMBEDDED_PUBKEY_B64.is_empty() {
        return None;
    }
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    B64.decode(EMBEDDED_PUBKEY_B64).ok()
}

/// 公钥指纹：sha256 前 8 字节 hex
pub fn pubkey_fingerprint(pubkey: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(pubkey);
    digest[..8].iter().map(|b| format!("{:02x}", b)).collect()
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 生成新密钥对，返回 (pkcs8, 公钥)
pub fn generate_keypair() -> crate::Result<(Vec<u8>, Vec<u8>)> {
    let rng = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| Error::Other("ed25519 密钥生成失败".into()))?;
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
        .map_err(|_| Error::Other("ed25519 密钥解析失败".into()))?;
    Ok((pkcs8.as_ref().to_vec(), pair.public_key().as_ref().to_vec()))
}

/// 读取或创建签名密钥（DPAPI 加密落盘）；返回 (公钥, pkcs8)
pub fn load_or_create_signing_key() -> crate::Result<(Vec<u8>, Vec<u8>)> {
    let path = signing_key_path()?;
    if path.exists() {
        let enc = std::fs::read(&path)?;
        let pkcs8 = crate::vault::keystore::dpapi_decrypt_bytes(&enc)?;
        let pair = Ed25519KeyPair::from_pkcs8(&pkcs8)
            .map_err(|_| Error::Other("签名密钥解析失败（数据损坏？）".into()))?;
        return Ok((pair.public_key().as_ref().to_vec(), pkcs8));
    }
    let (pkcs8, pubkey) = generate_keypair()?;
    let dir = release_dir()?;
    std::fs::create_dir_all(&dir)?;
    let enc = crate::vault::keystore::dpapi_encrypt_bytes(&pkcs8)?;
    crate::vault::keystore::atomic_write_bytes(&path, &enc)?;
    Ok((pubkey, pkcs8))
}

/// canonical JSON（键排序，签名/验签双方一致）
/// canonical JSON（签名与 manifest 文件输出统一使用，保证验签一致）
pub fn canonical_manifest_json(manifest: &ReleaseManifest) -> crate::Result<String> {
    serde_json::to_string(manifest).map_err(|e| Error::Other(format!("manifest 序列化失败: {}", e)))
}

/// 对 manifest 签名，返回签名字节
pub fn sign_manifest(manifest: &ReleaseManifest, pkcs8: &[u8]) -> crate::Result<Vec<u8>> {
    let pair =
        Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|_| Error::Other("签名密钥解析失败".into()))?;
    let json = canonical_manifest_json(manifest)?;
    Ok(pair.sign(json.as_bytes()).as_ref().to_vec())
}

/// 验签（公钥）
pub fn verify_manifest(manifest_json: &str, sig: &[u8], pubkey: &[u8]) -> bool {
    UnparsedPublicKey::new(&ED25519, pubkey)
        .verify(manifest_json.as_bytes(), sig)
        .is_ok()
}

/// 生成 manifest + 签名（files: (名称, URL, 本地文件路径)）
pub fn make_manifest(
    version: &str,
    min_version: &str,
    released: &str,
    files: &[(&str, &str, &Path)],
) -> crate::Result<(ReleaseManifest, Vec<u8>)> {
    let (pubkey, pkcs8) = load_or_create_signing_key()?;
    let mut manifest_files = Vec::new();
    for (name, url, path) in files {
        let data = std::fs::read(path)?;
        manifest_files.push(ManifestFile {
            name: name.to_string(),
            url: url.to_string(),
            sha256: sha256_hex(&data),
            size: data.len() as u64,
        });
    }
    let manifest = ReleaseManifest {
        version: version.to_string(),
        min_version: min_version.to_string(),
        released: released.to_string(),
        files: manifest_files,
        pubkey_fp: pubkey_fingerprint(&pubkey),
    };
    let sig = sign_manifest(&manifest, &pkcs8)?;
    Ok((manifest, sig))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keygen_sign_verify_roundtrip() {
        let (pkcs8, pubkey) = generate_keypair().unwrap();
        let msg = b"hello release";
        let pair = Ed25519KeyPair::from_pkcs8(&pkcs8).unwrap();
        let sig = pair.sign(msg).as_ref().to_vec();
        assert!(verify_manifest(
            std::str::from_utf8(msg).unwrap(),
            &sig,
            &pubkey
        ));
        let mut tampered = sig.clone();
        tampered[0] ^= 0x01;
        assert!(!verify_manifest(
            std::str::from_utf8(msg).unwrap(),
            &tampered,
            &pubkey
        ));
    }

    #[test]
    fn fingerprint_is_stable_and_short() {
        let (_, pubkey) = generate_keypair().unwrap();
        let fp = pubkey_fingerprint(&pubkey);
        assert_eq!(fp.len(), 16);
        assert_eq!(fp, pubkey_fingerprint(&pubkey));
    }

    #[test]
    fn canonical_json_deterministic() {
        let (pubkey, _) = generate_keypair().unwrap();
        let m = ReleaseManifest {
            version: "1.0.0".into(),
            min_version: "0.9.0".into(),
            released: "2026-08-11".into(),
            files: vec![ManifestFile {
                name: "a.json".into(),
                url: "https://x/a.json".into(),
                sha256: "ab".into(),
                size: 2,
            }],
            pubkey_fp: pubkey_fingerprint(&pubkey),
        };
        let a = canonical_manifest_json(&m).unwrap();
        let b = canonical_manifest_json(&m).unwrap();
        assert_eq!(a, b);
    }
}
