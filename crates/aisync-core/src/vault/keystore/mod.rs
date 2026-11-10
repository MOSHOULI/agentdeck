//! 主密钥存储（M4）
//!
//! Windows：DPAPI（CryptProtectData）加密随机 32 字节主密钥，落盘
//! `master.key`（base64 编码的 DPAPI blob）——与 V2 的 `keystore_win.py` 格式完全兼容。
//! 其他平台：暂不支持（Windows 优先产品），返回明确错误。

use std::path::Path;

#[cfg(windows)]
use zeroize::Zeroizing;

use crate::Error;

const MASTER_LEN: usize = 32;

#[cfg(windows)]
mod dpapi {
    use windows_sys::Win32::Foundation::{HLOCAL, LocalFree};
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_PROMPTSTRUCT, CryptProtectData, CryptUnprotectData,
    };
    fn blob(data: &mut [u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_mut_ptr(),
        }
    }

    fn last_error() -> String {
        std::io::Error::last_os_error().to_string()
    }

    /// DPAPI 加密（当前 Windows 用户绑定，flags=0 与 V2 一致）
    pub fn encrypt(plain: &[u8]) -> crate::Result<Vec<u8>> {
        let mut buf = plain.to_vec();
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ret = unsafe {
            CryptProtectData(
                &blob(&mut buf),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null::<CRYPTPROTECT_PROMPTSTRUCT>(),
                0,
                &mut out,
            )
        };
        use zeroize::Zeroize;
        buf.zeroize();
        if ret == 0 {
            return Err(crate::Error::VaultError(format!(
                "DPAPI 加密失败: {}",
                last_error()
            )));
        }
        let result =
            unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe {
            use zeroize::Zeroize;
            std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize).zeroize();
            LocalFree(out.pbData as HLOCAL);
        }
        Ok(result)
    }

    /// DPAPI 解密
    pub fn decrypt(cipher: &[u8]) -> crate::Result<Vec<u8>> {
        let mut buf = cipher.to_vec();
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ret = unsafe {
            CryptUnprotectData(
                &blob(&mut buf),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null::<CRYPTPROTECT_PROMPTSTRUCT>(),
                0,
                &mut out,
            )
        };
        if ret == 0 {
            return Err(crate::Error::VaultError(format!(
                "DPAPI 解密失败: {}",
                last_error()
            )));
        }
        let result =
            unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe {
            use zeroize::Zeroize;
            std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize).zeroize();
            LocalFree(out.pbData as HLOCAL);
        }
        Ok(result)
    }
}

#[cfg(windows)]
fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    B64.encode(data)
}

#[cfg(windows)]
fn base64_decode(text: &str) -> crate::Result<Vec<u8>> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    B64.decode(text.trim())
        .map_err(|e| Error::VaultError(format!("base64 解码失败: {}", e)))
}

/// DPAPI 加密（release 签名私钥等复用）
#[cfg(windows)]
pub(crate) fn dpapi_encrypt_bytes(data: &[u8]) -> crate::Result<Vec<u8>> {
    dpapi::encrypt(data)
}

/// DPAPI 解密
#[cfg(windows)]
pub(crate) fn dpapi_decrypt_bytes(data: &[u8]) -> crate::Result<Vec<u8>> {
    dpapi::decrypt(data)
}

#[cfg(not(windows))]
pub(crate) fn dpapi_encrypt_bytes(_data: &[u8]) -> crate::Result<Vec<u8>> {
    Err(Error::VaultError("非 Windows 平台暂不支持 DPAPI".into()))
}

#[cfg(not(windows))]
pub(crate) fn dpapi_decrypt_bytes(_data: &[u8]) -> crate::Result<Vec<u8>> {
    Err(Error::VaultError("非 Windows 平台暂不支持 DPAPI".into()))
}

