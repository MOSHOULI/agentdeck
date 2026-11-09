//! One explicitly owned, non-credential setting. No agent process or discovery.
//! The receipt backs up only the original boolean/absence, never the document.
//! All other fields are preserved semantically (JSON formatting can change).
use super::{parse, reader::NativeSelectedFileReader};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const RECEIPT: &str = ".agentdeck-qwen-vim.json";
const LOCK: &str = ".agentdeck-qwen-vim.lock";
type Result<T> = std::result::Result<T, EditError>;

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("Unsupported or unsafe explicit file; no configuration values are reported")]
    Unsupported,
    #[error("Preview expired or selected files changed; preview again")]
    Changed,
    #[error("An ownership receipt already exists; restore it before applying another change")]
    AlreadyOwned,
    #[error("Managed field changed externally; refusing to overwrite it")]
    Drift,
    #[error("Another AgentDeck operation is in progress")]
    Busy,
    #[error("Configuration operation failed; inspect the ownership receipt before retrying")]
    Io,
}

#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    pub field: &'static str,
    pub before: Option<bool>,
    pub after: bool,
    pub changes: bool,
    /// Binds the explicit document bytes, manifest and selected value, not a credential.
    pub token: String,
    pub limitations: &'static str,
}

/// Read-only observation, not an authorization token or a guarantee of a later write.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryState {
    NoReceipt,
    RestoreField,
    CleanupReceipt,
    Conflict,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecoveryPreview {
    pub field: &'static str,
    pub state: RecoveryState,
    pub changes_settings: bool,
    pub limitations: &'static str,
}

#[derive(PartialEq)]
enum RecoveryAction {
    RestoreField,
    CleanupReceipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    format: u8,
    phase: Phase,
    selection_binding: String,
    manifest_digest: String,
    restore_digest: Option<String>,
    before: Option<bool>,
    after: bool,
    had_general: bool,
    before_digest: String,
    after_digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Prepared,
    Applied,
    Restoring,
}

struct Selection {
    binding: String,
    bytes: Vec<u8>,
    document: Value,
    manifest: Vec<u8>,
    before: Option<bool>,
    had_general: bool,
}

fn read(path: &Path, name: &str) -> Result<Vec<u8>> {
    NativeSelectedFileReader
        .read_named(path, name)
        .map_err(|_| EditError::Unsupported)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn parse_document(bytes: &[u8]) -> Result<Value> {
    let value = parse::parse_json(bytes).map_err(|_| EditError::Unsupported)?;
    if value
        .as_object()
        .and_then(|v| v.get("$version"))
        .and_then(Value::as_u64)
        != Some(4)
    {
        return Err(EditError::Unsupported);
    }
    field(&value)?;
    Ok(value)
}
fn field(document: &Value) -> Result<Option<bool>> {
    match document.get("general") {
        None => Ok(None),
        Some(Value::Object(general)) => match general.get("vimMode") {
            None => Ok(None),
            Some(Value::Bool(v)) => Ok(Some(*v)),
            _ => Err(EditError::Unsupported),
        },
        _ => Err(EditError::Unsupported),
    }
}
fn selection(settings: &Path, manifest: &Path) -> Result<Selection> {
    // Validate both before reading either.
    super::validate_settings_path(settings).map_err(|_| EditError::Unsupported)?;
    super::validate_runtime_manifest_path(manifest).map_err(|_| EditError::Unsupported)?;
    let mut identity = Sha256::new();
    identity.update(settings.as_os_str().as_encoded_bytes());
    identity.update([0]);
    identity.update(manifest.as_os_str().as_encoded_bytes());
    let binding = format!("{:x}", identity.finalize());
    let manifest = read(manifest, "package.json")?;
    let declaration = parse::parse_json(&manifest).map_err(|_| EditError::Unsupported)?;
    if declaration.get("name").and_then(Value::as_str) != Some("@qwen-code/qwen-code")
        || declaration.get("version").and_then(Value::as_str) != Some("0.27.0")
    {
        return Err(EditError::Unsupported);
    }
    let bytes = read(settings, "settings.json")?;
    let document = parse_document(&bytes)?;
    Ok(Selection {
        binding,
        before: field(&document)?,
        had_general: document.get("general").is_some(),
        bytes,
        document,
        manifest,
    })
}
fn token(selected: &Selection, after: bool) -> String {
    let mut hash = Sha256::new();
    hash.update(b"agentdeck/qwen-vim/1\0");
    hash.update(&selected.binding);
    hash.update(digest(&selected.bytes));
    hash.update(digest(&selected.manifest));
    hash.update([u8::from(after)]);
    format!("{:x}", hash.finalize())
}
pub fn preview(settings: &Path, manifest: &Path, after: bool) -> Result<Preview> {
    let selected = selection(settings, manifest)?;
    Ok(Preview {
        field: "general.vimMode",
        before: selected.before,
        after,
        changes: selected.before != Some(after),
        token: token(&selected, after),
        limitations: "Explicit strict-JSON file only. Manifest declaration is not installation proof. Other scopes can override this value. Formatting may change; unrelated fields remain semantically intact. Stop Qwen and other configuration editors first. Not a cross-process transaction.",
    })
}
fn sibling(settings: &Path, name: &str) -> Result<PathBuf> {
    Ok(settings.parent().ok_or(EditError::Unsupported)?.join(name))
}
fn absent(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err(EditError::Io),
    }
}
fn set_field(document: &mut Value, value: Option<bool>, remove_empty_general: bool) {
    let root = document.as_object_mut().expect("validated object");
    let general = root
        .entry("general")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("validated general");
    match value {
        Some(v) => {
            general.insert("vimMode".into(), Value::Bool(v));
        }
        None => {
            general.remove("vimMode");
        }
    }
    if remove_empty_general && general.is_empty() {
        root.remove("general");
    }
}
fn encode(document: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(document).map_err(|_| EditError::Unsupported)?;
    bytes.push(b'\n');
    // Reapply reader/parser limits before writing a larger serialized result.
    parse_document(&bytes)?;
    if bytes.len() > super::LIMITS.max_manifest_bytes {
        return Err(EditError::Unsupported);
    }
    Ok(bytes)
}

