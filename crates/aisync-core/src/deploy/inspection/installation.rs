//! Install-only npm adapter. Observations never authorize a reinstall, upgrade,
//! uninstall, or rollback. A plan is ephemeral and must be rebuilt before use.
use super::*;
use crate::registry::InstallMethod;
use sha2::{Digest, Sha256};

const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);
const MANAGER_FILE_LIMIT: usize = 256 * 1024 * 1024;

// Authoritative execution policy, independent of the registry recipe used for
// adapter eligibility. Keep bundled/retained Codex recipes supported while
// enforcing the safety override in both preview and execution.
fn disables_lifecycle_scripts(package: &str) -> bool {
    matches!(package, "@openai/codex" | "@earendil-works/pi-coding-agent")
}

fn adapter(agent: &AgentDefinition) -> Option<KnownAgent> {
    // Upstream deprecated npm installation. Preserved runtime registries must
    // not re-enable this channel; read-only legacy inspection remains separate.
    if agent.id == "claude-code" {
        return None;
    }
    let known = known_agent(agent)?;
    let package = known.package?;
    let recipe = if agent.id == "pi" {
        format!("npm install -g --ignore-scripts {package}")
    } else {
        format!("npm install -g {package}")
    };
    (agent.install.method == InstallMethod::Npm
        && agent.install.command.as_deref().map(str::trim) == Some(recipe.as_str()))
    .then_some(known)
}

pub(crate) fn can_install(agent: &AgentDefinition) -> bool {
    adapter(agent).is_some()
}

// Preview and both install entrypoints share the application's installer lock.
// This does not exclude other processes or same-user filesystem changes.
static INSTALL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn locked<T>(action: impl FnOnce() -> Result<T, String>) -> crate::Result<T> {
    let _guard = INSTALL_LOCK.try_lock().map_err(|_| {
        crate::Error::InstallFailed("Another installation is already running".into())
    })?;
    action().map_err(crate::Error::InstallFailed)
}

fn inputs(agent: &AgentDefinition) -> Result<(KnownAgent, SearchPath), String> {
    let known = adapter(agent).ok_or(
        "No supported safe installation adapter; use the original official installation method",
    )?;
    let search =
        SearchPath::from_environment().map_err(|_| "Cannot inspect the executable search path")?;
    Ok((known, search))
}

/// A reviewable observation, not a permission token or a zero-side-effect sandbox.
/// npm queries execute local code. Only the Codex root version is pinned.
/// Registry, dependency closure, npm environment/configuration and script
/// downloads are not pinned by this revision.
/// nodePath is the installer interpreter, not every future launcher interpreter.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPreview {
    pub schema_version: u32,
    pub agent_id: String,
    pub package: String,
    pub package_version: Option<String>,
    pub executable: String,
    pub manager_path: String,
    pub node_path: String,
    pub installer_node_version: String,
    pub installer_node_minimum: String,
    pub prefix: String,
    pub global_root: String,
    pub package_root: String,
    pub launcher_directory: String,
    pub script_policy: String,
    pub version_policy: String,
    pub revision: String,
}

pub(crate) fn preview_install(agent: &AgentDefinition) -> crate::Result<InstallPreview> {
    locked(|| {
        let (known, search) = inputs(agent)?;
        let plan = build_plan(known, &search, &mut ProcessRunner)?;
        project_plan(&agent.id, known, search.platform, &plan)
    })
}

pub(crate) fn install_agent(agent: &AgentDefinition) -> crate::Result<()> {
    locked(|| {
        let (known, search) = inputs(agent)?;
        install_with(known, &search, &mut ProcessRunner, |command| {
            run_bounded(command, INSTALL_TIMEOUT, 1024 * 1024).map(|_| ())
        })
    })
}

pub(crate) fn install_agent_from_preview(
    agent: &AgentDefinition,
    revision: &str,
) -> crate::Result<()> {
    install_agent_from_preview_observed(agent, revision, || {})
}

pub(crate) fn install_agent_from_preview_observed(
    agent: &AgentDefinition,
    revision: &str,
    on_installer_start: impl FnOnce(),
) -> crate::Result<()> {
    locked(|| {
        let (known, search) = inputs(agent)?;
        install_reviewed_observed_with(
            &agent.id,
            revision,
            known,
            &search,
            &mut ProcessRunner,
            on_installer_start,
            |command| run_bounded(command, INSTALL_TIMEOUT, 1024 * 1024).map(|_| ()),
        )
    })
}

#[derive(Debug, PartialEq, Eq)]
struct Manager {
    command: ProbeCommand,
    cli: PathBuf,
    // Detect ordinary replacement of either frozen program before invoking it.
    // npm's module tree and concurrent same-user edits are not sandboxed.
    program_digest: Vec<u8>,
    cli_digest: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct InstallPlan {
    manager: Manager,
    prefix: PathBuf,
    root: PathBuf,
    package: &'static str,
    package_version: Option<String>,
    installer_node_version: NodeVersion,
    installer_node_minimum: NodeVersion,
}

// Do not silently replace non-Unicode bytes/surrogates or display invisible
// controls. Preserve exact spelling, including Windows verbatim prefixes.
fn display_value(value: &std::ffi::OsStr) -> Result<String, String> {
    let value = value
        .to_str()
        .ok_or("Installation path is not representable as Unicode")?;
    if value.is_empty() || value.len() > 32768 || value.trim() != value
        || value.chars().any(|c| c.is_control() || matches!(c,
            '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')) {
        return Err("Installation path contains ambiguous, control or overlong text".into());
    }
    Ok(value.to_owned())
}

fn project_plan(
    id: &str,
    known: KnownAgent,
    platform: Platform,
    plan: &InstallPlan,
) -> Result<InstallPreview, String> {
    let mut preview = InstallPreview {
        schema_version: 1,
        agent_id: display_value(id.as_ref())?,
        package: display_value(plan.package.as_ref())?,
        package_version: plan.package_version.clone(),
        executable: display_value(known.executable.as_ref())?,
        manager_path: display_value(plan.manager.cli.as_os_str())?,
        node_path: display_value(plan.manager.command.program.as_os_str())?,
        installer_node_version: format!("v{}", plan.installer_node_version),
        installer_node_minimum: plan.installer_node_minimum.to_string(),
        prefix: display_value(plan.prefix.as_os_str())?,
        global_root: display_value(plan.root.as_os_str())?,
        package_root: display_value(plan.root.join(plan.package).as_os_str())?,
        launcher_directory: display_value(
            if platform == Platform::Windows {
                plan.prefix.clone()
            } else {
                plan.prefix.join("bin")
            }
            .as_os_str(),
        )?,
        script_policy: if disables_lifecycle_scripts(plan.package) {
            "disabled"
        } else {
            "npm_configured"
        }
        .into(),
        version_policy: if plan.package_version.is_some() {
            "exact_root"
        } else {
            "npm_configured"
        }
        .into(),
        revision: String::new(),
    };
    let mut hash = Sha256::new();
    let mut field = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    };
    field(b"AgentDeck reviewed npm install v1; absent-only; double-preflight; bounded; provenance-postcheck");
    field(if platform == Platform::Windows {
        b"windows"
    } else {
        b"unix"
    });
    field(&serde_json::to_vec(&preview).map_err(|_| "Cannot serialize installation preview")?);
    let command = install_command(plan);
    field(&[u8::from(command.npm_metadata)]);
    field(display_value(command.program.as_os_str())?.as_bytes());
    for arg in &command.args {
        field(display_value(arg)?.as_bytes());
    }
    field(&plan.manager.program_digest);
    field(&plan.manager.cli_digest);
    preview.revision = format!("{:x}", hash.finalize());
    Ok(preview)
}

fn install_reviewed_observed_with(
    id: &str,
    revision: &str,
    known: KnownAgent,
    search: &SearchPath,
    runner: &mut impl Runner,
    on_installer_start: impl FnOnce(),
    install: impl FnOnce(&ProbeCommand) -> io::Result<()>,
) -> Result<(), String> {
    install_reviewed_with(id, revision, known, search, runner, |command| {
        // The observer marks an attempted mutation, not proof of a successful
        // spawn or write. It must run before the potentially mutating callback.
        on_installer_start();
        install(command)
    })
}

fn install_reviewed_with(
    id: &str,
    revision: &str,
    known: KnownAgent,
    search: &SearchPath,
    runner: &mut impl Runner,
    install: impl FnOnce(&ProbeCommand) -> io::Result<()>,
) -> Result<(), String> {
    if revision.len() != 64
        || !revision
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(
            "A valid installation preview revision is required; refresh and review the target"
                .into(),
        );
    }
    let plan = build_plan(known, search, runner)?;
    if project_plan(id, known, search.platform, &plan)?.revision != revision {
        return Err(
            "Installation target changed since confirmation; refresh and review the target".into(),
        );
    }
    execute_plan(plan, known, search, runner, install)
}

