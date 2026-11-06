//! Read-only installation observations, never installation history or permission
//! to mutate a package. Registry recipes are not provenance. Unsupported layouts
//! remain unknown; a future management operation must repeat all observations.
use super::{InstallationProbe, InstallationStatus};
use crate::registry::AgentDefinition;
use std::collections::HashSet;
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const OUTPUT_LIMIT: usize = 8192;
const FILE_LIMIT: usize = 128 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

mod installation;
pub(super) use installation::{can_install, install_agent};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationDetails {
    pub probe: InstallationProbe,
    pub instances: Vec<InstallationInstance>,
    /// More than one distinct executable target, not repeated PATH entries or
    /// symlinks to the same target. Such observations cannot select a write target.
    pub ambiguous: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationInstance {
    /// Absolute location selected from the application's command search path.
    pub path: PathBuf,
    /// Canonical discovered file. For a Windows .cmd this is the shim itself.
    pub resolved_path: Option<PathBuf>,
    /// Actual package bin, only when the selected link/shim was verified.
    pub entrypoint_path: Option<PathBuf>,
    /// Measured from the active instance, never a registry or manifest version.
    pub version: Option<String>,
    pub source: InstallationSource,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum InstallationSource {
    Unknown {
        reason: String,
    },
    /// Matches the currently queried npm global layout and package manifest.
    /// This does NOT prove who installed it, or that npm historically installed it.
    Npm {
        package: String,
        package_root: PathBuf,
        global_root: PathBuf,
        prefix: PathBuf,
        manager_path: PathBuf,
    },
}

impl InstallationSource {
    fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Platform {
    Unix,
    Windows,
}

impl Platform {
    fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

struct SearchPath {
    directories: Vec<PathBuf>,
    suffixes: Vec<String>,
    platform: Platform,
}

impl SearchPath {
    fn from_environment() -> io::Result<Self> {
        let path =
            std::env::var_os("PATH").ok_or_else(|| io::Error::other("PATH is not available"))?;
        let cwd = std::env::current_dir()?;
        let platform = Platform::current();
        let mut directories = Vec::new();
        if platform == Platform::Windows {
            // The application's previous cmd verification also searched CWD.
            directories.push(cwd.clone());
        }
        directories.extend(std::env::split_paths(&path).map(|dir| {
            if dir.is_absolute() {
                dir
            } else {
                cwd.join(dir)
            }
        }));
        let suffixes = if platform == Platform::Windows {
            windows_suffixes(
                &std::env::var("PATHEXT")
                    .map_err(|_| io::Error::other("PATHEXT is not available"))?,
            )?
        } else {
            vec![String::new()]
        };
        Ok(Self {
            directories,
            suffixes,
            platform,
        })
    }

    fn candidates(&self, name: &str) -> io::Result<Vec<InstallationInstance>> {
        let (instances, incomplete) = self.scan_candidates(name);
        if incomplete {
            Err(io::Error::other("Executable search is incomplete"))
        } else {
            Ok(instances)
        }
    }

    fn scan_candidates(&self, name: &str) -> (Vec<InstallationInstance>, bool) {
        let mut seen = HashSet::new();
        let mut instances = Vec::new();
        let mut incomplete = self.directories.len() > 512;
        for directory in self.directories.iter().take(512) {
            for suffix in &self.suffixes {
                let path = directory.join(format!("{name}{suffix}"));
                if !seen.insert(path.clone()) {
                    continue;
                }
                let mut reason = None;
                match std::fs::symlink_metadata(&path) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                    Err(_) => reason = Some("A PATH candidate cannot be inspected"),
                    Ok(_) => {}
                }
                if reason.is_none() {
                    match std::fs::metadata(&path) {
                        Ok(metadata) if metadata.is_file() =>
                        {
                            #[cfg(unix)]
                            if self.platform == Platform::Unix {
                                use std::os::unix::fs::PermissionsExt;
                                if metadata.permissions().mode() & 0o111 == 0 {
                                    reason = Some("A PATH candidate is not executable");
                                }
                            }
                        }
                        Ok(_) => reason = Some("A PATH candidate is not a regular file"),
                        Err(_) => {
                            reason = Some("A PATH candidate is unreadable or a dangling link")
                        }
                    }
                }
                let resolved = std::fs::canonicalize(&path).ok();
                if resolved.is_none() && reason.is_none() {
                    reason = Some("A PATH candidate cannot be resolved");
                }
                if instances.len() >= 64 {
                    return (instances, true);
                }
                incomplete |= reason.is_some();
                instances.push(InstallationInstance {
                    path,
                    resolved_path: resolved,
                    entrypoint_path: None,
                    version: None,
                    source: InstallationSource::unknown(
                        reason.unwrap_or("Installation source has not been verified"),
                    ),
                    active: instances.is_empty(),
                });
            }
        }
        (instances, incomplete)
    }

    fn node_for(&self, directory: &Path) -> io::Result<PathBuf> {
        let adjacent = directory.join("node.exe");
        match std::fs::symlink_metadata(&adjacent) {
            Ok(_) => return canonical_windows_program(&adjacent),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        // Do not use node.cmd/node.bat or another shell wrapper as an interpreter.
        let node = self
            .candidates("node")?
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::other("A native Node executable was not found"))?;
        canonical_windows_program(&node.path)
    }
}

fn windows_suffixes(pathext: &str) -> io::Result<Vec<String>> {
    let mut suffixes = Vec::new();
    for value in pathext.split(';') {
        let value = value.to_ascii_lowercase();
        // Enumerate all declared suffixes, including standard Windows script
        // handlers. Unsupported candidates are reported, never executed.
        if !value.starts_with('.')
            || value.len() < 2
            || !value[1..].bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(io::Error::other("PATHEXT cannot be resolved reliably"));
        }
        if !suffixes.contains(&value) {
            suffixes.push(value);
        }
    }
    if suffixes.is_empty() {
        return Err(io::Error::other("PATHEXT is empty"));
    }
    Ok(suffixes)
}

#[derive(Clone, Copy)]
struct KnownAgent {
    executable: &'static str,
    package: Option<&'static str>,
}

fn known_agent(agent: &AgentDefinition) -> Option<KnownAgent> {
    let command = agent.install.verify_cmd.as_deref()?.trim();
    let (id, executable, package) = match command {
        "codex --version" => ("codex", "codex", Some("@openai/codex")),
        "claude --version" => ("claude-code", "claude", Some("@anthropic-ai/claude-code")),
        "opencode --version" => ("opencode", "opencode", Some("opencode-ai")),
        "gemini --version" => ("gemini-cli", "gemini", Some("@google/gemini-cli")),
        "pi --version" => ("pi", "pi", Some("@earendil-works/pi-coding-agent")),
        "hermes --version" => ("hermes", "hermes", None),
        _ => return None,
    };
    Some(KnownAgent {
        executable,
        package: (agent.id == id).then_some(package).flatten(),
    })
}

/// Observe the selected local instance and any shadowed PATH candidates. This is
/// read-only evidence, not authorization or a durable receipt for future changes.
pub fn inspect_installation(agent: &AgentDefinition) -> InstallationDetails {
    let Some(known) = known_agent(agent) else {
        // Existing custom verification remains bounded, but arbitrary shell text
        // is never interpreted as a path, actual version, or package-manager proof.
        return InstallationDetails {
            probe: super::probe_installation_with(agent, super::executable_on_path, |command| {
                super::run_shell_with_timeout(command, PROBE_TIMEOUT)
            }),
            instances: Vec::new(),
            ambiguous: false,
            message: Some(
                "Custom verification has no supported instance or source inspection".into(),
            ),
        };
    };
    match SearchPath::from_environment() {
        Ok(search) => inspect_with(known, &search, &mut ProcessRunner),
        Err(_) => error_details("Cannot inspect the executable search path"),
    }
}

fn error_details(message: &str) -> InstallationDetails {
    InstallationDetails {
        probe: InstallationProbe {
            status: InstallationStatus::Error,
            message: Some(message.into()),
        },
        instances: Vec::new(),
        ambiguous: false,
        message: None,
    }
}

fn inspect_with(
    known: KnownAgent,
    search: &SearchPath,
    runner: &mut impl Runner,
) -> InstallationDetails {
    let (mut instances, incomplete) = search.scan_candidates(known.executable);
    if incomplete {
        let mut details = error_details(
            "Executable search is incomplete; an unusable or unreadable candidate may exist",
        );
        details.ambiguous = instances.len() > 1;
        details.instances = instances;
        return details;
    }
    if instances.is_empty() {
        return InstallationDetails {
            probe: InstallationProbe {
                status: InstallationStatus::NotDetected,
                message: Some(format!(
                    "{} was not found on this process's command search path; it may be installed elsewhere",
                    known.executable
                )),
            },
            instances,
            ambiguous: false,
            message: None,
        };
    }
    // Recognize a safe Windows launcher independently from npm availability.
    // A missing/broken manager must not turn a readable known launcher into a
    // failed software probe or hide its actual version.
    if search.platform == Platform::Windows
        && let Some(package) = known.package
    {
        for instance in &mut instances {
            if let Ok(bin) = windows_entrypoint(instance, package, known.executable) {
                instance.entrypoint_path = Some(bin);
            }
        }
    }
    let npm = known
        .package
        .map(|package| inspect_npm(package, known.executable, search, runner));
    for instance in &mut instances {
        match &npm {
            Some(Ok(layout)) => match layout.bind(instance, search.platform) {
                Ok(()) => {}
                Err(reason) => instance.source = InstallationSource::unknown(reason),
            },
            Some(Err(reason)) => instance.source = InstallationSource::unknown(reason.clone()),
            None => {
                instance.source = InstallationSource::unknown(
                    "No supported package-source adapter for this agent",
                )
            }
        }
    }
    let distinct: HashSet<_> = instances
        .iter()
        .map(|instance| {
            instance
                .entrypoint_path
                .as_ref()
                .or(instance.resolved_path.as_ref())
                .unwrap_or(&instance.path)
        })
        .collect();
    let ambiguous = distinct.len() > 1;
    let active = &mut instances[0];
    let result = version_command(active, search).and_then(|command| runner.run(&command));
    let probe = match result {
        Ok(output) => {
            active.version = parse_version(known.executable, &output);
            InstallationProbe {
                status: InstallationStatus::Installed,
                message: active.version.is_none().then(|| "The selected executable responded successfully, but its version could not be identified".into()),
            }
        }
        Err(error) => InstallationProbe {
            status: InstallationStatus::Error,
            message: Some(if error.kind() == io::ErrorKind::TimedOut {
                "Selected executable verification timed out".into()
            } else {
                "Selected executable could not be safely verified".into()
            }),
        },
    };
    InstallationDetails {
        probe,
        instances,
        ambiguous,
        message: ambiguous.then(|| "Multiple distinct installation targets were found; resolve the intended installation before configuring".into()),
    }
}

struct NpmLayout {
    package: String,
    package_root: PathBuf,
    global_root: PathBuf,
    prefix: PathBuf,
    manager_path: PathBuf,
    bin: PathBuf,
    relative_bin: String,
}

impl NpmLayout {
    fn bind(&self, instance: &mut InstallationInstance, platform: Platform) -> Result<(), String> {
        let expected_directory = if platform == Platform::Windows {
            self.prefix.clone()
        } else {
            self.prefix.join("bin")
        };
        let directory = instance
            .path
            .parent()
            .ok_or("Executable has no parent directory")?;
        let actual_directory = std::fs::canonicalize(directory)
            .map_err(|_| "Executable parent directory could not be resolved")?;
        let expected_directory = std::fs::canonicalize(expected_directory)
            .map_err(|_| "npm bin directory could not be resolved")?;
        if actual_directory != expected_directory {
            return Err(
                "The selected executable is outside the queried npm global bin directory".into(),
            );
        }
        if platform == Platform::Windows {
            match_node_shim(instance, &self.package, &self.relative_bin, &self.bin)?;
        } else if instance.resolved_path.as_ref() != Some(&self.bin) {
            return Err("The selected executable target does not match the npm package bin".into());
        }
        instance.entrypoint_path = Some(self.bin.clone());
        instance.source = InstallationSource::Npm {
            package: self.package.clone(),
            package_root: self.package_root.clone(),
            global_root: self.global_root.clone(),
            prefix: self.prefix.clone(),
            manager_path: self.manager_path.clone(),
        };
        Ok(())
    }
}

fn inspect_npm(
    package: &str,
    executable: &str,
    search: &SearchPath,
    runner: &mut impl Runner,
) -> Result<NpmLayout, String> {
    let command = resolved_npm_command(search, runner)?;
    let prefix = query_path(runner, &command, &["prefix", "--global"])
        .map_err(|_| "npm global prefix could not be verified")?;
    let root = query_path(runner, &command, &["root", "--global"])
        .map_err(|_| "npm global package root could not be verified")?;
    let expected_root = prefix.join(if search.platform == Platform::Windows {
        "node_modules"
    } else {
        "lib/node_modules"
    });
    if canonical_file_or_dir(&expected_root).ok().as_ref() != Some(&root)
        || !root.starts_with(&prefix)
        || root == prefix
    {
        return Err("npm global prefix and package root do not describe a supported layout".into());
    }
    let (package_root, bin, relative_bin) = inspect_package(&root, package, executable)?;
    let manager_path = if search.platform == Platform::Windows {
        PathBuf::from(&command.args[0])
    } else {
        command.program.clone()
    };
    Ok(NpmLayout {
        package: package.into(),
        package_root,
        global_root: root,
        prefix,
        manager_path,
        bin,
        relative_bin,
    })
}

fn resolved_npm_command(
    search: &SearchPath,
    runner: &mut impl Runner,
) -> Result<ProbeCommand, String> {
    let manager =
        npm_command(search).map_err(|_| "A supported npm manager could not be resolved")?;
    let mut command = manager.command;
    if let Some(prefix_command) = manager.redirect_prefix {
        // The distributed npm.cmd invokes npm-prefix.js, not `npm prefix`.
        // Respect that exact program before selecting the final npm CLI.
        let redirect_prefix = query_path(runner, &prefix_command, &[])
            .map_err(|_| "npm launcher prefix redirection could not be resolved")?;
        let redirected = redirect_prefix.join("node_modules/npm/bin/npm-cli.js");
        match std::fs::symlink_metadata(&redirected) {
            Ok(_) => {
                command.args[0] = canonical_file(&redirected)
                    .map_err(|_| "npm prefix redirection cannot be resolved")?
                    .into_os_string()
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err("npm prefix redirection could not be inspected".into()),
        }
    }
    Ok(command)
}

fn inspect_package(
    root: &Path,
    package: &str,
    executable: &str,
) -> Result<(PathBuf, PathBuf, String), String> {
    let package_path = root.join(package);
    let package_root = canonical_file_or_dir(&package_path)
        .map_err(|_| "Allowlisted npm package was not found")?;
    // Reject linked packages, including npm link, even if a declared bin happens
    // to match: the global manager must not claim ownership of an external tree.
    if package_root != package_path || !package_root.starts_with(root) {
        return Err("Linked or redirected npm packages are not supported source evidence".into());
    }
    let manifest_path = package_root.join("package.json");
    let manifest_target =
        canonical_file(&manifest_path).map_err(|_| "Package manifest could not be inspected")?;
    if !manifest_target.starts_with(&package_root) {
        return Err("Package manifest escapes the package directory".into());
    }
    let manifest: serde_json::Value = serde_json::from_slice(
        &read_small(&manifest_path, FILE_LIMIT)
            .map_err(|_| "Package manifest could not be read within its size limit")?,
    )
    .map_err(|_| "Package manifest is not valid JSON")?;
    if manifest.get("name").and_then(|value| value.as_str()) != Some(package) {
        return Err("Package manifest name does not match the exact allowlisted package".into());
    }
    let bin = manifest
        .get("bin")
        .ok_or("Package manifest has no executable mapping")?;
    let relative_bin = if let Some(value) = bin.as_str() {
        if package.rsplit('/').next() != Some(executable) {
            return Err("Package string bin maps a different executable name".into());
        }
        value
    } else {
        bin.get(executable)
            .and_then(|value| value.as_str())
            .ok_or("Package manifest does not map this executable")?
    };
    if !safe_relative_bin(relative_bin) {
        return Err("Package executable mapping contains an unsupported or escaping path".into());
    }
    let bin = canonical_file(&package_root.join(relative_bin))
        .map_err(|_| "Package executable could not be resolved")?;
    if !bin.starts_with(&package_root) {
        return Err("Package executable escapes its package directory".into());
    }
    Ok((package_root, bin, relative_bin.into()))
}

fn windows_entrypoint(
    instance: &InstallationInstance,
    package: &str,
    executable: &str,
) -> Result<PathBuf, String> {
    let prefix = instance
        .path
        .parent()
        .ok_or("Executable has no parent directory")?;
    let prefix =
        std::fs::canonicalize(prefix).map_err(|_| "Executable directory cannot be resolved")?;
    let root = canonical_file_or_dir(&prefix.join("node_modules"))
        .map_err(|_| "No local package directory")?;
    if !root.starts_with(&prefix) || root == prefix {
        return Err("Launcher package directory escapes its prefix".into());
    }
    let (_, bin, relative_bin) = inspect_package(&root, package, executable)?;
    match_node_shim(instance, package, &relative_bin, &bin)?;
    Ok(bin)
}

fn match_node_shim(
    instance: &InstallationInstance,
    package: &str,
    relative_bin: &str,
    bin: &Path,
) -> Result<(), String> {
    if !instance
        .path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd"))
    {
        return Err(
            "Only an exact supported npm .cmd launcher can be associated on Windows".into(),
        );
    }
    let target = format!("node_modules/{package}/{relative_bin}");
    let text = read_small(&instance.path, FILE_LIMIT)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    if text.as_deref().map(normalize_lines).as_deref() != Some(node_cmd_shim(&target).as_str()) {
        return Err("The Windows launcher is not an exact supported npm Node shim".into());
    }
    // Read only a bounded prefix, not a potentially large bundled JS file.
    let mut head = Vec::new();
    std::fs::File::open(bin)
        .and_then(|file| file.take(256).read_to_end(&mut head))
        .map_err(|_| "Package entrypoint cannot be inspected")?;
    let head = head.split(|byte| *byte == b'\n').next().unwrap_or_default();
    if head != b"#!/usr/bin/env node" && head != b"#!/usr/bin/env node\r" {
        return Err("Package entrypoint does not use the supported Node interpreter".into());
    }
    Ok(())
}

fn safe_relative_bin(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-/".contains(&byte))
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn canonical_file(path: &Path) -> io::Result<PathBuf> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::other("Expected a regular file"));
    }
    std::fs::canonicalize(path)
}

/// Windows Command can implicitly invoke the batch interpreter for .cmd/.bat.
/// Validate both the selected filename and the canonical target before returning
/// a program path; a native-looking symlink must never reveal a batch program.
fn canonical_windows_program(path: &Path) -> io::Result<PathBuf> {
    fn native_suffix(path: &Path) -> bool {
        path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("com")
        })
    }
    if !native_suffix(path) {
        return Err(io::Error::other(
            "The selected Windows program is not a native executable",
        ));
    }
    let resolved = canonical_file(path)?;
    if !native_suffix(&resolved) {
        return Err(io::Error::other(
            "The resolved Windows program is not a native executable",
        ));
    }
    Ok(resolved)
}