/// Apply only with a token obtained from preview and explicit CLI confirmation.
/// The durable field backup is written before settings. A crash can therefore
/// leave a receipt without a settings change; restore handles that exact state.
pub fn apply(settings: &Path, manifest: &Path, after: bool, expected_token: &str) -> Result<bool> {
    let selected = selection(settings, manifest)?;
    if token(&selected, after) != expected_token {
        return Err(EditError::Changed);
    }
    if selected.before == Some(after) {
        return Ok(false);
    }
    let _lock = lock(settings)?;
    let receipt_path = sibling(settings, RECEIPT)?;
    if !absent(&receipt_path)? {
        return Err(EditError::AlreadyOwned);
    }
    let current = selection(settings, manifest)?;
    if token(&current, after) != expected_token {
        return Err(EditError::Changed);
    }
    let mut updated = current.document.clone();
    set_field(&mut updated, Some(after), false);
    let output = encode(&updated)?;
    let mut receipt = Receipt {
        format: 1,
        phase: Phase::Prepared,
        selection_binding: current.binding.clone(),
        manifest_digest: digest(&current.manifest),
        restore_digest: None,
        before: current.before,
        after,
        had_general: current.had_general,
        before_digest: digest(&current.bytes),
        after_digest: digest(&output),
    };
    let receipt_bytes = serde_json::to_vec(&receipt).map_err(|_| EditError::Io)?;
    // create_new never replaces someone else's receipt.
    create_private(&receipt_path, &receipt_bytes)?;
    if read(settings, "settings.json")? != current.bytes
        || read(manifest, "package.json")? != current.manifest
    {
        return Err(EditError::Changed);
    }
    replace(settings, &output)?;
    if read(settings, "settings.json")? != output {
        return Err(EditError::Changed);
    }
    receipt.phase = Phase::Applied;
    replace_receipt(
        &receipt_path,
        &serde_json::to_vec(&receipt).map_err(|_| EditError::Io)?,
    )?;
    Ok(true)
}

fn parse_receipt(bytes: &[u8]) -> Result<Receipt> {
    let value = parse::parse_json(bytes).map_err(|_| EditError::Unsupported)?;
    serde_json::from_value(value).map_err(|_| EditError::Unsupported)
}

// Shared with the mutating restore path: an observation never relaxes its checks.
fn recovery_action(selected: &Selection, receipt: &Receipt) -> Result<RecoveryAction> {
    if receipt.format != 1
        || receipt.selection_binding != selected.binding
        || receipt.manifest_digest != digest(&selected.manifest)
        || receipt.before == Some(receipt.after)
        || (!receipt.had_general && receipt.before.is_some())
        || ![&receipt.before_digest, &receipt.after_digest]
            .iter()
            .all(|d| d.len() == 64 && d.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(EditError::Unsupported);
    }
    if receipt.phase == Phase::Restoring {
        let expected = receipt
            .restore_digest
            .as_ref()
            .ok_or(EditError::Unsupported)?;
        if digest(&selected.bytes) == *expected {
            return Ok(RecoveryAction::CleanupReceipt);
        }
        // A crash before restore replacement can be retried only against the
        // precise document we prepared, not any externally edited document.
        if digest(&selected.bytes) != receipt.after_digest {
            return Err(EditError::Drift);
        }
    }
    // Receipt persisted but replacement never happened (or exact original restored).
    if digest(&selected.bytes) == receipt.before_digest && selected.before == receipt.before {
        return Ok(RecoveryAction::CleanupReceipt);
    }
    if receipt.phase == Phase::Prepared && digest(&selected.bytes) != receipt.after_digest {
        return Err(EditError::Drift);
    }
    if selected.before != Some(receipt.after) {
        return Err(EditError::Drift);
    }
    Ok(RecoveryAction::RestoreField)
}

