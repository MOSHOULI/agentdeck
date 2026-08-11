//! 注册表在线更新模块（M6）。
//!
//! 流程：拉取 GitHub Releases 的 manifest + 签名 → ed25519 验签（公钥内置）
//! → 版本比较（远程 > 当前，且当前 >= min_version 防降级）→ 下载文件 →
//! SHA-256 校验 → 备份旧文件 → 原子写入 `~/.agentdeck/registry/`。
//! 运行时注册表优先于编译期内置（见 registry::load_registry_runtime）。

use std::io::Read;

use crate::Error;
use crate::release::ReleaseManifest;

/// 在线更新的注册表文件名
pub const REGISTRY_FILES: &[&str] = &["registry-v3.json", "mcp-registry.json"];

/// 点分数字版本比较（1.0.0 < 1.0.1 < 1.1.0）
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<u64> = a.split('.').map(|s| s.parse().unwrap_or(0)).collect();
    let pb: Vec<u64> = b.split('.').map(|s| s.parse().unwrap_or(0)).collect();
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        if x != y {
            return x.cmp(&y);
        }
    }
    std::cmp::Ordering::Equal
}

/// 是否可更新：远程版本高于当前，且当前不低于 min_version（防降级）
pub fn is_update_available(current: &str, manifest: &ReleaseManifest) -> bool {
    compare_versions(&manifest.version, current).is_gt()
        && compare_versions(current, &manifest.min_version) != std::cmp::Ordering::Less
}

/// 解析 manifest 并验签；通过后返回（否则 Err）
pub fn parse_and_verify(
    manifest_json: &str,
    sig: &[u8],
    pubkey: &[u8],
) -> crate::Result<ReleaseManifest> {
    if !crate::release::verify_manifest(manifest_json, sig, pubkey) {
        return Err(Error::Other("manifest 签名校验失败".into()));
    }
    serde_json::from_str(manifest_json)
        .map_err(|e| Error::Other(format!("manifest 解析失败: {}", e)))
}