fn reject_symlinks(path: &Path) -> crate::Result<()> {
    for ancestor in path.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(Error::VaultError(format!(
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

/// Read-only existence check before opening a vault lock. Missing directories
/// are empty vaults, not a request to initialize one. Check ancestors first so
/// dangling links (including a linked ancestor above a missing vault) still fail.
pub(super) fn vault_directory_exists(dir: &Path) -> crate::Result<bool> {
    reject_symlinks(dir)?;
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => Ok(true),
        Ok(_) => Err(Error::VaultError("vault 路径不是目录".into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// 原子写（临时文件 + 替换）
pub(crate) fn atomic_write(path: &Path, content: &str) -> crate::Result<()> {
    atomic_write_bytes(path, content.as_bytes())
}

/// 原子写二进制（临时文件 + 替换）
pub(crate) fn atomic_write_bytes(path: &Path, content: &[u8]) -> crate::Result<()> {
    reject_symlinks(path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    use std::io::Write;
    let tmp = parent.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&tmp)?;
        file.write_all(content)?;
        file.sync_all()
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(error.into());
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

        let to_wide = |p: &Path| -> Vec<u16> {
            p.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        };
        let src = to_wide(&tmp);
        let dst = to_wide(path);
        const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
        let ret = unsafe { MoveFileExW(src.as_ptr(), dst.as_ptr(), MOVEFILE_REPLACE_EXISTING) };
        if ret == 0 {
            let _ = std::fs::remove_file(&tmp);
            return Err(Error::VaultError(format!(
                "原子写失败: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if let Err(error) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(error.into());
        }
        Ok(())
    }
}

/// OS-backed advisory lock: released on panic/process exit, never stolen by age.
/// The persistent lock file must not be removed (unlinking splits lock ownership).
pub(crate) fn with_file_lock<T>(
    dir: &Path,
    f: impl FnOnce() -> crate::Result<T>,
) -> crate::Result<T> {
    reject_symlinks(dir)?;
    std::fs::create_dir_all(dir)?;
    let lock_path = dir.join(".vault.lock");
    reject_symlinks(&lock_path)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(lock_path)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match lock.try_lock() {
            Ok(()) => return f(), // Dropping the file releases its lock, also on unwind.
            Err(std::fs::TryLockError::WouldBlock) => {
                if std::time::Instant::now() >= deadline {
                    return Err(Error::VaultError("vault 锁等待超时".into()));
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
    }
}

/// 读取主密钥；不存在时创建（创建过程带锁，防并发首启竞态）
#[cfg(windows)]
pub fn load_or_create_master(dir: &Path) -> crate::Result<[u8; MASTER_LEN]> {
    with_file_lock(dir, || load_or_create_master_unlocked(dir))
}

/// 不加锁版本（调用方必须已持有 vault 目录锁）
#[cfg(windows)]
pub(crate) fn load_or_create_master_unlocked(dir: &Path) -> crate::Result<[u8; MASTER_LEN]> {
    let path = dir.join("master.key");
    if path.exists() {
        return load_master(dir);
    }
    // Existing ciphertext with a missing key is corruption, not a fresh vault.
    if dir.join("vault.json").exists() {
        return Err(Error::VaultError(
            "主密钥丢失；拒绝生成替代密钥，请恢复原 master.key".into(),
        ));
    }
    let mut key = Zeroizing::new([0u8; MASTER_LEN]);
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut *key);
    let blob = dpapi::encrypt(&*key)?;
    atomic_write(&path, &base64_encode(&blob))?;
    Ok(*key)
}

/// 读取已存在的主密钥（V2 导入用；DPAPI 绑定当前用户，换用户会失败）
#[cfg(windows)]
pub fn load_master(dir: &Path) -> crate::Result<[u8; MASTER_LEN]> {
    let path = dir.join("master.key");
    reject_symlinks(&path)?;
    let text = std::fs::read_to_string(&path)?;
    let blob = base64_decode(&text)?;
    let key = Zeroizing::new(dpapi::decrypt(&blob)?);
    if key.len() != MASTER_LEN {
        return Err(Error::VaultError(format!(
            "主密钥长度异常: {}（应为 32）",
            key.len()
        )));
    }
    let mut out = [0u8; MASTER_LEN];
    out.copy_from_slice(&key);
    Ok(out)
}

#[cfg(not(windows))]
pub fn load_or_create_master(_dir: &Path) -> crate::Result<[u8; MASTER_LEN]> {
    Err(Error::VaultError(
        "非 Windows 平台暂不支持 DPAPI 主密钥（M4 仅实现 Windows）".into(),
    ))
}

#[cfg(not(windows))]
pub(crate) fn load_or_create_master_unlocked(dir: &Path) -> crate::Result<[u8; MASTER_LEN]> {
    load_or_create_master(dir)
}

#[cfg(not(windows))]
pub fn load_master(_dir: &Path) -> crate::Result<[u8; MASTER_LEN]> {
    Err(Error::VaultError(
        "非 Windows 平台暂不支持 DPAPI 主密钥（M4 仅实现 Windows）".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentdeck-keystore-{}-{}", tag, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_master_roundtrip_and_stable() {
        let dir = temp_dir("dpapi");
        let first = load_or_create_master(&dir).unwrap();
        let second = load_or_create_master(&dir).unwrap();
        assert_eq!(first, second, "同一 vault 目录主密钥应稳定");
        let text = std::fs::read_to_string(dir.join("master.key")).unwrap();
        assert!(!text.contains("key"), "master.key 不应包含明文");
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_encrypt_decrypt_roundtrip() {
        let dir = temp_dir("dpapi2");
        let master = load_or_create_master(&dir).unwrap();
        let blob = dpapi::encrypt(&master).unwrap();
        let back = dpapi::decrypt(&blob).unwrap();
        assert_eq!(master.as_slice(), back.as_slice());
    }
    #[test]
    fn lock_released_after_panic_and_serializes_threads() {
        let dir = temp_dir("lock");
        let _ = std::panic::catch_unwind(|| {
            let _ = with_file_lock(&dir, || -> crate::Result<()> { panic!("test unwind") });
        });
        with_file_lock(&dir, || Ok(())).unwrap();
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let dir = dir.clone();
                let count = count.clone();
                std::thread::spawn(move || {
                    with_file_lock(&dir, || {
                        use std::sync::atomic::Ordering;
                        assert_eq!(count.fetch_add(1, Ordering::SeqCst), 0);
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        assert_eq!(count.fetch_sub(1, Ordering::SeqCst), 1);
                        Ok(())
                    })
                    .unwrap()
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_is_private_and_replaces_existing() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("atomic");
        let file = dir.join("secret");
        atomic_write_bytes(&file, b"old").unwrap();
        atomic_write_bytes(&file, b"new").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"new");
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1);
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_platform_never_creates_plaintext_master() {
        let dir = temp_dir("unsupported");
        assert!(load_or_create_master_unlocked(&dir).is_err());
        assert!(load_or_create_master(&dir).is_err());
        assert!(load_master(&dir).is_err());
        assert!(!dir.join("master.key").exists());
    }
    #[cfg(windows)]
    #[test]
    fn missing_master_with_existing_vault_never_rotates_key() {
        let dir = temp_dir("missing-key");
        std::fs::write(dir.join("vault.json"), br#"{"keys":{}}"#).unwrap();
        assert!(load_or_create_master(&dir).is_err());
        assert!(!dir.join("master.key").exists());
    }
}