/// Inspect only explicit selected files and their receipt, without creating a lock.
/// Missing receipts are observations; malformed/unsafe files remain errors.
/// Files may change concurrently: restore always rereads and validates under its lock.
pub fn recovery_preview(settings: &Path, manifest: &Path) -> Result<RecoveryPreview> {
    let selected = selection(settings, manifest)?;
    let receipt_path = sibling(settings, RECEIPT)?;
    let receipt_bytes = if absent(&receipt_path)? {
        None
    } else {
        Some(read(&receipt_path, RECEIPT)?)
    };
    let state = if let Some(bytes) = &receipt_bytes {
        let receipt = parse_receipt(bytes)?;
        match recovery_action(&selected, &receipt) {
            Ok(RecoveryAction::RestoreField) => RecoveryState::RestoreField,
            Ok(RecoveryAction::CleanupReceipt) => RecoveryState::CleanupReceipt,
            Err(EditError::Drift) => RecoveryState::Conflict,
            Err(error) => return Err(error),
        }
    } else {
        RecoveryState::NoReceipt
    };
    // Best-effort coherence only, not a lock or atomic snapshot.
    if read(settings, "settings.json")? != selected.bytes
        || read(manifest, "package.json")? != selected.manifest
    {
        return Err(EditError::Changed);
    }
    match receipt_bytes {
        Some(bytes) if read(&receipt_path, RECEIPT)? != bytes => return Err(EditError::Changed),
        None if !absent(&receipt_path)? => return Err(EditError::Changed),
        _ => {}
    }
    Ok(RecoveryPreview {
        field: "general.vimMode",
        changes_settings: state == RecoveryState::RestoreField,
        state,
        limitations: "Read-only observation, not a restore authorization or write-readiness check. No lock is acquired or created. Concurrent changes can invalidate this result; restore checks again. Stop other editors before restoring. Not a cross-process transaction.",
    })
}

/// Restore only the backed-up field. Unrelated external edits are preserved.
/// The receipt remains on any error; successful restoration removes it.
pub fn restore(settings: &Path, manifest: &Path) -> Result<bool> {
    let _initial = selection(settings, manifest)?;
    let _lock = lock(settings)?;
    let selected = selection(settings, manifest)?;
    let receipt_path = sibling(settings, RECEIPT)?;
    let receipt_bytes = read(&receipt_path, RECEIPT)?;
    let mut receipt = parse_receipt(&receipt_bytes)?;
    if recovery_action(&selected, &receipt)? == RecoveryAction::CleanupReceipt {
        fs::remove_file(receipt_path).map_err(|_| EditError::Io)?;
        return Ok(false);
    }
    let mut updated = selected.document;
    set_field(&mut updated, receipt.before, !receipt.had_general);
    let output = encode(&updated)?;
    if read(settings, "settings.json")? != selected.bytes
        || read(&receipt_path, RECEIPT)? != receipt_bytes
        || read(manifest, "package.json")? != selected.manifest
    {
        return Err(EditError::Changed);
    }
    receipt.phase = Phase::Restoring;
    receipt.after_digest = digest(&selected.bytes);
    receipt.restore_digest = Some(digest(&output));
    replace_receipt(
        &receipt_path,
        &serde_json::to_vec(&receipt).map_err(|_| EditError::Io)?,
    )?;
    if read(settings, "settings.json")? != selected.bytes {
        return Err(EditError::Changed);
    }
    replace(settings, &output)?;
    if read(settings, "settings.json")? != output {
        return Err(EditError::Changed);
    }
    fs::remove_file(receipt_path).map_err(|_| EditError::Io)?;
    Ok(true)
}