fn digest(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = read_small(path, MANAGER_FILE_LIMIT)
        .map_err(|_| "Installer program could not be read within its size limit")?;
    Ok(Sha256::digest(bytes).to_vec())
}

fn require_native_unix_node(path: &Path) -> Result<(), String> {
    let mut magic = [0; 4];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .map_err(|_| "Node executable header could not be inspected")?;
    // ELF, Mach-O and universal Mach-O. A shebang would reintroduce an
    // interpreter not frozen in our invocation. This is not publisher identity.
    if magic != *b"\x7fELF"
        && ![
            [0xfe, 0xed, 0xfa, 0xce],
            [0xfe, 0xed, 0xfa, 0xcf],
            [0xce, 0xfa, 0xed, 0xfe],
            [0xcf, 0xfa, 0xed, 0xfe],
            [0xca, 0xfe, 0xba, 0xbe],
            [0xbe, 0xba, 0xfe, 0xca],
            [0xca, 0xfe, 0xba, 0xbf],
            [0xbf, 0xba, 0xfe, 0xca],
        ]
        .contains(&magic)
    {
        return Err("Node must be a supported native executable, not a script or wrapper".into());
    }
    Ok(())
}

// Fixed AgentDeck installer policy, not a registry-controlled command or a claim
// about the selected package's complete engines/dependency/native requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct NodeVersion(u32, u32, u32);

impl std::fmt::Display for NodeVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

fn installer_node_minimum(package: &str) -> Result<NodeVersion, String> {
    match package {
        "@openai/codex" | "opencode-ai" => Ok(NodeVersion(18, 0, 0)),
        "@google/gemini-cli" => Ok(NodeVersion(20, 0, 0)),
        "@earendil-works/pi-coding-agent" => Ok(NodeVersion(22, 19, 0)),
        _ => Err("No fixed installer Node policy for this package".into()),
    }
}

fn parse_node_version(output: &[u8]) -> Option<NodeVersion> {
    if output.len() > 64 {
        return None;
    }
    let text = std::str::from_utf8(output).ok()?;
    // Native Node emits one LF (CRLF on Windows); don't normalize other output.
    let text = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    let mut parts = text.strip_prefix('v')?.split('.');
    let mut component = || {
        let token = parts.next()?;
        if token.is_empty()
            || (token.len() > 1 && token.starts_with('0'))
            || !token.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        token.parse::<u32>().ok()
    };
    let version = NodeVersion(component()?, component()?, component()?);
    parts.next().is_none().then_some(version)
}

fn inspect_installer_node(
    manager: &Manager,
    runner: &mut impl Runner,
) -> Result<NodeVersion, String> {
    let command = ProbeCommand {
        npm_metadata: false,
        program: manager.command.program.clone(),
        args: vec!["--version".into()],
    };
    let output = runner.run(&command).map_err(|_| {
        "Installer Node version could not be measured; update Node and restart AgentDeck before installing"
    })?;
    parse_node_version(&output).ok_or_else(||
        "Installer Node must report a stable vMAJOR.MINOR.PATCH version; update Node and restart AgentDeck before installing".into())
}

fn frozen_manager(search: &SearchPath, runner: &mut impl Runner) -> Result<Manager, String> {
    let mut command = resolved_npm_command(search, runner)?;
    let cli = if search.platform == Platform::Unix {
        canonical_file(&command.program).map_err(|_| "npm entrypoint cannot be resolved")?
    } else {
        PathBuf::from(&command.args[0])
    };
    // Only the npm package's real Node entrypoint is supported. An arbitrary
    // wrapper named npm is not an installation manager, even if it prints paths.
    let package_root = cli
        .parent()
        .and_then(Path::parent)
        .ok_or("npm entrypoint has no package directory")?;
    let root = package_root.parent().ok_or("npm has no package root")?;
    let (_, expected_cli, _) = inspect_package(root, "npm", "npm")?;
    if cli != expected_cli {
        return Err("npm entrypoint does not match its package manifest".into());
    }
    let mut head = Vec::new();
    std::fs::File::open(&cli)
        .and_then(|file| file.take(256).read_to_end(&mut head))
        .map_err(|_| "npm entrypoint cannot be read")?;
    let first_line = head.split(|byte| *byte == b'\n').next().unwrap_or_default();
    if first_line != b"#!/usr/bin/env node" && first_line != b"#!/usr/bin/env node\r" {
        return Err("Unsupported npm entrypoint interpreter".into());
    }
    if search.platform == Platform::Unix {
        let node = search
            .candidates("node")
            .map_err(|_| "Node executable search is incomplete")?
            .into_iter()
            .next()
            .ok_or("Node executable was not found")?;
        command.program =
            canonical_file(&node.path).map_err(|_| "Node executable could not be resolved")?;
        require_native_unix_node(&command.program)?;
        command.args = vec![cli.as_os_str().to_owned()];
    }
    Ok(Manager {
        program_digest: digest(&command.program)?,
        cli_digest: digest(&cli)?,
        command,
        cli,
    })
}

/// Inspect each existing component beneath a frozen canonical prefix. Missing
/// directories are allowed; symlinks, junctions, non-directories and unreadable
/// components are not. Never turn a failed lookup into proof of absence.
fn safe_directory_chain(prefix: &Path, directory: &Path) -> Result<(), String> {
    if canonical_file_or_dir(prefix).ok().as_deref() != Some(prefix) {
        return Err("npm prefix changed or is no longer an inspectable directory".into());
    }
    let relative = directory
        .strip_prefix(prefix)
        .map_err(|_| "npm destination escapes its prefix")?;
    let mut path = prefix.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err("Unsupported npm destination component".into());
        }
        path.push(component);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || !metadata.is_dir()
                    || canonical_file_or_dir(&path).ok().as_ref() != Some(&path)
                {
                    return Err("npm destination contains a linked or unusable directory".into());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err("npm destination could not be inspected".into()),
        }
    }
    Ok(())
}

fn require_absent(prefix: &Path, destination: &Path) -> Result<(), String> {
    require_absent_with(prefix, destination, |path| std::fs::symlink_metadata(path))
}

fn require_absent_with(
    prefix: &Path,
    destination: &Path,
    metadata: impl FnOnce(&Path) -> io::Result<std::fs::Metadata>,
) -> Result<(), String> {
    safe_directory_chain(
        prefix,
        destination.parent().ok_or("Destination has no parent")?,
    )?;
    match metadata(destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err("An npm package or launcher already exists, including off PATH; reinstall and upgrade are not supported".into()),
        Err(_) => Err("npm destination cannot be proven absent".into()),
    }
}