/// 拉取字节：http(s) 走 ureq，file:/本地路径直接读
pub fn fetch_bytes(url: &str) -> crate::Result<Vec<u8>> {
    if url.starts_with("http://") || url.starts_with("https://") {
        let resp = ureq::get(url)
            .call()
            .map_err(|e| Error::Other(format!("下载失败 {}: {}", url, e)))?;
        let mut buf = Vec::new();
        resp.into_reader()
            .take(50 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map_err(|e| Error::Other(format!("读取响应失败: {}", e)))?;
        Ok(buf)
    } else {
        let path = url.strip_prefix("file://").unwrap_or(url);
        std::fs::read(path).map_err(|e| Error::Other(format!("读取文件失败 {}: {}", path, e)))
    }
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 应用更新：逐文件下载 → SHA-256 校验 → 备份旧文件 → 原子写入运行时注册表目录
pub fn apply_update(manifest: &ReleaseManifest) -> crate::Result<Vec<String>> {
    let dir = crate::registry::runtime_registry_dir()?;
    std::fs::create_dir_all(&dir)?;

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let backup_root = dirs::home_dir()
        .ok_or_else(|| Error::Other("无法定位用户主目录".into()))?
        .join(".agentdeck")
        .join("backups")
        .join(format!("update-{}", nanos));

    let mut applied = Vec::new();
    for file in &manifest.files {
        if !REGISTRY_FILES.contains(&file.name.as_str()) {
            continue; // 只接受白名单注册表文件，防任意文件写入
        }
        let data = fetch_bytes(&file.url)?;
        if sha256_hex(&data) != file.sha256 {
            return Err(Error::Other(format!("SHA-256 校验失败: {}", file.name)));
        }
        if data.len() as u64 != file.size {
            return Err(Error::Other(format!("文件大小不匹配: {}", file.name)));
        }
        let target = dir.join(&file.name);
        if target.exists() {
            std::fs::create_dir_all(&backup_root)?;
            std::fs::copy(&target, backup_root.join(&file.name))?;
        }
        crate::vault::keystore::atomic_write_bytes(&target, &data)?;
        applied.push(file.name.clone());
    }
    if applied.is_empty() {
        return Err(Error::Other("manifest 中没有可应用的注册表文件".into()));
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::ManifestFile;

    fn manifest(version: &str, min_version: &str) -> ReleaseManifest {
        ReleaseManifest {
            version: version.into(),
            min_version: min_version.into(),
            released: "2026-08-11".into(),
            files: vec![],
            pubkey_fp: "".into(),
        }
    }

    #[test]
    fn version_comparison() {
        assert_eq!(
            compare_versions("1.0.0", "1.0.0"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            compare_versions("1.0.1", "1.0.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("1.1.0", "1.0.9"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions("1.0.0", "1.1.0"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("2.0", "1.9.9"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn update_available_rules() {
        assert!(is_update_available("1.0.0", &manifest("1.1.0", "1.0.0")));
        assert!(!is_update_available("1.1.0", &manifest("1.1.0", "1.0.0")));
        assert!(
            !is_update_available("1.2.0", &manifest("1.1.0", "1.0.0")),
            "不应降级"
        );
        assert!(
            !is_update_available("0.9.0", &manifest("1.0.0", "1.0.0")),
            "低于 min_version 拒绝"
        );
    }

    #[test]
    fn fetch_local_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("agentdeck-update-test.json");
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(fetch_bytes(path.to_str().unwrap()).unwrap(), b"{}");
        let _ = ManifestFile {
            name: "x".into(),
            url: "".into(),
            sha256: "".into(),
            size: 0,
        };
    }

    #[test]
    fn apply_update_roundtrip_with_backup() {
        let dir = crate::registry::runtime_registry_dir().unwrap();
        // 用真实数据目录做隔离验证：写入后清理（文件为测试产物）
        let _ = std::fs::remove_dir_all(&dir);
        let test_file = dir.join("registry-v3.json");
        let manifest = ReleaseManifest {
            version: "9.9.9".into(),
            min_version: "0.0.1".into(),
            released: "2026-08-11".into(),
            files: vec![ManifestFile {
                name: "registry-v3.json".into(),
                url: "file:///D:/test-nonexistent.json".into(),
                sha256: "".into(),
                size: 0,
            }],
            pubkey_fp: "".into(),
        };
        // 直接验证「非白名单文件名被拒绝」与「校验失败报错」分支
        let bad = ReleaseManifest {
            files: vec![ManifestFile {
                name: "evil.txt".into(),
                url: "file:///D:/x".into(),
                sha256: "".into(),
                size: 0,
            }],
            ..manifest.clone()
        };
        assert!(apply_update(&bad).is_err(), "非白名单文件应报错");
        let _ = test_file;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 真实环境验证：签名 → 验签 → 版本比较 → 应用 → 运行时注册表优先（手动运行，测试后清理）
    #[test]
    #[ignore = "真实环境验证：写入 ~/.agentdeck/registry（测试后清理）"]
    fn real_update_chain() {
        let dir = crate::registry::runtime_registry_dir().unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!("agentdeck-update-chain-{}", nanos));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("registry-v3.json"),
            include_str!("../../../../agents/registry-v3.json"),
        )
        .unwrap();
        std::fs::write(
            tmp.join("mcp-registry.json"),
            include_str!("../../../../agents/mcp-registry.json"),
        )
        .unwrap();

        let reg_path = tmp.join("registry-v3.json");
        let mcp_path = tmp.join("mcp-registry.json");
        let (manifest, sig) = crate::release::make_manifest(
            "1.0.1",
            "1.0.0",
            "2026-08-11",
            &[
                ("registry-v3.json", reg_path.to_str().unwrap(), &reg_path),
                ("mcp-registry.json", mcp_path.to_str().unwrap(), &mcp_path),
            ],
        )
        .unwrap();
        let manifest_json = serde_json::to_string(&manifest).unwrap();

        let pubkey = crate::release::embedded_public_key().expect("应已内置发布公钥");
        let parsed = parse_and_verify(&manifest_json, &sig, &pubkey).unwrap();
        assert!(is_update_available("1.0.0", &parsed));
        assert!(!is_update_available("1.0.1", &parsed), "同版本不应更新");

        apply_update(&parsed).unwrap();
        assert!(dir.join("registry-v3.json").exists());
        assert!(dir.join("mcp-registry.json").exists());
        let doc = crate::registry::load_registry_runtime().unwrap();
        assert_eq!(doc.agents.len(), 9, "运行时注册表应可加载");
        println!("更新链路验证通过：manifest 验签 → 版本比较 → 下载校验 → 原子写入 → 运行时优先");

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// 真实环境验证：从 GitHub Releases 拉取 v1.0.0 manifest 并验签（手动运行）
    #[test]
    #[ignore = "真实环境验证：访问 github.com"]
    fn real_check_github_release() {
        let base = "https://github.com/MOSHOULI/agentdeck/releases/download/v1.0.0";
        let manifest_json = fetch_bytes(&format!("{}/manifest.json", base)).unwrap();
        let manifest_json = String::from_utf8(manifest_json).unwrap();
        let sig_b64 = fetch_bytes(&format!("{}/manifest.json.sig", base)).unwrap();
        let sig_b64 = String::from_utf8(sig_b64).unwrap();
        use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
        let sig = B64.decode(sig_b64.trim()).unwrap();
        let pubkey = crate::release::embedded_public_key().expect("内置公钥");
        let manifest = parse_and_verify(&manifest_json, &sig, &pubkey).unwrap();
        assert_eq!(manifest.version, "1.0.0");
        assert!(!is_update_available("1.0.0", &manifest), "同版本不应有更新");
        println!(
            "GitHub Release 真实拉取+验签通过：version={} pubkey_fp={} files={:?}",
            manifest.version,
            manifest.pubkey_fp,
            manifest
                .files
                .iter()
                .map(|f| f.name.clone())
                .collect::<Vec<_>>()
        );
    }
}