fn private_file(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::{Foundation::GENERIC_WRITE, Storage::FileSystem::WRITE_DAC};
        options.access_mode(GENERIC_WRITE | WRITE_DAC).share_mode(0);
    }
    options.open(path).map_err(|_| EditError::Io)
}
fn create_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = private_file(path)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| EditError::Io)
}
fn lock(settings: &Path) -> Result<fs::File> {
    let path = sibling(settings, LOCK)?;
    if absent(&path)? {
        // Concurrent creation is fine; the checked read below determines safety.
        let _ = create_private(&path, b"");
    }
    read(&path, LOCK)?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0o400000 | 0o4000);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(&path).map_err(|_| EditError::Io)?;
    file.try_lock().map_err(|_| EditError::Busy)?;
    Ok(file)
}
fn replace_receipt(path: &Path, bytes: &[u8]) -> Result<()> {
    replace_named(path, bytes, RECEIPT)
}
fn replace(path: &Path, bytes: &[u8]) -> Result<()> {
    replace_named(path, bytes, "settings.json")
}
fn replace_named(path: &Path, bytes: &[u8], name: &str) -> Result<()> {
    // Revalidate target/ancestors and regular-file restrictions immediately
    // before replacement. This is not an atomic compare-and-swap against Qwen.
    read(path, name)?;
    if fs::metadata(path)
        .map_err(|_| EditError::Io)?
        .permissions()
        .readonly()
    {
        return Err(EditError::Unsupported);
    }
    let temporary = sibling(
        path,
        &format!(".agentdeck-vim-{}.tmp", uuid::Uuid::new_v4()),
    )?;
    let mut file = private_file(&temporary)?;
    let mut write_temp = || -> Result<()> {
        #[cfg(windows)]
        copy_dacl(path, &file)?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| EditError::Io)
    };
    if let Err(error) = write_temp() {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    #[cfg(unix)]
    let result = (|| {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let source = fs::metadata(path)?;
        let target = fs::metadata(&temporary)?;
        if source.uid() != target.uid()
            || source.gid() != target.gid()
            || source.permissions().mode() & 0o7000 != 0
            || !linux_has_no_acl(path)
            || !linux_has_no_acl(&temporary)
        {
            return Err(std::io::Error::other("unsupported permission metadata"));
        }
        fs::set_permissions(&temporary, source.permissions())?;
        fs::rename(&temporary, path)
    })();
    #[cfg(windows)]
    let result = {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
        let wide = |p: &Path| {
            p.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        // ReplaceFile preserves the destination ACL rather than inheriting the
        // temp file's ACL. No fallback that weakens access control is allowed.
        let ok = unsafe {
            ReplaceFileW(
                wide(path).as_ptr(),
                wide(&temporary).as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    };
    #[cfg(not(any(unix, windows)))]
    let result: std::io::Result<()> = Err(std::io::Error::other("unsupported platform"));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|_| EditError::Io)
}

#[cfg(unix)]
fn linux_has_no_acl(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        unsafe extern "C" {
            fn getxattr(
                path: *const std::ffi::c_char,
                name: *const std::ffi::c_char,
                value: *mut std::ffi::c_void,
                size: usize,
            ) -> isize;
        }
        let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // Query only whether an ACL exists, never read its contents. Refuse both
        // access ACLs and inherited default ACLs on temporary files.
        let result = unsafe {
            getxattr(
                path.as_ptr(),
                c"system.posix_acl_access".as_ptr(),
                std::ptr::null_mut(),
                0,
            )
        };
        result == -1
            && matches!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(61 | 95)
            )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        false
    }
}

#[cfg(windows)]
fn copy_dacl(source: &Path, destination: &fs::File) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, GetFileSecurityW, PROTECTED_DACL_SECURITY_INFORMATION,
        SetKernelObjectSecurity,
    };
    let wide = |p: &Path| {
        p.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let mut needed = 0;
    unsafe {
        GetFileSecurityW(
            wide(source).as_ptr(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    if needed == 0 || needed > 1024 * 1024 {
        return Err(EditError::Io);
    }
    // Aligned storage for the self-relative SECURITY_DESCRIPTOR.
    let mut descriptor = vec![0u32; (needed as usize).div_ceil(4)];
    let success = unsafe {
        GetFileSecurityW(
            wide(source).as_ptr(),
            DACL_SECURITY_INFORMATION,
            descriptor.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    };
    if success == 0 {
        return Err(EditError::Io);
    }
    // Copy effective DACL and protect from broader inherited directory grants
    // BEFORE writing any configuration bytes into the temporary file.
    let success = unsafe {
        SetKernelObjectSecurity(
            std::os::windows::io::AsRawHandle::as_raw_handle(destination) as _,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor.as_mut_ptr().cast(),
        )
    };
    if success == 0 {
        return Err(EditError::Io);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