fn canonical_file_or_dir(path: &Path) -> io::Result<PathBuf> {
    if !std::fs::metadata(path)?.is_dir() {
        return Err(io::Error::other("Expected a directory"));
    }
    std::fs::canonicalize(path)
}

fn read_small(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::other("Inspection input exceeds size limit"));
    }
    Ok(bytes)
}

fn normalize_lines(value: &str) -> String {
    value.replace("\r\n", "\n")
}

/// Exact npm cmd-shim Node template, without extra flags or environment exports.
/// New npm templates intentionally require review rather than a substring match.
fn node_cmd_shim(target: &str) -> String {
    let target = target.replace('/', "\\");
    format!(
        "@ECHO off\nGOTO start\n:find_dp0\nSET dp0=%~dp0\nEXIT /b\n:start\nSETLOCAL\nCALL :find_dp0\n\nIF EXIST \"%dp0%\\node.exe\" (\n  SET \"_prog=%dp0%\\node.exe\"\n) ELSE (\n  SET \"_prog=node\"\n  SET PATHEXT=%PATHEXT:;.JS;=;%\n)\n\nendLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\"  \"%dp0%\\{target}\" %*\n"
    )
}

const NPM_DISTRIBUTED_CMD: &str = ":: Created by npm, please don't edit manually.\n@ECHO OFF\n\nSETLOCAL\n\nSET \"NODE_EXE=%~dp0\\node.exe\"\nIF NOT EXIST \"%NODE_EXE%\" (\n  SET \"NODE_EXE=node\"\n)\n\nSET \"NPM_PREFIX_JS=%~dp0\\node_modules\\npm\\bin\\npm-prefix.js\"\nSET \"NPM_CLI_JS=%~dp0\\node_modules\\npm\\bin\\npm-cli.js\"\nFOR /F \"delims=\" %%F IN ('CALL \"%NODE_EXE%\" \"%NPM_PREFIX_JS%\"') DO (\n  SET \"NPM_PREFIX_NPM_CLI_JS=%%F\\node_modules\\npm\\bin\\npm-cli.js\"\n)\nIF EXIST \"%NPM_PREFIX_NPM_CLI_JS%\" (\n  SET \"NPM_CLI_JS=%NPM_PREFIX_NPM_CLI_JS%\"\n)\n\n\"%NODE_EXE%\" \"%NPM_CLI_JS%\" %*\n";

