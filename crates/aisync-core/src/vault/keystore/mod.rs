//! 主密钥存储（M4）
//!
//! Windows：DPAPI（CryptProtectData）加密随机 32 字节主密钥，落盘
//! `master.key`（base64 编码的 DPAPI blob）——与 V2 的 `keystore_win.py` 格式完全兼容。
//! 其他平台：暂不支持（Windows 优先产品），返回明确错误。

use std::path::Path;

use zeroize::Zeroize;

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
        if ret == 0 {
            return Err(crate::Error::VaultError(format!(
                "DPAPI 加密失败: {}",
                last_error()
            )));
        }
        let result =
            unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe {
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
            LocalFree(out.pbData as HLOCAL);
        }
        Ok(result)
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    B64.encode(data)
}

fn base64_decode(text: &str) -> crate::Result<Vec<u8>> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    B64.decode(text.trim())
        .map_err(|e| Error::VaultError(format!("base64 解码失败: {}", e)))
}

/// 原子写（临时文件 + 替换）
pub(crate) fn atomic_write(path: &Path, content: &str) -> crate::Result<()> {
    atomic_write_bytes(path, content.as_bytes())
}

/// 原子写二进制（临时文件 + 替换）
pub(crate) fn atomic_write_bytes(path: &Path, content: &[u8]) -> crate::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = parent.join(format!(".tmp-{}", nanos));
    std::fs::write(&tmp, content)?;

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
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// vault 目录级文件锁（跨进程/跨线程互斥，create_new + 轮询）
pub(crate) fn with_file_lock<T>(
    dir: &Path,
    f: impl FnOnce() -> crate::Result<T>,
) -> crate::Result<T> {
    std::fs::create_dir_all(dir)?;
    let lock_path = dir.join(".vault.lock");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(_) => {
                let result = f();
                let _ = std::fs::remove_file(&lock_path);
                return result;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // 崩溃残留的锁文件：超过 30 秒视为陈旧，强制接管
                if let Ok(meta) = std::fs::metadata(&lock_path)
                    && let Ok(modified) = meta.modified()
                    && modified
                        .elapsed()
                        .map(|d| d.as_secs() > 30)
                        .unwrap_or(false)
                {
                    let _ = std::fs::remove_file(&lock_path);
                    continue;
                }
                if std::time::Instant::now() > deadline {
                    return Err(Error::VaultError("vault 锁等待超时".into()));
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(e) => return Err(e.into()),
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
    let mut key = [0u8; MASTER_LEN];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
    let blob = dpapi::encrypt(&key)?;
    atomic_write(&path, &base64_encode(&blob))?;
    Ok(key)
}

/// 读取已存在的主密钥（V2 导入用；DPAPI 绑定当前用户，换用户会失败）
#[cfg(windows)]
pub fn load_master(dir: &Path) -> crate::Result<[u8; MASTER_LEN]> {
    let path = dir.join("master.key");
    let text = std::fs::read_to_string(&path)?;
    let blob = base64_decode(&text)?;
    let mut key = dpapi::decrypt(&blob)?;
    if key.len() != MASTER_LEN {
        return Err(Error::VaultError(format!(
            "主密钥长度异常: {}（应为 32）",
            key.len()
        )));
    }
    let mut out = [0u8; MASTER_LEN];
    out.copy_from_slice(&key);
    key.zeroize();
    Ok(out)
}

#[cfg(not(windows))]
pub fn load_or_create_master(_dir: &Path) -> crate::Result<[u8; MASTER_LEN]> {
    Err(Error::VaultError(
        "非 Windows 平台暂不支持 DPAPI 主密钥（M4 仅实现 Windows）".into(),
    ))
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
}
