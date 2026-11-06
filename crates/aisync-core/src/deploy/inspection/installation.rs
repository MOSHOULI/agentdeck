//! Install-only npm adapter. Observations never authorize a reinstall, upgrade,
//! uninstall, or rollback. A plan is ephemeral and must be rebuilt before use.
use super::*;
use crate::registry::InstallMethod;
use sha2::{Digest, Sha256};

const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);
const MANAGER_FILE_LIMIT: usize = 256 * 1024 * 1024;

fn adapter(agent: &AgentDefinition) -> Option<KnownAgent> {
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

pub(crate) fn install_agent(agent: &AgentDefinition) -> crate::Result<()> {
    // Serialize this application's installers, including direct IPC calls. This
    // is not a lock against other package managers or another OS process.
    static INSTALL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = INSTALL_LOCK.try_lock().map_err(|_| {
        crate::Error::InstallFailed("Another installation is already running".into())
    })?;
    let known = adapter(agent).ok_or_else(|| {
        crate::Error::InstallFailed(
            "No supported safe installation adapter; use the original official installation method"
                .into(),
        )
    })?;
    let search = SearchPath::from_environment().map_err(|_| {
        crate::Error::InstallFailed("Cannot inspect the executable search path".into())
    })?;
    install_with(known, &search, &mut ProcessRunner, |command| {
        run_bounded(command, INSTALL_TIMEOUT, 1024 * 1024).map(|_| ())
    })
    .map_err(crate::Error::InstallFailed)
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
    let manager = frozen_manager(search, runner)?;
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
    let package = known.package.ok_or("No supported npm package")?;
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
    Ok(InstallPlan {
        manager,
        prefix,
        root,
        package,
    })
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
    if plan.package == "@earendil-works/pi-coding-agent" {
        command.args.push("--ignore-scripts".into());
    }
    command.args.push(plan.package.into());
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
            Ok(())
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
            if command.args.last().is_some_and(|arg| arg == "--version") {
                return Ok(self.version.clone());
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
            assert_eq!(command.args.last().unwrap(), "@openai/codex");
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
    fn safe_install_capability_requires_exact_adapter_recipe_and_pi_disables_scripts() {
        let registry =
            crate::registry::parse_registry(include_str!("../../../../../agents/registry-v3.json"))
                .unwrap();
        for id in ["codex", "claude-code", "opencode", "gemini-cli", "pi"] {
            let mut agent = registry.agents[id].clone();
            assert!(can_install(&agent), "bundled {id} recipe must be supported");
            agent.install.command = Some("npm install -g unexpected-package".into());
            assert!(!can_install(&agent));
            assert!(install_agent(&agent).is_err());
        }
        let fixture = Fixture::new(Platform::Windows);
        let mut plan = build_plan(CODEX, &fixture.search, &mut fixture.runner()).unwrap();
        plan.package = "@earendil-works/pi-coding-agent";
        assert!(
            install_command(&plan)
                .args
                .iter()
                .any(|arg| arg == "--ignore-scripts")
        );
    }
}