struct NpmCommand {
    command: ProbeCommand,
    redirect_prefix: Option<ProbeCommand>,
}

fn npm_command(search: &SearchPath) -> io::Result<NpmCommand> {
    let manager = search
        .candidates("npm")?
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("npm was not found"))?;
    if search.platform == Platform::Unix {
        return Ok(NpmCommand {
            command: ProbeCommand {
                npm_metadata: true,
                program: manager.path,
                args: Vec::new(),
            },
            redirect_prefix: None,
        });
    }
    if !manager
        .path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd"))
    {
        return Err(io::Error::other("Unsupported npm manager entrypoint"));
    }
    let text = String::from_utf8(read_small(&manager.path, FILE_LIMIT)?)
        .map_err(|_| io::Error::other("Unsupported npm launcher encoding"))?;
    let text = normalize_lines(&text);
    let redirect_prefix = text == NPM_DISTRIBUTED_CMD;
    if !redirect_prefix && text != node_cmd_shim("node_modules/npm/bin/npm-cli.js") {
        return Err(io::Error::other("Unsupported npm launcher template"));
    }
    let directory = manager
        .path
        .parent()
        .ok_or_else(|| io::Error::other("npm has no directory"))?;
    let cli = canonical_file(&directory.join("node_modules/npm/bin/npm-cli.js"))?;
    let node = search.node_for(directory)?;
    let prefix_command = if redirect_prefix {
        Some(ProbeCommand {
            npm_metadata: true,
            program: node.clone(),
            args: vec![
                canonical_file(&directory.join("node_modules/npm/bin/npm-prefix.js"))?
                    .into_os_string(),
            ],
        })
    } else {
        None
    };
    Ok(NpmCommand {
        command: ProbeCommand {
            npm_metadata: true,
            program: node,
            args: vec![cli.into_os_string()],
        },
        redirect_prefix: prefix_command,
    })
}