fn search_contains_directory(search: &SearchPath, directory: &Path) -> bool {
    search.directories.iter().any(|candidate| {
        // npm may create a new Unix bin directory, but its existing parent must
        // resolve to the intended prefix. No PATH edits are performed here.
        let resolved = match std::fs::canonicalize(candidate) {
            Ok(path) => Some(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => candidate
                .parent()
                .and_then(|parent| std::fs::canonicalize(parent).ok())
                .zip(candidate.file_name())
                .map(|(parent, name)| parent.join(name)),
            Err(_) => None,
        };
        resolved.as_deref() == Some(directory)
    })
}

fn build_plan(
    known: KnownAgent,
    search: &SearchPath,
    runner: &mut impl Runner,
) -> Result<InstallPlan, String> {
    if search.platform == Platform::Windows
        && (!search.suffixes.iter().any(|suffix| suffix == ".cmd")
            || search.suffixes.iter().any(|suffix| suffix == ".ps1"))
    {
        return Err("Windows command search must support the npm .cmd launcher without selecting its unsupported PowerShell companion; restore the standard PATHEXT before installing".into());
    }
    let (instances, incomplete) = search.scan_candidates(known.executable);
    if incomplete || !instances.is_empty() {
        return Err(
            "Installation requires a complete PATH search with no existing candidates".into(),
        );
    }
    let package = known.package.ok_or("No supported npm package")?;
    let installer_node_minimum = installer_node_minimum(package)?;
    let manager = frozen_manager(search, runner)?;
    let installer_node_version = inspect_installer_node(&manager, runner)?;
    if installer_node_version < installer_node_minimum {
        return Err(format!(
            "AgentDeck installer policy requires Node >= {installer_node_minimum}; detected v{installer_node_version}. Update Node and restart AgentDeck before installing; no package installation was attempted"
        ));
    }
    let raw_prefix = query_raw_path(runner, &manager.command, &["prefix", "--global"])
        .map_err(|_| "npm global prefix could not be verified")?;
    let prefix = canonical_file_or_dir(&raw_prefix)
        .map_err(|_| "npm global prefix must be an existing inspectable directory")?;
    let raw_root = query_raw_path(runner, &manager.command, &["root", "--global"])
        .map_err(|_| "npm global root could not be verified")?;
    let relative_root = if search.platform == Platform::Windows {
        "node_modules"
    } else {
        "lib/node_modules"
    };
    if raw_root != raw_prefix.join(relative_root) {
        return Err("npm global prefix and root do not describe the supported layout".into());
    }
    let root = prefix.join(relative_root);
    safe_directory_chain(&prefix, &root)?;
    require_absent(&prefix, &root.join(package))?;
    let bin = if search.platform == Platform::Windows {
        prefix.clone()
    } else {
        prefix.join("bin")
    };
    let suffixes = if search.platform == Platform::Windows {
        // npm creates all three launchers, regardless of the current PATHEXT.
        &["", ".cmd", ".ps1", ".exe", ".com", ".bat"][..]
    } else {
        &[""][..]
    };
    for suffix in suffixes {
        require_absent(&prefix, &bin.join(format!("{}{suffix}", known.executable)))?;
    }
    if !search_contains_directory(search, &bin) {
        return Err("The planned npm bin directory is not on this process's search path. Add it to PATH and restart AgentDeck before installing; no software was changed".into());
    }
    if search.platform == Platform::Windows {
        search.node_for(&bin).map_err(
            |_| "The planned npm launcher cannot resolve a supported native Node interpreter",
        )?;
    }
    let package_version = resolve_package_version(package, &manager, runner)?;
    Ok(InstallPlan {
        manager,
        prefix,
        root,
        package,
        package_version,
        installer_node_version,
        installer_node_minimum,
    })
}

// A single stable root-package version, not a tag, range, source URL, or
// dependency closure. The configured npm registry is intentionally unchanged.
fn parse_package_version(output: &[u8]) -> Option<String> {
    if output.len() > 128 {
        return None;
    }
    let version: String = serde_json::from_slice(output).ok()?;
    // Reuse the strict bounded integer triplet grammar, without Node's prefix.
    parse_node_version(format!("v{version}").as_bytes())?;
    if version.contains(['\r', '\n']) {
        return None;
    }
    Some(version)
}

fn resolve_package_version(
    package: &str,
    manager: &Manager,
    runner: &mut impl Runner,
) -> Result<Option<String>, String> {
    if package != "@openai/codex" {
        return Ok(None);
    }
    let mut command = manager.command.clone();
    command.args.extend(
        [
            "view",
            "@openai/codex@latest",
            "version",
            "--json",
            "--fetch-retries=0",
            "--fetch-timeout=25000",
        ]
        .into_iter()
        .map(OsString::from),
    );
    let output = runner
        .run_registry_metadata(&command)
        .map_err(|_| "Codex package version could not be resolved; refresh the preview")?;
    parse_package_version(&output).map(Some).ok_or_else(|| {
        "Codex package metadata must report one stable MAJOR.MINOR.PATCH version".into()
    })
}

fn verify_root_version(plan: &InstallPlan) -> Result<(), String> {
    let Some(expected) = &plan.package_version else {
        return Ok(());
    };
    let package_root = canonical_file_or_dir(&plan.root.join(plan.package))
        .map_err(|_| "Installed package directory could not be inspected")?;
    if package_root != plan.root.join(plan.package) {
        return Err("Installed package directory changed during verification".into());
    }
    let manifest = package_root.join("package.json");
    let target = canonical_file(&manifest)
        .map_err(|_| "Installed package manifest could not be inspected")?;
    if !target.starts_with(&package_root) {
        return Err("Installed package manifest escapes the package directory".into());
    }
    let bytes = read_small(&manifest, FILE_LIMIT)
        .map_err(|_| "Installed package manifest could not be read within its size limit")?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Installed package manifest is not valid JSON")?;
    if value.get("name").and_then(|v| v.as_str()) != Some(plan.package)
        || value.get("version").and_then(|v| v.as_str()) != Some(expected)
    {
        return Err(
            "Installed root package version does not match the reviewed exact version".into(),
        );
    }
    Ok(())
}

fn install_command(plan: &InstallPlan) -> ProbeCommand {
    let mut command = plan.manager.command.clone();
    command.args.extend([
        "install".into(),
        "--global".into(),
        "--prefix".into(),
        plan.prefix.as_os_str().to_owned(),
        "--force=false".into(),
        "--no-audit".into(),
        "--no-fund".into(),
        "--no-progress".into(),
    ]);
    if disables_lifecycle_scripts(plan.package) {
        command.args.push("--ignore-scripts".into());
    }
    command.args.push(match &plan.package_version {
        Some(version) => format!("{}@{version}", plan.package).into(),
        None => plan.package.into(),
    });
    command
}

fn verify_target(plan: &InstallPlan, details: &InstallationDetails) -> Result<(), String> {
    if details.probe.status != InstallationStatus::Installed || details.ambiguous {
        return Err("The installed target could not be uniquely verified".into());
    }
    let mut active = details.instances.iter().filter(|instance| instance.active);
    let instance = active
        .next()
        .ok_or("No active installed instance was found")?;
    if active.next().is_some() || instance.version.is_none() || instance.entrypoint_path.is_none() {
        return Err("The installed instance or measured version is incomplete".into());
    }
    match &instance.source {
        InstallationSource::Npm {
            package,
            package_root,
            global_root,
            prefix,
            manager_path,
        } if package == plan.package
            && package_root == &plan.root.join(plan.package)
            && global_root == &plan.root
            && prefix == &plan.prefix
            && canonical_file(manager_path).ok().as_ref() == Some(&plan.manager.cli) =>
        {
            verify_root_version(plan)
        }
        _ => Err(
            "Observed installation does not match the planned npm manager, prefix and package"
                .into(),
        ),
    }
}

fn install_with(
    known: KnownAgent,
    search: &SearchPath,
    runner: &mut impl Runner,
    install: impl FnOnce(&ProbeCommand) -> io::Result<()>,
) -> Result<(), String> {
    let plan = build_plan(known, search, runner)?;
    execute_plan(plan, known, search, runner, install)
}

fn execute_plan(
    plan: InstallPlan,
    known: KnownAgent,
    search: &SearchPath,
    runner: &mut impl Runner,
    install: impl FnOnce(&ProbeCommand) -> io::Result<()>,
) -> Result<(), String> {
    // Re-resolve the selected manager, paths and occupied destinations. The plan
    // is never accepted from IPC and is not a durable ownership receipt.
    if plan != build_plan(known, search, runner)? {
        return Err("Installation target changed during preflight; refresh and try again".into());
    }
    install(&install_command(&plan)).map_err(|_| {
        "Installer did not complete successfully; software may have changed. Refresh and inspect before retrying; no rollback was attempted".to_string()
    })?;
    let verification = (|| {
        if plan.manager != frozen_manager(search, runner)? {
            return Err("npm manager changed during installation".into());
        }
        // Reject a mismatched Codex root before inspect_with can execute its
        // Agent version probe. Keep verify_target's later check too: ordinary
        // changes during inspection must not be accepted as the reviewed root.
        // This is not an atomic filesystem or global no-execution guarantee.
        verify_root_version(&plan)?;
        verify_target(&plan, &inspect_with(known, search, runner))
    })();
    verification.map_err(|reason| format!(
        "Installer finished, but target verification is incomplete: {reason}. Software may have changed; no rollback was attempted"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODEX: KnownAgent = KnownAgent {
        executable: "codex",
        package: Some("@openai/codex"),
    };

    #[test]
    fn package_version_accepts_only_one_stable_exact_json_string() {
        for output in [br#""0.167.1""#.as_slice(), b"\"1.2.3\"\r\n"] {
            assert!(parse_package_version(output).is_some());
        }
        for output in [
            "",
            "1.2.3",
            "null",
            "[]",
            "[\"1.2.3\"]",
            "{\"version\":\"1.2.3\"}",
            "\"latest\"",
            "\"^1.2.3\"",
            "\"v1.2.3\"",
            "\"01.2.3\"",
            "\"1.2\"",
            "\"1.2.3.4\"",
            "\"1.2.3-beta.1\"",
            "\"1.2.3+build\"",
            "\"4294967296.2.3\"",
            "\" 1.2.3\"",
            "\"1.2.3\\n\"",
            "\"1.2.3\" \"2.3.4\"",
            "\"https://example.test/pkg.tgz\"",
        ] {
            assert!(
                parse_package_version(output.as_bytes()).is_none(),
                "{output}"
            );
        }
        assert!(parse_package_version(&[b' '; 129]).is_none());
        assert!(parse_package_version(&[0xff]).is_none());
    }

    struct MetadataRunner {
        inner: FakeRunner,
        outputs: std::collections::VecDeque<io::Result<Vec<u8>>>,
    }
    impl Runner for MetadataRunner {
        fn run(&mut self, command: &ProbeCommand) -> io::Result<Vec<u8>> {
            if command.args.iter().any(|arg| arg == "view") {
                assert_eq!(
                    &command.args[1..],
                    &[
                        OsString::from("view"),
                        "@openai/codex@latest".into(),
                        "version".into(),
                        "--json".into(),
                        "--fetch-retries=0".into(),
                        "--fetch-timeout=25000".into(),
                    ]
                );
                return self.outputs.pop_front().expect("unexpected metadata query");
            }
            self.inner.run(command)
        }
    }

    #[test]
    fn package_metadata_failures_and_drift_never_invoke_install() {
        let fixture = Fixture::new(Platform::Windows);
        let plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
        let revision = project_plan("codex", CODEX, Platform::Windows, &plan)
            .unwrap()
            .revision;
        for outputs in [
            vec![Err(io::Error::other("timeout or failed query"))],
            vec![Ok(b"[]".to_vec())],
            vec![Ok(br#""99.99.98""#.to_vec())],
            vec![Ok(br#""99.99.99""#.to_vec()), Ok(br#""99.99.98""#.to_vec())],
        ] {
            let mut runner = MetadataRunner {
                inner: fixture.runner(),
                outputs: outputs.into(),
            };
            assert!(
                install_reviewed_with(
                    "codex",
                    &revision,
                    CODEX,
                    &fixture.search,
                    &mut runner,
                    |_| panic!("invalid or changed metadata must not install")
                )
                .is_err()
            );
            assert!(!fixture.root.exists());
        }
        let mut changed = plan;
        changed.package_version = Some("99.99.98".into());
        assert_ne!(
            revision,
            project_plan("codex", CODEX, Platform::Windows, &changed)
                .unwrap()
                .revision
        );
    }

    #[test]
    fn root_manifest_version_not_cli_measurement_must_match_pin() {
        for version in [None, Some("1.2.3"), Some("99.99.99-beta"), Some("99.99.99")] {
            let fixture = Fixture::new(Platform::Windows);
            let result = install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| {
                fixture.populate();
                let manifest = fixture.root.join("@openai/codex/package.json");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
                match version {
                    Some(v) => value["version"] = v.into(),
                    None => {
                        value.as_object_mut().unwrap().remove("version");
                    }
                }
                std::fs::write(manifest, serde_json::to_vec(&value).unwrap()).unwrap();
                Ok(())
            });
            assert_eq!(result.is_ok(), version == Some("99.99.99"));
            if let Err(error) = result {
                assert!(error.contains("root package version"));
                assert!(error.contains("no rollback"));
            }
            assert!(fixture.root.join("@openai/codex/package.json").exists());
        }
    }

    #[test]
    fn observed_reviewed_install_signals_only_attempted_mutation_once_before_callback() {
        for scenario in [
            "stale",
            "second-preflight",
            "success",
            "installer-error",
            "root-mismatch",
        ] {
            let fixture = Fixture::new(Platform::Windows);
            let plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
            let mut revision = project_plan("codex", CODEX, Platform::Windows, &plan)
                .unwrap()
                .revision;
            if scenario == "stale" {
                revision = "0".repeat(64);
            }
            let outputs = if scenario == "second-preflight" {
                vec![Ok(br#""99.99.99""#.to_vec()), Ok(br#""99.99.98""#.to_vec())]
            } else {
                vec![Ok(br#""99.99.99""#.to_vec()), Ok(br#""99.99.99""#.to_vec())]
            };
            let mut runner = MetadataRunner {
                inner: fixture.runner(),
                outputs: outputs.into(),
            };
            let starts = std::cell::Cell::new(0);
            let installs = std::cell::Cell::new(0);
            let result = install_reviewed_observed_with(
                "codex",
                &revision,
                CODEX,
                &fixture.search,
                &mut runner,
                || {
                    starts.set(starts.get() + 1);
                },
                |_| {
                    assert_eq!(starts.get(), 1, "observer must precede mutation");
                    installs.set(installs.get() + 1);
                    fixture.populate();
                    if scenario == "installer-error" {
                        return Err(io::Error::other("synthetic spawn/write failure"));
                    }
                    if scenario == "root-mismatch" {
                        std::fs::write(fixture.root.join("@openai/codex/package.json"), b"{}")
                            .unwrap();
                    }
                    Ok(())
                },
            );
            assert_eq!(result.is_ok(), scenario == "success", "{scenario}");
            let attempted = usize::from(!matches!(scenario, "stale" | "second-preflight"));
            assert_eq!(starts.get(), attempted, "{scenario}");
            assert_eq!(installs.get(), attempted, "{scenario}");
        }
    }

    struct AgentProbeRunner {
        inner: FakeRunner,
        agent_probes: usize,
        on_agent: Option<Box<dyn FnOnce()>>,
    }

    impl Runner for AgentProbeRunner {
        fn run(&mut self, command: &ProbeCommand) -> io::Result<Vec<u8>> {
            let installer_node = command.args == [OsString::from("--version")]
                && command
                    .program
                    .file_name()
                    .is_some_and(|name| name == "node" || name == "node.exe");
            if !command.npm_metadata && !installer_node {
                self.agent_probes += 1;
                if let Some(action) = self.on_agent.take() {
                    action();
                }
            }
            self.inner.run(command)
        }
    }

    #[test]
    fn invalid_installed_root_refuses_before_immediate_agent_probe_without_rollback() {
        for scenario in [
            "missing-manifest",
            "malformed",
            "wrong-name",
            "wrong-version",
            "missing-version",
            "nonstring-version",
            "oversized",
            "manifest-directory",
            "missing-root",
        ] {
            let fixture = Fixture::new(Platform::Windows);
            let mut runner = AgentProbeRunner {
                inner: fixture.runner(),
                agent_probes: 0,
                on_agent: None,
            };
            let marker = fixture.prefix.join("installer-residue");
            let mut remaining_manifest = None;
            let manifest = fixture.root.join("@openai/codex/package.json");
            let error = install_with(CODEX, &fixture.search, &mut runner, |_| {
                fixture.populate();
                std::fs::write(&marker, b"preserve partial installer output").unwrap();
                match scenario {
                    "missing-root" => std::fs::rename(
                        fixture.root.join("@openai/codex"),
                        fixture.root.join("@openai/retained-output"),
                    )
                    .unwrap(),
                    "missing-manifest" => std::fs::remove_file(&manifest).unwrap(),
                    "manifest-directory" => {
                        std::fs::remove_file(&manifest).unwrap();
                        std::fs::create_dir(&manifest).unwrap();
                    }
                    "malformed" => std::fs::write(&manifest, b"{").unwrap(),
                    "oversized" => std::fs::write(&manifest, vec![b' '; FILE_LIMIT + 1]).unwrap(),
                    _ => {
                        let mut value: serde_json::Value =
                            serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
                        match scenario {
                            "wrong-name" => value["name"] = "different-package".into(),
                            "wrong-version" => value["version"] = "1.2.3".into(),
                            "missing-version" => {
                                value.as_object_mut().unwrap().remove("version");
                            }
                            "nonstring-version" => value["version"] = 42.into(),
                            _ => unreachable!(),
                        }
                        std::fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
                    }
                }
                if manifest.is_file() {
                    remaining_manifest = Some(std::fs::read(&manifest).unwrap());
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(runner.agent_probes, 0, "{scenario}");
            assert!(
                error.contains("target verification is incomplete"),
                "{error}"
            );
            assert!(error.contains("no rollback was attempted"), "{error}");
            assert_eq!(
                std::fs::read(&marker).unwrap(),
                b"preserve partial installer output"
            );
            assert_eq!(
                std::fs::read_to_string(fixture.prefix.join("codex.cmd")).unwrap(),
                node_cmd_shim("node_modules/@openai/codex/bin/codex.js")
            );
            if let Some(bytes) = remaining_manifest {
                assert_eq!(std::fs::read(&manifest).unwrap(), bytes);
            }
            match scenario {
                "missing-root" => assert!(
                    fixture
                        .root
                        .join("@openai/retained-output/package.json")
                        .exists()
                ),
                "missing-manifest" => assert!(!manifest.exists()),
                "manifest-directory" => assert!(manifest.is_dir()),
                _ => assert!(manifest.is_file()),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn redirected_installed_root_refuses_before_immediate_agent_probe() {
        for redirect_manifest in [false, true] {
            let fixture = Fixture::new(Platform::Unix);
            let mut runner = AgentProbeRunner {
                inner: fixture.runner(),
                agent_probes: 0,
                on_agent: None,
            };
            let package = fixture.root.join("@openai/codex");
            let external = fixture.directory.join("retained-external");
            let error = install_with(CODEX, &fixture.search, &mut runner, |_| {
                fixture.populate();
                if redirect_manifest {
                    let manifest = package.join("package.json");
                    std::fs::rename(&manifest, &external).unwrap();
                    std::os::unix::fs::symlink(&external, &manifest).unwrap();
                } else {
                    std::fs::rename(&package, &external).unwrap();
                    std::os::unix::fs::symlink(&external, &package).unwrap();
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(runner.agent_probes, 0);
            assert!(error.contains("target verification is incomplete"));
            assert!(error.contains("no rollback"));
            assert!(external.exists());
            assert!(
                std::fs::symlink_metadata(if redirect_manifest {
                    package.join("package.json")
                } else {
                    package
                })
                .unwrap()
                .file_type()
                .is_symlink()
            );
        }
    }

    #[test]
    fn matching_root_keeps_measured_version_provenance_and_late_root_checks() {
        for scenario in [
            "matching",
            "invalid-measured-version",
            "wrong-provenance",
            "late-root-change",
        ] {
            let fixture = Fixture::new(Platform::Windows);
            let manifest = fixture.root.join("@openai/codex/package.json");
            let mut runner = AgentProbeRunner {
                inner: fixture.runner(),
                agent_probes: 0,
                on_agent: None,
            };
            if scenario == "invalid-measured-version" {
                runner.inner.version = b"not an agent version".to_vec();
            }
            if scenario == "late-root-change" {
                let manifest = manifest.clone();
                runner.on_agent = Some(Box::new(move || {
                    let mut value: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
                    value["version"] = "1.2.3".into();
                    std::fs::write(manifest, serde_json::to_vec(&value).unwrap()).unwrap();
                }));
            }
            let result = install_with(CODEX, &fixture.search, &mut runner, |_| {
                fixture.populate();
                if scenario == "wrong-provenance" {
                    // A valid root version does not authorize an unrelated launcher.
                    std::fs::write(
                        fixture.prefix.join("codex.cmd"),
                        node_cmd_shim("node_modules/unrelated/bin/codex.js"),
                    )
                    .unwrap();
                }
                Ok(())
            });
            assert_eq!(result.is_ok(), scenario == "matching", "{scenario}");
            if scenario != "wrong-provenance" {
                assert_eq!(runner.agent_probes, 1, "{scenario}");
            }
            if let Err(error) = result {
                assert!(
                    error.contains("target verification is incomplete"),
                    "{error}"
                );
                assert!(error.contains("no rollback"), "{error}");
                if scenario == "late-root-change" {
                    assert!(error.contains("root package version"), "{error}");
                }
            }
            assert!(manifest.exists());
        }
    }

    #[test]
    fn non_codex_adapters_do_not_resolve_or_pin_versions() {
        let fixture = Fixture::new(Platform::Windows);
        for package in [
            "opencode-ai",
            "@google/gemini-cli",
            "@earendil-works/pi-coding-agent",
        ] {
            let known = KnownAgent {
                executable: "other-agent",
                package: Some(package),
            };
            let mut runner = MetadataRunner {
                inner: fixture.runner(),
                outputs: Default::default(),
            };
            let plan = build_plan(known, &fixture.search, &mut runner).unwrap();
            let preview = project_plan("other-agent", known, Platform::Windows, &plan).unwrap();
            assert!(preview.package_version.is_none());
            assert!(
                verify_root_version(&plan).is_ok(),
                "unpinned adapter must not add a root guard"
            );
            assert_eq!(preview.version_policy, "npm_configured");
            assert_eq!(install_command(&plan).args.last().unwrap(), package);
        }
    }

    #[test]
    fn preview_serialization_and_exact_confirmation_install_once() {
        for platform in [Platform::Windows, Platform::Unix] {
            if platform == Platform::Unix && !cfg!(unix) {
                continue;
            }
            let fixture = Fixture::new(platform);
            let plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
            let preview = project_plan("codex", CODEX, platform, &plan).unwrap();
            let json = serde_json::to_value(&preview).unwrap();
            assert_eq!(json.as_object().unwrap().len(), 16);
            assert_eq!(json["schemaVersion"], 1);
            assert_eq!(json["agentId"], "codex");
            assert_eq!(json["package"], "@openai/codex");
            assert_eq!(json["executable"], "codex");
            assert_eq!(json["managerPath"], plan.manager.cli.to_str().unwrap());
            assert_eq!(
                json["nodePath"],
                plan.manager.command.program.to_str().unwrap()
            );
            assert_eq!(json["prefix"], plan.prefix.to_str().unwrap());
            assert_eq!(json["globalRoot"], plan.root.to_str().unwrap());
            assert_eq!(
                json["packageRoot"],
                plan.root.join(plan.package).to_str().unwrap()
            );
            let bin = if platform == Platform::Windows {
                plan.prefix.clone()
            } else {
                plan.prefix.join("bin")
            };
            assert_eq!(json["launcherDirectory"], bin.to_str().unwrap());
            assert_eq!(json["scriptPolicy"], "disabled");
            assert_eq!(json["versionPolicy"], "exact_root");
            assert_eq!(json["packageVersion"], "99.99.99");
            assert_eq!(preview.revision.len(), 64);
            assert!(
                preview
                    .revision
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            );
            assert!(
                !fixture.root.exists(),
                "Preview must not create the installation target"
            );
            let mut calls = 0;
            install_reviewed_with(
                "codex",
                &preview.revision,
                CODEX,
                &fixture.search,
                &mut fixture.runner(),
                |_| {
                    calls += 1;
                    fixture.populate();
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn missing_malformed_wrong_agent_and_wrong_revision_never_install() {
        let fixture = Fixture::new(Platform::Windows);
        let plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
        let preview = project_plan("codex", CODEX, Platform::Windows, &plan).unwrap();
        for (id, revision) in [
            ("codex", "".to_owned()),
            ("codex", "0".repeat(64)),
            ("codex", "A".repeat(64)),
            ("other", preview.revision),
        ] {
            assert!(
                install_reviewed_with(
                    id,
                    &revision,
                    CODEX,
                    &fixture.search,
                    &mut fixture.runner(),
                    |_| panic!("Unconfirmed target must not install")
                )
                .is_err()
            );
        }
        assert!(!fixture.root.exists());
    }

    #[test]
    fn every_displayed_and_execution_identity_changes_revision() {
        let fixture = Fixture::new(Platform::Windows);
        let mut plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
        let original = project_plan("codex", CODEX, Platform::Windows, &plan).unwrap();
        let other_known = KnownAgent {
            executable: "other",
            package: CODEX.package,
        };
        assert_ne!(
            original.revision,
            project_plan("codex", other_known, Platform::Windows, &plan)
                .unwrap()
                .revision
        );
        assert_ne!(
            original.revision,
            project_plan("codex", CODEX, Platform::Unix, &plan)
                .unwrap()
                .revision
        );
        plan.package = "@earendil-works/pi-coding-agent";
        let pi = project_plan("codex", CODEX, Platform::Windows, &plan).unwrap();
        assert_eq!(pi.script_policy, "disabled");
        assert_ne!(original.revision, pi.revision);
        plan.package = "other-package";
        assert_ne!(
            original.revision,
            project_plan("codex", CODEX, Platform::Windows, &plan)
                .unwrap()
                .revision
        );
        plan.package = "@openai/codex";
        plan.manager.command.args.push("--synthetic-option".into());
        assert_ne!(
            original.revision,
            project_plan("codex", CODEX, Platform::Windows, &plan)
                .unwrap()
                .revision
        );
    }

    #[test]
    fn stale_node_cli_prefix_and_root_never_reach_installer() {
        for changed in ["node", "cli", "prefix", "root"] {
            let mut fixture = Fixture::new(Platform::Windows);
            let plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
            let preview = project_plan("codex", CODEX, Platform::Windows, &plan).unwrap();
            let mut runner = fixture.runner();
            match changed {
                "node" => {
                    std::fs::write(&plan.manager.command.program, b"\x7fELFchanged bytes").unwrap()
                }
                "cli" => std::fs::write(&fixture.manager_cli, b"#!/usr/bin/env node\n// changed")
                    .unwrap(),
                "prefix" => {
                    let prefix = fixture.directory.join("new global");
                    std::fs::create_dir(&prefix).unwrap();
                    fixture.search.directories.push(prefix.clone());
                    runner.prefix = prefix.clone();
                    runner.root = prefix.join("node_modules");
                }
                "root" => runner.root = fixture.directory.join("other root"),
                _ => unreachable!(),
            }
            assert!(
                install_reviewed_with(
                    "codex",
                    &preview.revision,
                    CODEX,
                    &fixture.search,
                    &mut runner,
                    |_| panic!("Stale preview must not install")
                )
                .is_err(),
                "{changed}"
            );
            assert!(!fixture.root.exists());
        }
    }

    #[test]
    fn reviewed_target_still_requires_absence_double_preflight_and_postcheck() {
        for scenario in ["occupied", "race", "postcheck", "failure"] {
            let mut fixture = Fixture::new(Platform::Windows);
            let plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
            let preview = project_plan("codex", CODEX, Platform::Windows, &plan).unwrap();
            let mut runner = fixture.runner();
            if scenario == "occupied" {
                fixture.search.directories.pop();
                std::fs::create_dir_all(fixture.root.join("@openai/codex")).unwrap();
            }
            if scenario == "race" {
                let cli = fixture.manager_cli.clone();
                runner.mutate = Some(Box::new(move |call| {
                    if call == 2 {
                        std::fs::write(&cli, "#!/usr/bin/env node\n// changed").unwrap();
                    }
                }));
            }
            let mut calls = 0;
            let error = install_reviewed_with(
                "codex",
                &preview.revision,
                CODEX,
                &fixture.search,
                &mut runner,
                |_| {
                    calls += 1;
                    assert!(scenario == "postcheck" || scenario == "failure");
                    // An unrelated/partial install must be left intact, never rolled back.
                    std::fs::create_dir_all(fixture.root.join("partial-output")).unwrap();
                    if scenario == "failure" {
                        Err(io::Error::other("synthetic failure"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            if scenario == "postcheck" || scenario == "failure" {
                assert_eq!(calls, 1);
                assert!(error.contains("no rollback"));
                assert!(fixture.root.join("partial-output").exists());
            } else {
                assert_eq!(calls, 0);
            }
        }
    }

    #[test]
    fn display_projection_is_lossless_bounded_and_rejects_hidden_controls() {
        for valid in [
            "/tmp/中文 & %VAR% !",
            r"C:\Users\测试\App Data",
            r"\\?\C:\npm",
        ] {
            assert_eq!(display_value(valid.as_ref()).unwrap(), valid);
        }
        for invalid in [
            "",
            " /tmp/path",
            "/tmp/path ",
            "/tmp/a\nb",
            "/tmp/a\0b",
            "/tmp/a\u{202e}b",
            "/tmp/a\u{200b}b",
        ] {
            assert!(display_value(invalid.as_ref()).is_err());
        }
        assert!(display_value("x".repeat(32769).as_ref()).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            assert!(display_value(&OsString::from_vec(vec![b'/', 0xff])).is_err());
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            assert!(display_value(&OsString::from_wide(&[0xd800])).is_err());
        }
    }

    #[test]
    fn preview_and_reviewed_install_keep_unsupported_adapters_closed() {
        let registry =
            crate::registry::parse_registry(include_str!("../../../../../agents/registry-v3.json"))
                .unwrap();
        for id in ["claude-code", "qwen-code", "codex"] {
            let mut agent = registry.agents[id].clone();
            if id == "codex" {
                agent.install.command = Some("npm install -g arbitrary".into());
            }
            assert!(preview_install(&agent).is_err());
            assert!(install_agent_from_preview(&agent, &"0".repeat(64)).is_err());
        }
    }

    struct Fixture {
        directory: PathBuf,
        prefix: PathBuf,
        root: PathBuf,
        manager_cli: PathBuf,
        search: SearchPath,
    }

    fn executable(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    impl Fixture {
        fn new(platform: Platform) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "agentdeck install 中文 & %VAR% ! {}",
                uuid::Uuid::new_v4()
            ));
            let manager = directory.join("manager");
            let manager_root = manager.join(if platform == Platform::Windows {
                "node_modules"
            } else {
                "lib/node_modules"
            });
            let manager_cli = manager_root.join("npm/bin/npm-cli.js");
            executable(
                &manager_cli,
                "#!/usr/bin/env node\n// synthetic; never execute\n",
            );
            std::fs::write(
                manager_root.join("npm/package.json"),
                r#"{"name":"npm","bin":{"npm":"bin/npm-cli.js"}}"#,
            )
            .unwrap();
            let manager_bin = if platform == Platform::Windows {
                manager.clone()
            } else {
                manager.join("bin")
            };
            std::fs::create_dir_all(&manager_bin).unwrap();
            if platform == Platform::Windows {
                executable(
                    &manager_bin.join("node.exe"),
                    "\u{7f}ELFsynthetic native interpreter; never execute",
                );
                std::fs::write(
                    manager_bin.join("npm.cmd"),
                    node_cmd_shim("node_modules/npm/bin/npm-cli.js"),
                )
                .unwrap();
            } else {
                #[cfg(unix)]
                {
                    executable(
                        &manager_bin.join("node"),
                        "\u{7f}ELFsynthetic native interpreter; never execute",
                    );
                    std::os::unix::fs::symlink(&manager_cli, manager_bin.join("npm")).unwrap();
                }
                #[cfg(not(unix))]
                unreachable!("Unix fixture requires Unix links");
            }
            let prefix = directory.join("global");
            let root = prefix.join(if platform == Platform::Windows {
                "node_modules"
            } else {
                "lib/node_modules"
            });
            let bin = if platform == Platform::Windows {
                prefix.clone()
            } else {
                prefix.join("bin")
            };
            std::fs::create_dir_all(&bin).unwrap();
            let search = SearchPath {
                directories: vec![manager_bin, bin],
                suffixes: if platform == Platform::Windows {
                    windows_suffixes(".COM;.EXE;.BAT;.CMD").unwrap()
                } else {
                    vec![String::new()]
                },
                platform,
            };
            Self {
                directory,
                prefix,
                root,
                manager_cli,
                search,
            }
        }

        fn runner(&self) -> FakeRunner {
            FakeRunner {
                prefix: self.prefix.clone(),
                root: self.root.clone(),
                calls: 0,
                fail: false,
                version: b"codex-cli 1.2.3\n".to_vec(),
                node_version: b"v24.9.0\n".to_vec(),
                package_version: b"\"99.99.99\"\n".to_vec(),
                mutate: None,
            }
        }

        fn populate(&self) {
            let package = self.root.join("@openai/codex");
            let bin = package.join("bin/codex.js");
            executable(&bin, "#!/usr/bin/env node\n// synthetic; never execute\n");
            std::fs::write(
                package.join("package.json"),
                r#"{"name":"@openai/codex","version":"99.99.99","bin":{"codex":"bin/codex.js"}}"#,
            )
            .unwrap();
            if self.search.platform == Platform::Windows {
                std::fs::write(
                    self.prefix.join("codex.cmd"),
                    node_cmd_shim("node_modules/@openai/codex/bin/codex.js"),
                )
                .unwrap();
            } else {
                #[cfg(unix)]
                std::os::unix::fs::symlink(&bin, self.prefix.join("bin/codex")).unwrap();
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    struct FakeRunner {
        prefix: PathBuf,
        root: PathBuf,
        calls: usize,
        fail: bool,
        version: Vec<u8>,
        node_version: Vec<u8>,
        package_version: Vec<u8>,
        mutate: Option<Box<dyn FnMut(usize)>>,
    }

    impl Runner for FakeRunner {
        fn run(&mut self, command: &ProbeCommand) -> io::Result<Vec<u8>> {
            self.calls += 1;
            if let Some(mutate) = &mut self.mutate {
                mutate(self.calls);
            }
            assert!(command.program.is_absolute());
            assert!(
                !command.args.iter().any(|arg| arg == "install"),
                "probe runner must never install"
            );
            if self.fail {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "synthetic query failure",
                ));
            }
            if command.args == [OsString::from("--version")]
                && command
                    .program
                    .file_name()
                    .is_some_and(|name| name == "node" || name == "node.exe")
            {
                return Ok(self.node_version.clone());
            }
            if command.args.last().is_some_and(|arg| arg == "--version") {
                return Ok(self.version.clone());
            }
            if command.args.iter().any(|arg| arg == "view") {
                return Ok(self.package_version.clone());
            }
            let path = if command.args.iter().any(|arg| {
                arg == "prefix"
                    || Path::new(arg)
                        .file_name()
                        .is_some_and(|name| name == "npm-prefix.js")
            }) {
                &self.prefix
            } else {
                &self.root
            };
            Ok(format!("{}\n", path.display()).into_bytes())
        }
    }

    /// Explicit prerequisite-only acceptance. Never executes npm or an Agent.
    #[test]
    #[ignore = "requires an explicitly supplied trusted native Node"]
    fn trusted_installer_node_version_smoke() {
        let supplied = PathBuf::from(
            std::env::var_os("AGENTDECK_TEST_NODE")
                .expect("Set AGENTDECK_TEST_NODE to an explicit trusted native Node path"),
        );
        assert!(supplied.is_absolute());
        let program = canonical_file(&supplied).unwrap();
        #[cfg(unix)]
        require_native_unix_node(&program).unwrap();
        #[cfg(windows)]
        canonical_windows_program(&program).unwrap();
        let manager = Manager {
            program_digest: digest(&program).unwrap(),
            cli_digest: Vec::new(),
            command: ProbeCommand {
                npm_metadata: false,
                program,
                args: Vec::new(),
            },
            // Unused: the measured invocation must not contain an npm entrypoint.
            cli: PathBuf::from("not-an-npm-entrypoint"),
        };
        let version = inspect_installer_node(&manager, &mut ProcessRunner).unwrap();
        assert!(version >= NodeVersion(18, 0, 0));
        assert_eq!(
            manager.program_digest,
            digest(&manager.command.program).unwrap()
        );
        println!("Frozen installer Node measured directly: v{version}; no npm or Agent invoked");
    }

    #[test]
    fn installer_node_parser_is_strict_and_bounded() {
        for bytes in [b"v18.0.0".as_slice(), b"v18.0.0\n", b"v18.0.0\r\n"] {
            assert_eq!(parse_node_version(bytes), Some(NodeVersion(18, 0, 0)));
        }
        for invalid in [
            "",
            "18.0.0",
            " v18.0.0",
            "v18.0.0 ",
            "v18.0",
            "v18.0.0.0",
            "v18.0.0-pre",
            "v18.0.0+build",
            "v018.0.0",
            "v18.00.0",
            "v18.0.-1",
            "v4294967296.0.0",
            "v18.0.0\n\n",
            "v18.0.0\r",
            "v18.0.0\nsecret",
            "node v18.0.0",
        ] {
            assert_eq!(parse_node_version(invalid.as_bytes()), None, "{invalid:?}");
        }
        assert_eq!(parse_node_version(&[0xff]), None);
        assert_eq!(parse_node_version(&[b'1'; 65]), None);
    }

    #[test]
    fn each_fixed_installer_node_minimum_is_enforced_before_npm_queries() {
        for (package, executable, minimum, below) in [
            ("@openai/codex", "codex", "v18.0.0", "v17.99.99"),
            ("opencode-ai", "opencode", "v18.0.0", "v17.99.99"),
            ("@google/gemini-cli", "gemini", "v20.0.0", "v19.99.99"),
            (
                "@earendil-works/pi-coding-agent",
                "pi",
                "v22.19.0",
                "v22.18.99",
            ),
        ] {
            let known = KnownAgent {
                executable,
                package: Some(package),
            };
            let fixture = Fixture::new(Platform::Windows);
            let mut runner = fixture.runner();
            runner.node_version = below.as_bytes().to_vec();
            assert!(build_plan(known, &fixture.search, &mut runner).is_err());
            assert_eq!(runner.calls, 1, "Only direct Node version may be queried");
            for valid in [minimum, "v24.9.0", "v100.0.0"] {
                runner.node_version = valid.as_bytes().to_vec();
                let plan = build_plan(known, &fixture.search, &mut runner).unwrap();
                assert_eq!(format!("v{}", plan.installer_node_version), valid);
                assert_eq!(format!("v{}", plan.installer_node_minimum), minimum);
            }
        }
        assert!(installer_node_minimum("@arbitrary/package").is_err());
    }

    #[test]
    fn malformed_and_failed_installer_node_probes_fail_closed() {
        let fixture = Fixture::new(Platform::Windows);
        for invalid in [b"v24.9.0-rc.1".as_slice(), b"v24.9.0\nextra", b"", &[0xff]] {
            let mut runner = fixture.runner();
            runner.node_version = invalid.to_vec();
            assert!(
                install_with(CODEX, &fixture.search, &mut runner, |_| panic!(
                    "must not install"
                ))
                .is_err()
            );
            assert_eq!(runner.calls, 1);
        }
        for kind in [
            io::ErrorKind::TimedOut,
            io::ErrorKind::Other,
            io::ErrorKind::NotFound,
        ] {
            struct FailedRunner(io::ErrorKind);
            impl Runner for FailedRunner {
                fn run(&mut self, command: &ProbeCommand) -> io::Result<Vec<u8>> {
                    assert_eq!(command.args, [OsString::from("--version")]);
                    assert!(!command.npm_metadata);
                    Err(io::Error::new(self.0, "private output"))
                }
            }
            let error = install_with(CODEX, &fixture.search, &mut FailedRunner(kind), |_| {
                panic!("must not install")
            })
            .unwrap_err();
            assert!(!error.contains("private output"));
        }
    }

    #[test]
    fn changed_installer_node_measurement_invalidates_review_and_second_preflight() {
        let fixture = Fixture::new(Platform::Windows);
        let mut runner = fixture.runner();
        let plan = build_plan(CODEX, &fixture.search, &mut runner).unwrap();
        let preview = project_plan("codex", CODEX, Platform::Windows, &plan).unwrap();
        assert_eq!(preview.installer_node_version, "v24.9.0");
        assert_eq!(preview.installer_node_minimum, "18.0.0");
        runner.node_version = b"v24.10.0".to_vec();
        assert!(
            install_reviewed_with(
                "codex",
                &preview.revision,
                CODEX,
                &fixture.search,
                &mut runner,
                |_| panic!("stale measurement must not install")
            )
            .is_err()
        );
        assert!(
            execute_plan(plan, CODEX, &fixture.search, &mut runner, |_| panic!(
                "changed preflight must not install"
            ))
            .is_err()
        );
    }

    #[test]
    fn old_installer_node_cannot_reach_package_mutation() {
        let fixture = Fixture::new(Platform::Windows);
        let mut runner = fixture.runner();
        runner.node_version = b"v17.9.1\n".to_vec();
        let mut installed = false;
        let result = install_with(CODEX, &fixture.search, &mut runner, |_| {
            installed = true;
            fixture.populate();
            Ok(())
        });
        assert!(!installed, "older installer Node reached package mutation");
        assert!(result.is_err());
    }

    #[test]
    fn fresh_target_uses_frozen_node_npm_prefix_and_target_bound_verification() {
        let fixture = Fixture::new(Platform::Windows);
        assert!(
            !fixture.root.exists(),
            "a fresh global root need not already exist"
        );
        let mut calls = 0;
        install_with(CODEX, &fixture.search, &mut fixture.runner(), |command| {
            calls += 1;
            assert_eq!(
                command.program,
                std::fs::canonicalize(fixture.search.directories[0].join("node.exe")).unwrap()
            );
            assert_eq!(
                PathBuf::from(&command.args[0]),
                std::fs::canonicalize(&fixture.manager_cli).unwrap()
            );
            assert_eq!(
                &command.args[1..4],
                &[
                    OsString::from("install"),
                    "--global".into(),
                    "--prefix".into()
                ]
            );
            assert_eq!(
                PathBuf::from(&command.args[4]),
                std::fs::canonicalize(&fixture.prefix).unwrap()
            );
            assert_eq!(command.args.last().unwrap(), "@openai/codex@99.99.99");
            assert!(!command.args.iter().any(|arg| arg == "/C" || arg == "-c"));
            fixture.populate();
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
    }

    #[test]
    fn distributed_windows_npm_freezes_redirected_cli_without_switching_node() {
        let fixture = Fixture::new(Platform::Windows);
        let manager_bin = &fixture.search.directories[0];
        std::fs::write(manager_bin.join("npm.cmd"), NPM_DISTRIBUTED_CMD).unwrap();
        std::fs::write(
            fixture.manager_cli.parent().unwrap().join("npm-prefix.js"),
            "synthetic prefix helper; never execute",
        )
        .unwrap();
        let redirected = fixture.root.join("npm/bin/npm-cli.js");
        executable(
            &redirected,
            "#!/usr/bin/env node\n// redirected synthetic npm\n",
        );
        std::fs::write(
            fixture.root.join("npm/package.json"),
            r#"{"name":"npm","bin":{"npm":"bin/npm-cli.js"}}"#,
        )
        .unwrap();
        install_with(CODEX, &fixture.search, &mut fixture.runner(), |command| {
            assert_eq!(
                PathBuf::from(&command.args[0]),
                std::fs::canonicalize(&redirected).unwrap()
            );
            assert_eq!(
                command.program,
                std::fs::canonicalize(manager_bin.join("node.exe")).unwrap()
            );
            fixture.populate();
            Ok(())
        })
        .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn windows_native_looking_node_link_to_batch_cannot_reach_installer() {
        let fixture = Fixture::new(Platform::Windows);
        let manager_bin = &fixture.search.directories[0];
        let batch = manager_bin.join("node.cmd");
        std::fs::write(&batch, "@echo off\necho unexpected\n").unwrap();
        std::fs::remove_file(manager_bin.join("node.exe")).unwrap();
        std::os::unix::fs::symlink(&batch, manager_bin.join("node.exe")).unwrap();
        let mut runner = fixture.runner();
        assert!(
            install_with(CODEX, &fixture.search, &mut runner, |_| {
                panic!("A native-looking link to a batch file must not install")
            })
            .is_err()
        );
        assert_eq!(runner.calls, 0);
    }

    #[cfg(unix)]
    #[test]
    fn unix_npm_shebang_is_replaced_with_a_frozen_node_invocation() {
        let fixture = Fixture::new(Platform::Unix);
        install_with(CODEX, &fixture.search, &mut fixture.runner(), |command| {
            assert_eq!(
                command.program,
                std::fs::canonicalize(fixture.search.directories[0].join("node")).unwrap()
            );
            assert_eq!(
                PathBuf::from(&command.args[0]),
                std::fs::canonicalize(&fixture.manager_cli).unwrap()
            );
            fixture.populate();
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn off_path_package_and_launcher_destinations_block_without_installer_calls() {
        for destination in [
            "node_modules/@openai/codex",
            "codex.cmd",
            "codex.ps1",
            "codex",
        ] {
            let mut fixture = Fixture::new(Platform::Windows);
            fixture.search.directories.pop(); // global bin is not on PATH
            let target = fixture.prefix.join(destination);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(&target, "preexisting; do not overwrite").unwrap();
            let before = std::fs::read(&target).unwrap();
            assert!(
                install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| panic!(
                    "off-PATH destination must block"
                ))
                .is_err()
            );
            assert_eq!(std::fs::read(target).unwrap(), before);
        }
        let mut fixture = Fixture::new(Platform::Windows);
        fixture.populate();
        fixture.search.directories.pop();
        std::fs::remove_file(fixture.prefix.join("codex.cmd")).unwrap();
        assert!(
            install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| {
                panic!("An existing npm package with no PATH launcher must not be reinstalled")
            })
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn linked_dangling_and_redirected_destinations_are_never_absent() {
        for kind in ["package", "dangling", "scope", "root"] {
            let fixture = Fixture::new(Platform::Unix);
            let external = fixture.directory.join("external");
            std::fs::create_dir(&external).unwrap();
            let target = match kind {
                "scope" => fixture.root.join("@openai"),
                "root" => fixture.root.clone(),
                _ => fixture.root.join("@openai/codex"),
            };
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(
                if kind == "dangling" {
                    fixture.directory.join("missing")
                } else {
                    external
                },
                target,
            )
            .unwrap();
            assert!(
                install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| panic!(
                    "linked destination must block"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn uncertain_manager_or_root_never_runs_the_installer() {
        let fixture = Fixture::new(Platform::Windows);
        let mut runner = fixture.runner();
        runner.fail = true;
        assert!(
            install_with(CODEX, &fixture.search, &mut runner, |_| panic!(
                "failed metadata cannot authorize install"
            ))
            .is_err()
        );
        let mut runner = fixture.runner();
        runner.root = fixture.directory.clone();
        assert!(
            install_with(CODEX, &fixture.search, &mut runner, |_| panic!(
                "conflicting layout cannot authorize install"
            ))
            .is_err()
        );
        std::fs::write(
            fixture.search.directories[0].join("npm.cmd"),
            "@echo off\necho fake prefix\n",
        )
        .unwrap();
        assert!(
            install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| panic!(
                "unknown manager cannot authorize install"
            ))
            .is_err()
        );
    }

    #[test]
    fn fresh_prefix_off_path_is_rejected_before_any_installation() {
        let mut fixture = Fixture::new(Platform::Windows);
        fixture.search.directories.pop();
        let error = install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| {
            panic!("Do not install a target that the current search path cannot verify")
        })
        .unwrap_err();
        assert!(error.contains("not on this process's search path"));
        assert!(!fixture.root.exists());
    }

    #[test]
    fn windows_unverifiable_pathext_and_launcher_interpreter_block_before_installing() {
        for pathext in [".EXE", ".EXE;.CMD;.PS1", ".CMD"] {
            let mut fixture = Fixture::new(Platform::Windows);
            fixture.search.suffixes = windows_suffixes(pathext).unwrap();
            assert!(
                install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| {
                    panic!("Unsupported launcher search must fail before installing")
                })
                .is_err()
            );
            assert!(!fixture.root.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn unix_node_wrappers_are_rejected_and_missing_on_path_bin_is_supported() {
        let fixture = Fixture::new(Platform::Unix);
        std::fs::remove_dir(fixture.prefix.join("bin")).unwrap();
        assert!(build_plan(CODEX, &fixture.search, &mut fixture.runner()).is_ok());
        executable(
            &fixture.search.directories[0].join("node"),
            "#!/bin/sh\nexit 0\n",
        );
        let error = install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| {
            panic!("Node scripts cannot be used as a frozen interpreter")
        })
        .unwrap_err();
        assert!(error.contains("not a script or wrapper"));
    }

    #[test]
    fn unreadable_destination_and_non_directory_ancestor_are_not_absence() {
        let fixture = Fixture::new(Platform::Windows);
        let prefix = std::fs::canonicalize(&fixture.prefix).unwrap();
        let target = prefix.join("codex.cmd");
        for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::Other] {
            assert!(
                require_absent_with(&prefix, &target, |_| {
                    Err(io::Error::new(kind, "synthetic inaccessible destination"))
                })
                .is_err()
            );
        }
        std::fs::write(&fixture.root, "not a directory").unwrap();
        assert!(
            install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| {
                panic!("An unusable ancestor must block installation")
            })
            .is_err()
        );
    }

    #[test]
    fn changed_prefix_plan_and_post_install_source_are_not_accepted() {
        let mut fixture = Fixture::new(Platform::Windows);
        let plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
        let other_prefix = fixture.directory.join("other-global");
        std::fs::create_dir(&other_prefix).unwrap();
        fixture.search.directories.push(other_prefix.clone());
        let mut runner = fixture.runner();
        runner.prefix = other_prefix.clone();
        runner.root = other_prefix.join("node_modules");
        let other = build_plan(CODEX, &fixture.search, &mut runner).unwrap();
        assert_ne!(
            plan, other,
            "A changed prefix requires a different installation plan"
        );
        fixture.populate();
        let mut details = inspect_with(CODEX, &fixture.search, &mut fixture.runner());
        verify_target(&plan, &details).unwrap();
        if let InstallationSource::Npm { prefix, .. } = &mut details.instances[0].source {
            *prefix = other_prefix;
        } else {
            panic!("Synthetic npm association was expected");
        }
        assert!(verify_target(&plan, &details).is_err());
    }

    #[test]
    fn changed_manager_and_newly_occupied_package_block_during_revalidation() {
        for manager_change in [true, false] {
            let fixture = Fixture::new(Platform::Windows);
            let mut runner = fixture.runner();
            let cli = fixture.manager_cli.clone();
            let package = fixture.root.join("@openai/codex");
            runner.mutate = Some(Box::new(move |call| {
                if call == 2 {
                    if manager_change {
                        std::fs::write(&cli, "#!/usr/bin/env node\n// changed\n").unwrap();
                    } else {
                        std::fs::create_dir_all(&package).unwrap();
                    }
                }
            }));
            assert!(
                install_with(CODEX, &fixture.search, &mut runner, |_| panic!(
                    "changed preflight must block"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn missing_or_unrelated_post_install_target_is_not_success_and_never_rolled_back() {
        for unrelated in [false, true] {
            let fixture = Fixture::new(Platform::Windows);
            let error = install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| {
                if unrelated {
                    executable(
                        &fixture.prefix.join("codex.exe"),
                        "unrelated synthetic binary",
                    );
                }
                Ok(())
            })
            .unwrap_err();
            assert!(error.contains("verification is incomplete"));
            assert!(error.contains("no rollback"));
            if unrelated {
                assert!(fixture.prefix.join("codex.exe").exists());
            }
        }
    }

    #[test]
    fn ambiguous_or_unversioned_post_install_target_is_not_success() {
        for ambiguous in [false, true] {
            let fixture = Fixture::new(Platform::Windows);
            let mut runner = fixture.runner();
            if !ambiguous {
                runner.version = b"unrecognized output".to_vec();
            }
            assert!(
                install_with(CODEX, &fixture.search, &mut runner, |_| {
                    fixture.populate();
                    if ambiguous {
                        executable(
                            &fixture.prefix.join("codex.exe"),
                            "second synthetic instance",
                        );
                    }
                    Ok(())
                })
                .is_err()
            );
            assert!(fixture.root.join("@openai/codex").exists());
        }
    }

    #[test]
    fn failed_installer_leaves_partial_destination_and_reports_uncertainty() {
        let fixture = Fixture::new(Platform::Windows);
        let error = install_with(CODEX, &fixture.search, &mut fixture.runner(), |_| {
            fixture.populate();
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "synthetic installer timeout",
            ))
        })
        .unwrap_err();
        assert!(error.contains("software may have changed"));
        assert!(fixture.root.join("@openai/codex").exists());
    }

    #[test]
    fn safe_install_capability_requires_exact_adapter_recipe_and_fixed_script_policy() {
        let registry =
            crate::registry::parse_registry(include_str!("../../../../../agents/registry-v3.json"))
                .unwrap();
        for id in ["codex", "opencode", "gemini-cli", "pi"] {
            let mut agent = registry.agents[id].clone();
            assert!(can_install(&agent), "bundled {id} recipe must be supported");
            agent.install.command = Some("npm install -g unexpected-package".into());
            assert!(!can_install(&agent));
            assert!(install_agent(&agent).is_err());
        }
        let fixture = Fixture::new(Platform::Windows);
        let mut plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
        for (package, disabled) in [
            ("@openai/codex", true),
            ("@earendil-works/pi-coding-agent", true),
            ("@google/gemini-cli", false),
            ("opencode-ai", false),
        ] {
            plan.package = package;
            plan.package_version = (package == "@openai/codex").then(|| "99.99.99".into());
            let preview = project_plan("synthetic", CODEX, Platform::Windows, &plan).unwrap();
            let command = install_command(&plan);
            assert_eq!(
                command
                    .args
                    .iter()
                    .filter(|arg| *arg == "--ignore-scripts")
                    .count(),
                usize::from(disabled)
            );
            assert_eq!(
                preview.script_policy,
                if disabled {
                    "disabled"
                } else {
                    "npm_configured"
                }
            );
            assert_eq!(
                preview.version_policy,
                if package == "@openai/codex" {
                    "exact_root"
                } else {
                    "npm_configured"
                }
            );
        }
    }
}
