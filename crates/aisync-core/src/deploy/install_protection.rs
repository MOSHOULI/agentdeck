//! Persistent, process-coordinated Codex probe suppression. No package execution.
//! Recovery releases suppression only; it never removes or authenticates a package.
use super::InstallPreview;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
mod native;

const ERROR: &str = "安装保护状态不可用或已变化；继续暂停 Codex 探测，未清理或执行程序。";
const LIMIT: u64 = 16 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WritePoint {
    TemporarySynced,
    Replaced,
    DirectorySynced,
}
const LIMITATIONS: &str = "只读检查记录的安装槽和当前 PATH 候选；不验证软件真伪，不清理、不重试、不执行程序。确认仅恢复此 Codex 条目的正常探测，之后刷新可能执行其版本命令。同一 HOME 的 AgentDeck 实例由共享文件锁协调；不受此门禁协调的外部进程和恶意同用户文件竞态不在保证范围。";
const UNCERTAIN: &str = "安装调用未确认返回，可能仍有安装进程运行；即使槽为空也不能在应用内恢复。保留状态并先人工检查进程；不清理、不执行程序。";
type Result<T> = std::result::Result<T, String>;
fn err() -> String {
    ERROR.into()
}
fn platform() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u8,
    platform: String,
    phase: Phase,
    attempt: Option<String>,
    prefix: Option<String>,
    #[serde(rename = "prefixIdentity")]
    prefix_identity: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Clear,
    Armed,
    FailedReturned,
}
impl Record {
    fn clear() -> Self {
        Self {
            schema: 1,
            platform: platform().into(),
            phase: Phase::Clear,
            attempt: None,
            prefix: None,
            prefix_identity: None,
        }
    }
    fn validate(&self) -> Result<()> {
        if self.schema != 1 || self.platform != platform() {
            return Err(err());
        }
        if self.phase == Phase::Clear {
            if self.attempt.is_some() || self.prefix.is_some() || self.prefix_identity.is_some() {
                return Err(err());
            }
        } else {
            uuid::Uuid::parse_str(self.attempt.as_deref().ok_or_else(err)?).map_err(|_| err())?;
            normalize(Path::new(self.prefix.as_deref().ok_or_else(err)?))?;
            if self
                .prefix_identity
                .as_ref()
                .is_none_or(|v| v.is_empty() || v.len() > 1024)
            {
                return Err(err());
            }
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct Attempt {
    id: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPreview {
    pub schema_version: u8,
    pub agent_id: &'static str,
    pub state: &'static str,
    pub prefix: Option<String>,
    pub slots: Vec<RecoverySlot>,
    pub revision: Option<String>,
    pub runtime_not_run: bool,
    pub cleanup_supported: bool,
    pub limitations: &'static str,
}
#[derive(Debug, Serialize)]
pub struct RecoverySlot {
    pub slot: &'static str,
    pub state: &'static str,
}

struct HeldDirectory {
    path: PathBuf,
    file: File,
    identity: String,
}
fn directories(path: &Path) -> Result<Vec<HeldDirectory>> {
    let path = normalize(path)?;
    let mut held = Vec::new();
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part.as_os_str());
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        let file = native::open_dir(&current)?;
        let identity = native::identity(&file)?;
        held.push(HeldDirectory {
            path: current.clone(),
            file,
            identity,
        });
    }
    verify_directories(&held)?;
    Ok(held)
}
fn verify_directories(held: &[HeldDirectory]) -> Result<()> {
    for d in held {
        if native::identity(&d.file)? != d.identity
            || native::identity(&native::open_dir(&d.path)?)? != d.identity
        {
            return Err(err());
        }
    }
    Ok(())
}
fn normalize(path: &Path) -> Result<PathBuf> {
    let text = path.to_str().ok_or_else(err)?;
    #[cfg(windows)]
    let text = match text.strip_prefix(r"\\?\") {
        Some(rest)
            if rest.as_bytes().get(1..3) == Some(b":\\")
                && rest.as_bytes()[0].is_ascii_alphabetic() =>
        {
            rest
        }
        Some(_) => return Err(err()),
        None => text,
    };
    if text.len()>4096 || text.trim()!=text || text.chars().any(|c| c.is_control() || matches!(c,'\u{200b}'..='\u{200f}'|'\u{2028}'..='\u{202e}'|'\u{2060}'..='\u{206f}'|'\u{feff}')) { return Err(err()); }
    let bad_component = if cfg!(windows) {
        text.split(['\\', '/']).any(|s| s == "." || s == "..")
    } else {
        text.split('/').any(|s| s == "." || s == "..")
    };
    if bad_component {
        return Err(err());
    }
    let path = PathBuf::from(text);
    #[cfg(windows)]
    native::local_path(&path)?;
    if !path.is_absolute() || path.components().count() > 64 {
        return Err(err());
    }
    for part in path.components() {
        match part {
            Component::ParentDir | Component::CurDir => return Err(err()),
            Component::Prefix(p) => {
                #[cfg(windows)]
                if !matches!(p.kind(), std::path::Prefix::Disk(_)) {
                    return Err(err());
                }
                #[cfg(not(windows))]
                let _ = p;
            }
            Component::Normal(n) => {
                #[cfg(windows)]
                {
                    let n = n.to_str().ok_or_else(err)?;
                    let base = n.split('.').next().unwrap_or("").to_ascii_uppercase();
                    if n.ends_with([' ', '.'])
                        || n.chars().any(|c| r#"<>:\"|?*"#.contains(c))
                        || matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                        || (base.len() == 4
                            && (base.starts_with("COM") || base.starts_with("LPT"))
                            && base.as_bytes()[3].is_ascii_digit())
                    {
                        return Err(err());
                    }
                }
                #[cfg(not(windows))]
                let _ = n;
            }
            _ => {}
        }
    }
    Ok(path)
}

/// Owns the nonblocking OS coordination lock. Hold across the entire active operation.
/// The fixed lock is never unlinked. Reads of established state never repair it.
struct LockedFile(File);
impl std::ops::Deref for LockedFile {
    type Target = File;
    fn deref(&self) -> &File {
        &self.0
    }
}
impl Drop for LockedFile {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
pub struct Store {
    directory: PathBuf,
    held: Vec<HeldDirectory>,
    lock: LockedFile,
    lock_identity: String,
    record: Record,
    bytes: Vec<u8>,
    state_identity: String,
}
impl Store {
    pub fn open(home: &Path) -> Result<Self> {
        Self::open_impl(home, true)
    }
    /// Recovery reads never initialize, repair, or create missing state.
    pub fn open_existing(home: &Path) -> Result<Self> {
        Self::open_impl(home, false)
    }
    fn open_impl(home: &Path, initialize: bool) -> Result<Self> {
        if !cfg!(any(target_os = "linux", windows)) {
            return Err(
                "此平台尚不支持持久安装保护；桌面 Codex 探测和安装保持禁用，未执行程序。".into(),
            );
        }
        let home = normalize(home)?;
        let _home = directories(&home)?;
        let mut path = home;
        for name in [".agentdeck", "install-protection"] {
            path.push(name);
            match fs::symlink_metadata(&path) {
                Err(e) if initialize && e.kind() == std::io::ErrorKind::NotFound => {
                    match native::create_dir(&path) {
                        Ok(()) => {}
                        Err(_) if path.exists() => {}
                        Err(e) => return Err(e),
                    }
                }
                Err(_) => return Err(err()),
                Ok(_) => {}
            }
            let file = native::open_dir(&path)?;
            if name == "install-protection" {
                native::private(&file, true)?;
            }
        }
        path.push("codex");
        let created = if initialize {
            match native::create_dir(&path) {
                Ok(()) => true,
                Err(_) => {
                    native::open_dir(&path)?;
                    false
                }
            }
        } else {
            native::open_dir(&path)?;
            false
        };
        let held = directories(&path)?;
        native::private(&held.last().ok_or_else(err)?.file, true)?;
        if created {
            let lock = native::create_file(&path.join("gate.lock"))?;
            lock.sync_all().map_err(|_| err())?;
            let mut state = native::create_file(&path.join("state.json"))?;
            let bytes = serde_json::to_vec(&Record::clear()).map_err(|_| err())?;
            state.write_all(&bytes).map_err(|_| err())?;
            state.sync_all().map_err(|_| err())?;
            native::sync_dir(&held.last().ok_or_else(err)?.file)?;
        }
        let lock = native::open_file(&path.join("gate.lock"), true)?;
        native::private(&lock, false)?;
        if lock.metadata().map_err(|_| err())?.len() != 0 {
            return Err(err());
        }
        lock.try_lock().map_err(|_| err())?;
        let lock = LockedFile(lock);
        let lock_identity = native::identity(&lock)?;
        if native::identity(&native::open_file(&path.join("gate.lock"), false)?)? != lock_identity {
            let _ = lock.unlock();
            return Err(err());
        }
        let (record, bytes, state_identity) = read_record(&path)?;
        let store = Self {
            directory: path,
            held,
            lock,
            lock_identity,
            record,
            bytes,
            state_identity,
        };
        store.check()?;
        Ok(store)
    }
    pub fn is_clear(&self) -> bool {
        self.record.phase == Phase::Clear && self.check().is_ok()
    }
    fn check(&self) -> Result<()> {
        verify_directories(&self.held)?;
        native::private(&self.held.last().ok_or_else(err)?.file, true)?;
        let lock = native::open_file(&self.directory.join("gate.lock"), false)?;
        native::private(&lock, false)?;
        if native::identity(&self.lock)? != self.lock_identity
            || native::identity(&lock)? != self.lock_identity
            || lock.metadata().map_err(|_| err())?.len() != 0
        {
            return Err(err());
        }
        let (_, bytes, identity) = read_record(&self.directory)?;
        if bytes != self.bytes || identity != self.state_identity {
            return Err(err());
        }
        Ok(())
    }
    fn replace(&mut self, record: Record) -> Result<()> {
        self.replace_with(record, |_| Ok(()))
    }
    fn replace_with(
        &mut self,
        record: Record,
        mut hook: impl FnMut(WritePoint) -> Result<()>,
    ) -> Result<()> {
        self.check()?;
        record.validate()?;
        let bytes = serde_json::to_vec(&record).map_err(|_| err())?;
        let temporary = self
            .directory
            .join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let mut file = native::create_file(&temporary)?;
        file.write_all(&bytes).map_err(|_| err())?;
        file.sync_all().map_err(|_| err())?;
        hook(WritePoint::TemporarySynced)?;
        native::private(&file, false)?;
        let expected = native::identity(&file)?;
        drop(file);
        self.check()?;
        fs::rename(&temporary, self.directory.join("state.json")).map_err(|_| err())?;
        // Errors from here are uncertain commits: disk may already hold new state.
        // This lease stays stale/fail-closed; a new process validates actual bytes.
        hook(WritePoint::Replaced)?;
        native::sync_dir(&self.held.last().ok_or_else(err)?.file)?;
        hook(WritePoint::DirectorySynced)?;
        let (read, actual, identity) = read_record(&self.directory)?;
        if read != record || actual != bytes || identity != expected {
            return Err(err());
        }
        self.record = record;
        self.bytes = bytes;
        self.state_identity = identity;
        self.check()
    }
    pub fn begin(&mut self, preview: &InstallPreview) -> Result<Attempt> {
        self.check()?;
        if self.record.phase != Phase::Clear
            || preview.schema_version != 1
            || preview.script_policy != "disabled"
            || preview.agent_id != "codex"
            || preview.package != "@openai/codex"
            || preview.executable != "codex"
            || preview.version_policy != "exact_root"
            || preview.package_version.is_none()
        {
            return Err(err());
        }
        let prefix = normalize(Path::new(&preview.prefix))?;
        let root = prefix.join(if cfg!(windows) {
            "node_modules"
        } else {
            "lib/node_modules"
        });
        let bin = if cfg!(windows) {
            prefix.clone()
        } else {
            prefix.join("bin")
        };
        if normalize(Path::new(&preview.global_root))? != root
            || normalize(Path::new(&preview.package_root))? != root.join("@openai/codex")
            || normalize(Path::new(&preview.launcher_directory))? != bin
        {
            return Err(err());
        }
        let held = directories(&prefix)?;
        let identity = held.last().ok_or_else(err)?.identity.clone();
        let id = uuid::Uuid::new_v4().to_string();
        let record = Record {
            schema: 1,
            platform: platform().into(),
            phase: Phase::Armed,
            attempt: Some(id.clone()),
            prefix: Some(prefix.to_str().ok_or_else(err)?.into()),
            prefix_identity: Some(identity),
        };
        let (slots, _, eligible) = observe_slots(&record)?;
        if !eligible || slots.iter().any(|s| s.state != "missing") {
            return Err(err());
        }
        verify_directories(&held)?;
        self.replace(record)?;
        Ok(Attempt { id })
    }
    pub fn mark_failed(&mut self, attempt: &Attempt) -> Result<()> {
        self.check_attempt(attempt)?;
        let mut record = self.record.clone();
        record.phase = Phase::FailedReturned;
        self.replace(record)
    }
    pub fn finish_success(&mut self, attempt: &Attempt) -> Result<()> {
        self.check_attempt(attempt)?;
        self.replace(Record::clear())
    }
    fn check_attempt(&self, attempt: &Attempt) -> Result<()> {
        self.check()?;
        if self.record.phase != Phase::Armed || self.record.attempt.as_deref() != Some(&attempt.id)
        {
            return Err(err());
        }
        Ok(())
    }
    pub fn recovery_preview(&self) -> Result<RecoveryPreview> {
        self.check()?;
        let mut result = RecoveryPreview {
            schema_version: 1,
            agent_id: "codex",
            state: "blocked",
            prefix: self.record.prefix.clone(),
            slots: Vec::new(),
            revision: None,
            runtime_not_run: true,
            cleanup_supported: false,
            limitations: LIMITATIONS,
        };
        if self.record.phase == Phase::Clear {
            result.state = "clear";
            return Ok(result);
        }
        if self.record.phase == Phase::Armed {
            result.limitations = UNCERTAIN;
            return Ok(result);
        }
        let (slots, binding, absent) = observe_slots(&self.record)?;
        result.slots = slots;
        if absent && let Ok(path_binding) = path_absence() {
            let mut token = self.bytes.clone();
            token.extend_from_slice(self.state_identity.as_bytes());
            token.extend_from_slice(binding.as_bytes());
            token.extend_from_slice(path_binding.as_bytes());
            result.state = "eligible";
            result.revision = Some(hash(&token));
        }
        self.check()?;
        Ok(result)
    }
    pub fn confirm_recovery(&mut self, revision: &str) -> Result<()> {
        if revision.len() != 64 {
            return Err(err());
        }
        let current = self.recovery_preview()?;
        if current.state != "eligible" || current.revision.as_deref() != Some(revision) {
            return Err(err());
        }
        self.replace(Record::clear())
    }
}
fn file_stamp(file: &File) -> Result<(u64, std::time::SystemTime)> {
    let m = file.metadata().map_err(|_| err())?;
    Ok((m.len(), m.modified().map_err(|_| err())?))
}
fn read_record(directory: &Path) -> Result<(Record, Vec<u8>, String)> {
    let mut file = native::open_file(&directory.join("state.json"), false)?;
    native::private(&file, false)?;
    let identity = native::identity(&file)?;
    let before = file_stamp(&file)?;
    if file.metadata().map_err(|_| err())?.len() > LIMIT {
        return Err(err());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| err())?;
    if bytes.len() as u64 > LIMIT
        || identity != native::identity(&file)?
        || before != file_stamp(&file)?
    {
        return Err(err());
    }
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| err())?;
    record.validate()?;
    let named = native::open_file(&directory.join("state.json"), false)?;
    if native::identity(&named)? != identity || file_stamp(&named)? != before {
        return Err(err());
    }
    Ok((record, bytes, identity))
}
fn slot_paths(prefix: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut slots = vec![(
        "package_directory",
        prefix.join(if cfg!(windows) {
            "node_modules/@openai/codex"
        } else {
            "lib/node_modules/@openai/codex"
        }),
    )];
    let bin = if cfg!(windows) {
        prefix.to_owned()
    } else {
        prefix.join("bin")
    };
    slots.push(("posix_launcher", bin.join("codex")));
    if cfg!(windows) {
        for (name, suffix) in [
            ("cmd_launcher", "cmd"),
            ("powershell_launcher", "ps1"),
            ("exe_launcher", "exe"),
            ("com_launcher", "com"),
            ("bat_launcher", "bat"),
        ] {
            slots.push((name, bin.join(format!("codex.{suffix}"))));
        }
    }
    slots
}
fn observe_slots(record: &Record) -> Result<(Vec<RecoverySlot>, String, bool)> {
    let prefix = normalize(Path::new(record.prefix.as_deref().ok_or_else(err)?))?;
    let held = directories(&prefix)?;
    if held.last().ok_or_else(err)?.identity != *record.prefix_identity.as_ref().ok_or_else(err)? {
        return Err(err());
    }
    let mut binding = String::new();
    let mut slots = Vec::new();
    let mut absent = true;
    for (slot, path) in slot_paths(&prefix) {
        let state = match observe_absence(&path, &mut binding) {
            Ok(true) => "missing",
            Ok(false) => "present",
            Err(_) => "blocked",
        };
        absent &= state == "missing";
        slots.push(RecoverySlot { slot, state });
    }
    verify_directories(&held)?;
    Ok((slots, hash(binding.as_bytes()), absent))
}
/// Strict component walk: no symlink, reparse, unreadable or non-directory parent
/// can be converted into absence. Existing directory identity binds the preview.
fn observe_absence(path: &Path, binding: &mut String) -> Result<bool> {
    let path = normalize(path)?;
    let mut current = PathBuf::new();
    let mut missing = false;
    for part in path.components() {
        current.push(part.as_os_str());
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        if missing {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                binding.push_str(current.to_str().ok_or_else(err)?);
                binding.push_str("=missing\n");
                missing = true;
            }
            Err(_) => return Err(err()),
            Ok(_) if current == path => return Ok(false),
            Ok(_) => {
                let file = native::open_dir(&current)?;
                binding.push_str(current.to_str().ok_or_else(err)?);
                binding.push('=');
                binding.push_str(&native::identity(&file)?);
                binding.push('\n');
            }
        }
    }
    Ok(missing)
}
fn path_absence() -> Result<String> {
    let path = std::env::var_os("PATH").ok_or_else(err)?;
    if path.as_encoded_bytes().len() > 64 * 1024 {
        return Err(err());
    }
    let cwd = std::env::current_dir().map_err(|_| err())?;
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        dirs.push(cwd.clone());
    }
    dirs.extend(
        std::env::split_paths(&path).map(|p| if p.is_absolute() { p } else { cwd.join(p) }),
    );
    if dirs.len() > 512 {
        return Err(err());
    }
    let mut suffixes = vec![String::new()];
    if cfg!(windows) {
        let ext = std::env::var("PATHEXT").map_err(|_| err())?;
        if ext.len() > 1024 {
            return Err(err());
        }
        for suffix in ext.split(';') {
            if !suffix.starts_with('.')
                || suffix.len() < 2
                || !suffix[1..].bytes().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(err());
            }
            suffixes.push(suffix.to_ascii_lowercase());
        }
        for suffix in [".cmd", ".ps1", ".exe", ".com", ".bat"] {
            if !suffixes.iter().any(|s| s == suffix) {
                suffixes.push(suffix.into());
            }
        }
    }
    let mut binding = format!("{:?}\n{:?}\n", path, cwd);
    for dir in dirs {
        // PATH may legitimately contain symlinked directories such as /bin.
        // Resolve read-only, bind both spelling and canonical directory identity,
        // then inspect fixed candidates without executing them.
        let resolved = match fs::canonicalize(&dir) {
            Ok(p) => normalize(&p)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => dir.clone(),
            Err(_) => return Err(err()),
        };
        binding.push_str(&format!("{dir:?}=>{resolved:?}\n"));
        for suffix in &suffixes {
            if !observe_absence(&resolved.join(format!("codex{suffix}")), &mut binding)? {
                return Err(err());
            }
        }
    }
    Ok(hash(binding.as_bytes()))
}
#[cfg(test)]
mod tests;