fn query_path(runner: &mut impl Runner, base: &ProbeCommand, args: &[&str]) -> io::Result<PathBuf> {
    canonical_file_or_dir(&query_raw_path(runner, base, args)?)
}

fn query_raw_path(
    runner: &mut impl Runner,
    base: &ProbeCommand,
    args: &[&str],
) -> io::Result<PathBuf> {
    let mut command = base.clone();
    command.args.extend(args.iter().map(OsString::from));
    let output = runner.run(&command)?;
    if output.len() > OUTPUT_LIMIT {
        return Err(io::Error::other("npm path output exceeds size limit"));
    }
    let value =
        std::str::from_utf8(&output).map_err(|_| io::Error::other("npm path is not UTF-8"))?;
    let value = value.trim_end_matches(['\r', '\n']);
    if value.is_empty() || value.chars().any(|ch| ch.is_control()) || value.contains(['\n', '\r']) {
        return Err(io::Error::other("npm did not report one path"));
    }
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(io::Error::other("npm path is not absolute"));
    }
    Ok(path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeCommand {
    npm_metadata: bool,
    program: PathBuf,
    args: Vec<OsString>,
}

fn version_command(
    instance: &InstallationInstance,
    search: &SearchPath,
) -> io::Result<ProbeCommand> {
    if std::fs::canonicalize(&instance.path).ok().as_ref() != instance.resolved_path.as_ref() {
        return Err(io::Error::other("Executable changed during inspection"));
    }
    if search.platform == Platform::Windows {
        match instance
            .path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("cmd") => {
                let entrypoint = instance.entrypoint_path.as_ref().ok_or_else(|| {
                    io::Error::other("Unverified batch launchers are not executed")
                })?;
                let node = search.node_for(instance.path.parent().unwrap_or(Path::new("")))?;
                return Ok(ProbeCommand {
                    npm_metadata: false,
                    program: node,
                    args: vec![entrypoint.as_os_str().to_owned(), "--version".into()],
                });
            }
            Some("exe" | "com") => {
                return Ok(ProbeCommand {
                    npm_metadata: false,
                    program: canonical_windows_program(&instance.path)?,
                    args: vec!["--version".into()],
                });
            }
            _ => return Err(io::Error::other("Unsupported Windows executable type")),
        }
    }
    Ok(ProbeCommand {
        npm_metadata: false,
        program: instance.path.clone(),
        args: vec!["--version".into()],
    })
}

fn parse_version(executable: &str, output: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(output)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    if text.len() > 128 || text.chars().any(|ch| ch.is_control()) {
        return None;
    }
    let token = match executable {
        "codex" => text.strip_prefix("codex-cli ")?,
        "claude" => text.strip_suffix(" (Claude Code)")?,
        "opencode" | "gemini" | "pi" => text,
        // No stable output contract is established for a custom Hermes install.
        _ => return None,
    };
    let expression = regex::Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?(?:\+[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?$").ok()?;
    expression.is_match(token).then(|| token.to_owned())
}

trait Runner {
    fn run(&mut self, command: &ProbeCommand) -> io::Result<Vec<u8>>;
}

struct ProcessRunner;
impl Runner for ProcessRunner {
    fn run(&mut self, command: &ProbeCommand) -> io::Result<Vec<u8>> {
        run_bounded(command, PROBE_TIMEOUT, OUTPUT_LIMIT)
    }
}

/// Direct program/argv only. No paths or manager output are inserted into shell
/// source. Stderr is discarded so npm config warnings cannot leak credentials.
fn run_bounded(command: &ProbeCommand, timeout: Duration, limit: usize) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut process = Command::new(&command.program);
    if command.npm_metadata {
        // Quiet metadata queries without replacing userconfig/prefix/root. npm
        // still runs local code, so this is not a side-effect or security sandbox.
        for (key, value) in [
            ("npm_config_update_notifier", "false"),
            ("npm_config_logs_max", "0"),
            ("npm_config_loglevel", "silent"),
            ("npm_config_audit", "false"),
            ("npm_config_fund", "false"),
        ] {
            process.env(key, value).env(key.to_ascii_uppercase(), value);
        }
    }
    process
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        process.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        process.creation_flags(0x08000000 | 0x00000004); // NO_WINDOW | SUSPENDED
    }
    #[cfg(windows)]
    let job = super::process_guard::ProbeJob::new()?;
    let mut child = process.spawn()?;
    #[cfg(windows)]
    if let Err(error) = job.assign_and_resume(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Probe stdout is unavailable"))?;
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .and_then(|_| {
                if bytes.len() > limit {
                    Err(io::Error::other("Probe output exceeds size limit"))
                } else {
                    Ok(bytes)
                }
            });
        let _ = send.send(result);
    });
    let mut output = None;
    let result = loop {
        match receive.try_recv() {
            Ok(Ok(bytes)) => output = Some(bytes),
            Ok(Err(error)) => break Err(error),
            Err(std::sync::mpsc::TryRecvError::Disconnected) if output.is_none() => {
                break Err(io::Error::other("Probe output reader failed"));
            }
            _ => {}
        }
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                break Err(io::Error::other("Probe command exited unsuccessfully"));
            }
            Ok(Some(_)) if output.is_some() => break Ok(output.take().unwrap()),
            Err(error) => break Err(error),
            _ => {}
        }
        if Instant::now() >= deadline {
            break Err(io::Error::new(io::ErrorKind::TimedOut, "Probe timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Also remove any background descendants after a successful parent exit.
    // Windows closes the job, including descendants retaining stdout; Unix kills
    // the fresh probe process group. Neither path spawns an unbounded helper.
    #[cfg(unix)]
    super::process_guard::kill_group(child.id());
    #[cfg(windows)]
    drop(job);
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODEX: KnownAgent = KnownAgent {
        executable: "codex",
        package: Some("@openai/codex"),
    };

    struct Fixture {
        directory: PathBuf,
        prefix: PathBuf,
        root: PathBuf,
        package: PathBuf,
        bin: PathBuf,
        launcher: PathBuf,
        search: SearchPath,
    }

    impl Fixture {
        fn new(platform: Platform) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "agentdeck inspect 中文 & %VAR% ! {}",
                uuid::Uuid::new_v4()
            ));
            let prefix = directory.join("prefix");
            let root = prefix.join(if platform == Platform::Windows {
                "node_modules"
            } else {
                "lib/node_modules"
            });
            let package = root.join("@openai/codex");
            let bin = package.join("bin/codex.js");
            std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
            write_executable(
                &bin,
                "#!/usr/bin/env node\n// Synthetic fixture, never execute.\n",
            );
            write_manifest(
                &package,
                serde_json::json!({"name":"@openai/codex", "version":"99.99.99", "bin":{"codex":"bin/codex.js"}}),
            );
            let bindir = if platform == Platform::Windows {
                prefix.clone()
            } else {
                prefix.join("bin")
            };
            std::fs::create_dir_all(&bindir).unwrap();
            let launcher = bindir.join(if platform == Platform::Windows {
                "codex.cmd"
            } else {
                "codex"
            });
            if platform == Platform::Windows {
                std::fs::write(
                    &launcher,
                    node_cmd_shim("node_modules/@openai/codex/bin/codex.js").replace('\n', "\r\n"),
                )
                .unwrap();
                std::fs::write(
                    bindir.join("codex"),
                    "#!/bin/sh\n# npm companion; must not be selected by cmd\n",
                )
                .unwrap();
                std::fs::write(
                    bindir.join("npm.cmd"),
                    node_cmd_shim("node_modules/npm/bin/npm-cli.js"),
                )
                .unwrap();
                write_executable(
                    &bindir.join("node.exe"),
                    "Synthetic native executable, never run.",
                );
                let npm_cli = root.join("npm/bin/npm-cli.js");
                std::fs::create_dir_all(npm_cli.parent().unwrap()).unwrap();
                std::fs::write(npm_cli, "Synthetic npm CLI, never run.").unwrap();
            } else {
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(&bin, &launcher).unwrap();
                    write_executable(&bindir.join("npm"), "Synthetic npm executable, never run.");
                }
                #[cfg(not(unix))]
                unreachable!("Unix link fixtures are Unix-only");
            }
            let search = SearchPath {
                directories: vec![bindir],
                suffixes: if platform == Platform::Windows {
                    windows_suffixes(".COM;.EXE;.BAT;.CMD;.VBS;.JS;.WSF;.MSC").unwrap()
                } else {
                    vec![String::new()]
                },
                platform,
            };
            Self {
                directory,
                prefix,
                root,
                package,
                bin,
                launcher,
                search,
            }
        }

        fn runner(&self) -> FakeRunner {
            FakeRunner {
                prefix: format!("{}\n", self.prefix.display()).into_bytes(),
                root: format!("{}\n", self.root.display()).into_bytes(),
                version: b"codex-cli 0.77.2\n".to_vec(),
                fail: None,
                calls: Vec::new(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    struct FakeRunner {
        prefix: Vec<u8>,
        root: Vec<u8>,
        version: Vec<u8>,
        fail: Option<&'static str>,
        calls: Vec<ProbeCommand>,
    }

    impl Runner for FakeRunner {
        fn run(&mut self, command: &ProbeCommand) -> io::Result<Vec<u8>> {
            assert!(
                command.program.is_absolute(),
                "Only frozen absolute programs may be run"
            );
            self.calls.push(command.clone());
            let operation = if command.args.last().is_some_and(|arg| arg == "--version") {
                "version"
            } else if command.args.iter().any(|arg| {
                arg == "prefix"
                    || Path::new(arg)
                        .file_name()
                        .is_some_and(|file| file == "npm-prefix.js")
            }) {
                "prefix"
            } else if command.args.iter().any(|arg| arg == "root") {
                "root"
            } else {
                panic!("Unexpected probe command: {command:?}")
            };
            assert_eq!(command.npm_metadata, operation != "version");
            if self.fail == Some(operation) {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "Synthetic timeout"));
            }
            Ok(match operation {
                "version" => self.version.clone(),
                "prefix" => self.prefix.clone(),
                _ => self.root.clone(),
            })
        }
    }

    fn write_executable(path: &Path, content: &str) {
        std::fs::write(path, content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn write_manifest(package: &Path, value: serde_json::Value) {
        std::fs::write(
            package.join("package.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }

    fn unknown(details: &InstallationDetails) -> bool {
        matches!(
            details.instances[0].source,
            InstallationSource::Unknown { .. }
        )
    }

    #[cfg(unix)]
    #[test]
    fn unix_npm_link_requires_matching_global_layout_and_measures_selected_version() {
        let fixture = Fixture::new(Platform::Unix);
        let mut runner = fixture.runner();
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert_eq!(details.probe.status, InstallationStatus::Installed);
        assert!(!details.ambiguous);
        assert_eq!(details.instances.len(), 1);
        let instance = &details.instances[0];
        assert_eq!(instance.path, fixture.launcher);
        assert_eq!(
            instance.resolved_path,
            Some(std::fs::canonicalize(&fixture.bin).unwrap())
        );
        assert_eq!(instance.entrypoint_path, instance.resolved_path);
        assert_eq!(instance.version.as_deref(), Some("0.77.2"));
        assert!(!unknown(&details));
        assert_eq!(runner.calls.last().unwrap().program, fixture.launcher);
        assert_eq!(
            runner.calls.last().unwrap().args,
            [OsString::from("--version")]
        );
        assert_ne!(
            instance.version.as_deref(),
            Some("99.99.99"),
            "Manifest version is not measured version"
        );
    }

    #[test]
    fn windows_npm_cmd_is_distinct_from_entrypoint_and_extensionless_companion() {
        let fixture = Fixture::new(Platform::Windows);
        let mut runner = fixture.runner();
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert_eq!(details.probe.status, InstallationStatus::Installed);
        assert!(!details.ambiguous);
        assert_eq!(details.instances.len(), 1);
        let instance = &details.instances[0];
        assert_eq!(instance.path, fixture.launcher);
        assert_eq!(
            instance.resolved_path,
            Some(std::fs::canonicalize(&fixture.launcher).unwrap())
        );
        assert_eq!(
            instance.entrypoint_path,
            Some(std::fs::canonicalize(&fixture.bin).unwrap())
        );
        assert_ne!(instance.resolved_path, instance.entrypoint_path);
        assert_eq!(instance.version.as_deref(), Some("0.77.2"));
        let version = runner.calls.last().unwrap();
        assert_eq!(
            version.program,
            std::fs::canonicalize(fixture.prefix.join("node.exe")).unwrap()
        );
        assert_eq!(
            version.args,
            [
                fixture.bin.clone().canonicalize().unwrap().into_os_string(),
                "--version".into()
            ]
        );
        assert!(
            runner
                .calls
                .iter()
                .all(|command| command.program.extension().is_some_and(|ext| ext == "exe"))
        );
    }

    #[test]
    fn windows_large_node_entrypoint_is_inspected_without_reading_the_bundle() {
        let fixture = Fixture::new(Platform::Windows);
        let mut contents = String::from("#!/usr/bin/env node\n");
        contents.push_str(&"x".repeat(FILE_LIMIT * 2));
        std::fs::write(&fixture.bin, contents).unwrap();
        let details = inspect_with(CODEX, &fixture.search, &mut fixture.runner());
        assert!(!unknown(&details));
        assert_eq!(details.probe.status, InstallationStatus::Installed);
    }

    #[test]
    fn windows_tampered_or_nonstandard_shim_is_never_executed() {
        for contents in [
            format!(
                "{}echo secret\n",
                node_cmd_shim("node_modules/@openai/codex/bin/codex.js")
            ),
            node_cmd_shim("node_modules/@openai/codex/bin/another.js"),
            "@echo off\nnode node_modules\\@openai\\codex\\bin\\codex.js %*\n".into(),
        ] {
            let fixture = Fixture::new(Platform::Windows);
            std::fs::write(&fixture.launcher, contents).unwrap();
            let mut runner = fixture.runner();
            let details = inspect_with(CODEX, &fixture.search, &mut runner);
            assert!(unknown(&details));
            assert_eq!(details.probe.status, InstallationStatus::Error);
            assert!(details.instances[0].entrypoint_path.is_none());
            assert!(
                runner
                    .calls
                    .iter()
                    .all(|command| command.args.last().is_none_or(|arg| arg != "--version"))
            );
        }
    }

    #[test]
    fn windows_pathext_order_is_preserved_and_node_wrappers_are_rejected() {
        let fixture = Fixture::new(Platform::Windows);
        write_executable(
            &fixture.prefix.join("codex.exe"),
            "Synthetic native binary, never run.",
        );
        let details = inspect_with(CODEX, &fixture.search, &mut fixture.runner());
        assert_eq!(details.instances[0].path, fixture.prefix.join("codex.exe"));
        assert!(unknown(&details));
        assert!(details.ambiguous);
        assert!(details.instances[1].version.is_none());
        std::fs::remove_file(fixture.prefix.join("node.exe")).unwrap();
        std::fs::write(
            fixture.prefix.join("node.com"),
            "unverified first interpreter",
        )
        .unwrap();
        std::fs::write(fixture.prefix.join("node.cmd"), "unverified wrapper").unwrap();
        assert!(
            fixture
                .search
                .node_for(&fixture.prefix)
                .unwrap()
                .ends_with("node.com")
        );
        std::fs::remove_file(fixture.prefix.join("node.com")).unwrap();
        assert!(fixture.search.node_for(&fixture.prefix).is_err());
        assert!(windows_suffixes(".EXE;bad").is_err());
        assert!(windows_suffixes("").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn windows_adjacent_node_rejects_canonical_batch_targets() {
        for extension in ["cmd", "bat", "CmD", "BaT"] {
            let mut fixture = Fixture::new(Platform::Windows);
            let adjacent = fixture.prefix.join("node.exe");
            let target = fixture.directory.join(format!("target.{extension}"));
            write_executable(&target, "Synthetic batch target, never execute.");
            std::fs::remove_file(&adjacent).unwrap();
            std::os::unix::fs::symlink(&target, adjacent).unwrap();
            let later = fixture.directory.join("later good node");
            std::fs::create_dir_all(&later).unwrap();
            write_executable(
                &later.join("node.exe"),
                "Synthetic valid fallback, never execute.",
            );
            fixture.search.directories.push(later);
            assert!(fixture.search.node_for(&fixture.prefix).is_err());
            let mut runner = fixture.runner();
            let details = inspect_with(CODEX, &fixture.search, &mut runner);
            assert_eq!(details.probe.status, InstallationStatus::Error);
            assert!(unknown(&details));
            assert!(
                runner.calls.is_empty(),
                "Neither npm nor version may use a batch interpreter target"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn windows_path_node_rejects_canonical_batch_targets() {
        for extension in ["cmd", "bat", "CmD", "BaT"] {
            let mut fixture = Fixture::new(Platform::Windows);
            std::fs::remove_file(fixture.prefix.join("node.exe")).unwrap();
            let bindir = fixture.directory.join("node path");
            std::fs::create_dir_all(&bindir).unwrap();
            let target = fixture.directory.join(format!("target.{extension}"));
            write_executable(&target, "Synthetic batch target, never execute.");
            std::os::unix::fs::symlink(&target, bindir.join("node.exe")).unwrap();
            fixture.search.directories.push(bindir);
            let later = fixture.directory.join("later good node");
            std::fs::create_dir_all(&later).unwrap();
            write_executable(
                &later.join("node.exe"),
                "Synthetic valid fallback, never execute.",
            );
            fixture.search.directories.push(later);
            assert!(fixture.search.node_for(&fixture.prefix).is_err());
            let mut runner = fixture.runner();
            let details = inspect_with(CODEX, &fixture.search, &mut runner);
            assert_eq!(details.probe.status, InstallationStatus::Error);
            assert!(unknown(&details));
            assert!(
                runner.calls.is_empty(),
                "PATH aliases must not return canonical batch programs"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn windows_adjacent_node_inspection_failure_does_not_fall_back_to_path() {
        let fixture = Fixture::new(Platform::Windows);
        let unavailable_directory = fixture.directory.join("not a directory");
        std::fs::write(&unavailable_directory, "Synthetic obstruction").unwrap();
        let expected =
            std::fs::symlink_metadata(unavailable_directory.join("node.exe")).unwrap_err();
        assert_ne!(expected.kind(), io::ErrorKind::NotFound);
        assert_eq!(
            fixture
                .search
                .node_for(&unavailable_directory)
                .unwrap_err()
                .kind(),
            expected.kind()
        );
        // A working PATH node exists, so accepting it would conceal the error.
        assert!(fixture.search.node_for(&fixture.prefix).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn windows_node_native_targets_preserve_case_insensitive_exe_and_com_support() {
        for adjacent in [true, false] {
            for extension in ["exe", "com", "ExE", "CoM"] {
                let mut fixture = Fixture::new(Platform::Windows);
                std::fs::remove_file(fixture.prefix.join("node.exe")).unwrap();
                let bindir = if adjacent {
                    fixture.prefix.clone()
                } else {
                    let bindir = fixture.directory.join("node path");
                    std::fs::create_dir_all(&bindir).unwrap();
                    fixture.search.directories.push(bindir.clone());
                    bindir
                };
                let target = fixture.directory.join(format!("native.{extension}"));
                write_executable(&target, "Synthetic native target, never execute.");
                std::os::unix::fs::symlink(&target, bindir.join("node.exe")).unwrap();
                let canonical = std::fs::canonicalize(target).unwrap();
                assert_eq!(fixture.search.node_for(&fixture.prefix).unwrap(), canonical);
                let mut runner = fixture.runner();
                let details = inspect_with(CODEX, &fixture.search, &mut runner);
                assert_eq!(details.probe.status, InstallationStatus::Installed);
                assert!(
                    runner
                        .calls
                        .iter()
                        .all(|command| command.program == canonical)
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn windows_native_version_aliases_cannot_select_batch_targets() {
        for extension in ["cmd", "bat", "CmD", "BaT", "exe", "com", "ExE", "CoM"] {
            let fixture = Fixture::new(Platform::Windows);
            let alias = fixture.prefix.join("codex.exe");
            let target = fixture.directory.join(format!("agent.{extension}"));
            write_executable(&target, "Synthetic version target, never execute.");
            std::os::unix::fs::symlink(&target, &alias).unwrap();
            let mut runner = fixture.runner();
            let details = inspect_with(CODEX, &fixture.search, &mut runner);
            let native =
                extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("com");
            assert_eq!(details.instances[0].path, alias);
            assert_eq!(
                details.probe.status,
                if native {
                    InstallationStatus::Installed
                } else {
                    InstallationStatus::Error
                }
            );
            let version_calls: Vec<_> = runner
                .calls
                .iter()
                .filter(|command| !command.npm_metadata)
                .collect();
            if native {
                assert_eq!(version_calls.len(), 1);
                assert_eq!(
                    version_calls[0].program,
                    std::fs::canonicalize(target).unwrap()
                );
            } else {
                assert!(
                    version_calls.is_empty(),
                    "An Agent's native alias must not execute a batch target"
                );
            }
        }
    }

    #[test]
    fn windows_distributed_npm_manager_prefix_redirection_uses_the_actual_cli() {
        let mut fixture = Fixture::new(Platform::Windows);
        let managerdir = fixture.directory.join("node distribution");
        std::fs::create_dir_all(managerdir.join("node_modules/npm/bin")).unwrap();
        std::fs::write(managerdir.join("npm.cmd"), NPM_DISTRIBUTED_CMD).unwrap();
        std::fs::write(
            managerdir.join("node_modules/npm/bin/npm-prefix.js"),
            "Synthetic npm prefix, never run",
        )
        .unwrap();
        std::fs::write(
            managerdir.join("node_modules/npm/bin/npm-cli.js"),
            "Synthetic bundled npm, never run",
        )
        .unwrap();
        std::fs::write(
            managerdir.join("node.exe"),
            "Synthetic native node, never run",
        )
        .unwrap();
        std::fs::remove_file(fixture.prefix.join("npm.cmd")).unwrap();
        fixture.search.directories.insert(0, managerdir.clone());
        let mut runner = fixture.runner();
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert!(!unknown(&details));
        assert_eq!(
            runner.calls.len(),
            4,
            "Exact npm-prefix script, selected prefix/root, exact version"
        );
        assert_eq!(runner.calls[0].args.len(), 1);
        assert_eq!(
            Path::new(&runner.calls[0].args[0]).file_name().unwrap(),
            "npm-prefix.js"
        );
        let actual_cli = std::fs::canonicalize(fixture.root.join("npm/bin/npm-cli.js")).unwrap();
        assert_eq!(runner.calls[1].args[0], actual_cli.as_os_str());
        assert_eq!(runner.calls[2].args[0], actual_cli.as_os_str());
        if let InstallationSource::Npm { manager_path, .. } = &details.instances[0].source {
            assert_eq!(manager_path, &actual_cli);
        } else {
            panic!("Expected directory association");
        }
    }

    #[cfg(unix)]
    #[test]
    fn all_distinct_targets_are_visible_but_only_selected_instance_is_executed() {
        let mut fixture = Fixture::new(Platform::Unix);
        fixture
            .search
            .directories
            .push(fixture.search.directories[0].clone());
        let alias = fixture.directory.join("same-target alias");
        std::fs::create_dir_all(&alias).unwrap();
        std::os::unix::fs::symlink(&fixture.bin, alias.join("codex")).unwrap();
        fixture.search.directories.push(alias);
        let details = inspect_with(CODEX, &fixture.search, &mut fixture.runner());
        assert_eq!(
            details.instances.len(),
            2,
            "Repeated directory is not another candidate"
        );
        assert!(
            !details.ambiguous,
            "Aliases to one canonical target are not distinct installations"
        );
        let other = fixture.directory.join("other-target");
        std::fs::create_dir_all(&other).unwrap();
        write_executable(&other.join("codex"), "Synthetic different codex, never run");
        fixture.search.directories.push(other);
        let mut runner = fixture.runner();
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert_eq!(details.instances.len(), 3);
        assert!(details.ambiguous);
        assert!(details.instances[0].active);
        assert!(
            details
                .instances
                .iter()
                .skip(1)
                .all(|instance| !instance.active && instance.version.is_none())
        );
        assert_eq!(
            runner
                .calls
                .iter()
                .filter(|command| command.args.last().is_some_and(|arg| arg == "--version"))
                .count(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn unusable_earlier_candidate_is_preserved_and_cannot_be_hidden_by_a_later_success() {
        for dangling in [false, true] {
            let mut fixture = Fixture::new(Platform::Unix);
            let bad = fixture.directory.join("earlier");
            std::fs::create_dir_all(&bad).unwrap();
            if dangling {
                std::os::unix::fs::symlink(bad.join("missing-target"), bad.join("codex")).unwrap();
            } else {
                std::fs::write(bad.join("codex"), "not executable").unwrap();
            }
            fixture.search.directories.insert(0, bad.clone());
            let mut runner = fixture.runner();
            let details = inspect_with(CODEX, &fixture.search, &mut runner);
            assert_eq!(details.probe.status, InstallationStatus::Error);
            assert_eq!(details.instances[0].path, bad.join("codex"));
            assert_eq!(details.instances.len(), 2);
            assert_eq!(details.instances[0].resolved_path.is_none(), dangling);
            assert!(runner.calls.is_empty());
        }
    }

    #[cfg(unix)]
    #[test]
    fn installed_npm_package_does_not_claim_an_unrelated_executable() {
        let fixture = Fixture::new(Platform::Unix);
        std::fs::remove_file(&fixture.launcher).unwrap();
        write_executable(&fixture.launcher, "Synthetic standalone binary, never run");
        let details = inspect_with(CODEX, &fixture.search, &mut fixture.runner());
        assert!(unknown(&details));
        assert_eq!(details.probe.status, InstallationStatus::Installed);
        assert_eq!(details.instances[0].version.as_deref(), Some("0.77.2"));
        assert!(details.instances[0].entrypoint_path.is_none());
    }

    #[test]
    fn exact_package_and_bin_mapping_are_required_and_metadata_versions_are_ignored() {
        for manifest in [
            serde_json::json!({"name":"@someone/codex", "bin":{"codex":"bin/codex.js"}}),
            serde_json::json!({"name":"@openai/codex", "bin":{"other":"bin/codex.js"}}),
            serde_json::json!({"name":"@openai/codex", "bin":{"codex":"../bin/codex.js"}}),
            serde_json::json!({"name":"@openai/codex", "bin":{"codex":"/bin/codex.js"}}),
            serde_json::json!({"name":"@openai/codex", "bin":{"codex":"C:\\bin\\codex.js"}}),
            serde_json::json!({"name":"@openai/codex", "bin":{"codex":"bin/%EVIL%.js"}}),
        ] {
            let fixture = Fixture::new(Platform::Windows);
            write_manifest(&fixture.package, manifest);
            let details = inspect_with(CODEX, &fixture.search, &mut fixture.runner());
            assert!(unknown(&details));
            assert!(details.instances[0].entrypoint_path.is_none());
            assert!(details.instances[0].version.is_none());
        }
    }

    #[cfg(unix)]
    #[test]
    fn package_bin_and_npm_link_symlink_escapes_remain_unknown() {
        let fixture = Fixture::new(Platform::Unix);
        let external = fixture.directory.join("external.js");
        write_executable(&external, "Synthetic external entrypoint, never run");
        std::fs::remove_file(&fixture.bin).unwrap();
        std::os::unix::fs::symlink(&external, &fixture.bin).unwrap();
        assert!(unknown(&inspect_with(
            CODEX,
            &fixture.search,
            &mut fixture.runner()
        )));
        let fixture = Fixture::new(Platform::Unix);
        let external = fixture.directory.join("linked-package");
        std::fs::rename(&fixture.package, &external).unwrap();
        std::os::unix::fs::symlink(&external, &fixture.package).unwrap();
        assert!(unknown(&inspect_with(
            CODEX,
            &fixture.search,
            &mut fixture.runner()
        )));
    }

    #[test]
    fn npm_path_failures_cannot_turn_into_source_proof() {
        for failure in [
            b"relative/path\n".as_slice(),
            b"/one\n/two\n",
            b"\xff\n",
            b"\n",
        ] {
            let fixture = Fixture::new(Platform::Windows);
            let mut runner = fixture.runner();
            runner.root = failure.to_vec();
            assert!(unknown(&inspect_with(CODEX, &fixture.search, &mut runner)));
        }
        let fixture = Fixture::new(Platform::Windows);
        let mut runner = fixture.runner();
        runner.root = format!("{}\n", fixture.package.display()).into_bytes();
        assert!(
            unknown(&inspect_with(CODEX, &fixture.search, &mut runner)),
            "Conflicting prefix/root is not supported"
        );
        let mut runner = fixture.runner();
        runner.fail = Some("prefix");
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert!(unknown(&details));
        assert_eq!(details.probe.status, InstallationStatus::Installed);
        assert_eq!(details.instances[0].version.as_deref(), Some("0.77.2"));
        let mut runner = fixture.runner();
        runner.root = vec![b'x'; OUTPUT_LIMIT + 1];
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert!(unknown(&details));
        assert_eq!(details.probe.status, InstallationStatus::Installed);
    }

    #[test]
    fn unsupported_npm_manager_is_never_run_and_source_remains_unknown() {
        let fixture = Fixture::new(Platform::Windows);
        std::fs::write(
            fixture.prefix.join("npm.cmd"),
            "@echo off\necho fake prefix\n",
        )
        .unwrap();
        let mut runner = fixture.runner();
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert!(unknown(&details));
        assert_eq!(runner.calls.len(), 1);
        assert_eq!(runner.calls[0].args.last().unwrap(), "--version");
        assert_eq!(details.probe.status, InstallationStatus::Installed);
        assert_eq!(details.instances[0].version.as_deref(), Some("0.77.2"));
    }

    #[test]
    fn version_contract_rejects_stderr_empty_non_utf8_and_unrelated_text() {
        for output in [
            b"".as_slice(),
            b"\xff",
            b"node 22.10.0",
            b"codex-cli 1.2.3\nSECRET=abc",
            b"codex-cli 1.2.3\x1b[0m",
            b"99.99.99",
        ] {
            assert!(parse_version("codex", output).is_none());
        }
        assert_eq!(
            parse_version("codex", b"codex-cli 0.77.2-alpha.3+build.1\r\n").as_deref(),
            Some("0.77.2-alpha.3+build.1")
        );
        assert_eq!(
            parse_version("claude", b"2.0.30 (Claude Code)\n").as_deref(),
            Some("2.0.30")
        );
        assert_eq!(
            parse_version("opencode", b"1.2.3\n").as_deref(),
            Some("1.2.3")
        );
        let fixture = Fixture::new(Platform::Windows);
        let mut runner = fixture.runner();
        runner.version = b"node 22.10.0\n".to_vec();
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert_eq!(details.probe.status, InstallationStatus::Installed);
        assert!(details.instances[0].version.is_none());
        assert!(details.probe.message.is_some());
        let mut runner = fixture.runner();
        runner.fail = Some("version");
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert_eq!(details.probe.status, InstallationStatus::Error);
        assert!(details.instances[0].version.is_none());
        assert!(
            !unknown(&details),
            "Directory association is independent of version probe success"
        );
    }

    #[test]
    fn inspection_wire_contract_is_camel_case_with_an_explicit_source_kind() {
        let fixture = Fixture::new(Platform::Windows);
        let details = inspect_with(CODEX, &fixture.search, &mut fixture.runner());
        let value = serde_json::to_value(&details).unwrap();
        let instance = &value["instances"][0];
        assert!(instance.get("resolvedPath").is_some());
        assert!(instance.get("entrypointPath").is_some());
        assert_eq!(instance["source"]["kind"], "npm");
        assert!(instance["source"].get("packageRoot").is_some());
        assert!(instance["source"].get("managerPath").is_some());
        assert_eq!(
            serde_json::from_value::<InstallationDetails>(value).unwrap(),
            details
        );
    }

    #[test]
    fn missing_candidates_never_run_a_package_manager_or_version_probe() {
        let fixture = Fixture::new(Platform::Windows);
        std::fs::remove_file(&fixture.launcher).unwrap();
        let mut runner = fixture.runner();
        let details = inspect_with(CODEX, &fixture.search, &mut runner);
        assert_eq!(details.probe.status, InstallationStatus::NotDetected);
        assert!(details.instances.is_empty());
        assert!(runner.calls.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn direct_runner_bounds_stdout_stderr_exit_and_descendant_lifetime() {
        let fixture = Fixture::new(Platform::Unix);
        let script = fixture.directory.join("quoted ' \" ; $(never) probe");
        write_executable(
            &script,
            "#!/bin/sh\nprintf 'codex-cli 1.2.3\\n'\nprintf 'sensitive stderr' >&2\n",
        );
        let command = ProbeCommand {
            npm_metadata: false,
            program: script.clone(),
            args: vec!["--version".into()],
        };
        assert_eq!(
            run_bounded(&command, Duration::from_secs(1), 64).unwrap(),
            b"codex-cli 1.2.3\n"
        );
        write_executable(&script, "#!/bin/sh\nprintf 'sensitive stderr' >&2\n");
        assert!(
            run_bounded(&command, Duration::from_secs(1), 64)
                .unwrap()
                .is_empty()
        );
        write_executable(&script, "#!/bin/sh\nprintf 'too much output'\n");
        assert!(run_bounded(&command, Duration::from_secs(1), 4).is_err());
        write_executable(&script, "#!/bin/sh\nexit 9\n");
        assert!(run_bounded(&command, Duration::from_secs(1), 64).is_err());
        let marker = fixture.directory.join("descendant-marker");
        write_executable(
            &script,
            "#!/bin/sh\n(sleep 0.3; printf late > \"$1\") &\nexit 0\n",
        );
        let command = ProbeCommand {
            npm_metadata: false,
            program: script,
            args: vec![marker.as_os_str().to_owned()],
        };
        let result = run_bounded(&command, Duration::from_millis(40), 64).unwrap_err();
        assert_eq!(result.kind(), io::ErrorKind::TimedOut);
        std::thread::sleep(Duration::from_millis(400));
        assert!(
            !marker.exists(),
            "A descendant retaining stdout survived probe timeout"
        );
    }

    #[cfg(windows)]
    fn powershell_command(fixture: &Fixture, code: &str, name: &str) -> ProbeCommand {
        let executable =
            PathBuf::from(std::env::var_os("SystemRoot").expect("Windows system root"))
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let script = fixture.directory.join(format!("{name}.ps1"));
        std::fs::write(&script, code).unwrap();
        ProbeCommand {
            npm_metadata: false,
            program: executable,
            args: vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-File".into(),
                script.into_os_string(),
            ],
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_native_runner_bounds_output_and_reports_launch_failure() {
        let fixture = Fixture::new(Platform::Windows);
        let missing = ProbeCommand {
            npm_metadata: false,
            program: fixture.directory.join("missing.exe"),
            args: Vec::new(),
        };
        assert!(run_bounded(&missing, Duration::from_secs(10), 64).is_err());
        let invalid = ProbeCommand {
            npm_metadata: false,
            program: fixture.prefix.join("node.exe"),
            args: Vec::new(),
        };
        assert!(run_bounded(&invalid, Duration::from_secs(10), 64).is_err());
        let command = powershell_command(
            &fixture,
            "[Console]::Out.Write('codex-cli 1.2.3'); [Console]::Error.Write('sensitive stderr')",
            "normal",
        );
        assert_eq!(
            run_bounded(&command, Duration::from_secs(15), 64).unwrap(),
            b"codex-cli 1.2.3"
        );
        let command = powershell_command(
            &fixture,
            "[Console]::Out.Write('too much output')",
            "overflow",
        );
        assert!(run_bounded(&command, Duration::from_secs(15), 4).is_err());
        let command = powershell_command(&fixture, "exit 9", "failure");
        assert!(run_bounded(&command, Duration::from_secs(15), 64).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_job_terminates_grandchild_retaining_stdout_after_parent_exits() {
        let fixture = Fixture::new(Platform::Windows);
        let ready = fixture.directory.join("child-ready.txt");
        let marker = fixture.directory.join("late-write.txt");
        // The direct child exits immediately. Its grandchild inherits stdout and
        // would write later unless closing the private job really terminates it.
        // Fixture paths are passed as argv and child environment, never shell text.
        let code = r#"
$ErrorActionPreference = 'Stop'
$childCode = '[IO.File]::WriteAllText($env:AGENTDECK_READY, [string]$PID); Start-Sleep -Seconds 10; [IO.File]::WriteAllText($env:AGENTDECK_MARKER, "late")'
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($childCode))
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = [IO.Path]::Combine($env:SystemRoot, 'System32\WindowsPowerShell\v1.0\powershell.exe')
$info.Arguments = '-NoLogo -NoProfile -NonInteractive -EncodedCommand ' + $encoded
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.EnvironmentVariables['AGENTDECK_READY'] = $args[0]
$info.EnvironmentVariables['AGENTDECK_MARKER'] = $args[1]
$process = New-Object System.Diagnostics.Process
$process.StartInfo = $info
if (-not $process.Start()) { throw 'fixture child did not start' }
exit 0
"#;
        let mut command = powershell_command(&fixture, code, "spawn child");
        command.args.push(ready.as_os_str().to_owned());
        command.args.push(marker.as_os_str().to_owned());
        let error = run_bounded(&command, Duration::from_secs(8), 64).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        let child_id: u32 = std::fs::read_to_string(&ready)
            .expect("Real descendant must start before timeout")
            .trim()
            .parse()
            .unwrap();
        // Verify process termination directly, then wait past its late-write window.
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, child_id) };
        if !process.is_null() {
            let mut code = 259; // STILL_ACTIVE
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut success = 1;
            while code == 259 && Instant::now() < deadline {
                success = unsafe { GetExitCodeProcess(process, &mut code) };
                if success == 0 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            unsafe {
                CloseHandle(process);
            }
            assert_ne!(success, 0);
            assert_ne!(code, 259, "Grandchild survived closing the probe job");
        }
        std::thread::sleep(Duration::from_secs(11));
        assert!(
            !marker.exists(),
            "Timed-out Windows descendant performed a late write"
        );
    }
}
