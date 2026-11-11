#!/usr/bin/env python3
"""Install fixed hosted-CI dependencies using signed, canonical Ubuntu sources.

Never edits system sources. Network retries are apt's bounded acquisition retries;
an interrupted/failed package transaction is never repeated by this helper.
"""
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

PACKAGES = {
    'tracer': ('strace',),
    'glib': ('libglib2.0-dev', 'pkg-config'),
    'webview': ('libwebkit2gtk-4.1-dev', 'libgtk-3-dev',
                'libayatana-appindicator3-dev', 'webkit2gtk-driver',
                'xvfb', 'xauth', 'dbus-x11', 'strace'),
}
KEYRING = '/usr/share/keyrings/ubuntu-archive-keyring.gpg'
SOURCES = f'''Types: deb
URIs: https://archive.ubuntu.com/ubuntu
Suites: noble noble-updates noble-backports
Components: main restricted universe multiverse
Architectures: amd64
Signed-By: {KEYRING}

Types: deb
URIs: https://security.ubuntu.com/ubuntu
Suites: noble-security
Components: main restricted universe multiverse
Architectures: amd64
Signed-By: {KEYRING}
'''


def check_context(env, release, machine, architecture):
    if (env.get('GITHUB_ACTIONS'), env.get('RUNNER_ENVIRONMENT'), env.get('RUNNER_OS')) != ('true', 'github-hosted', 'Linux'):
        raise ValueError('requires GitHub-hosted Linux CI')
    if (release.get('ID'), release.get('VERSION_ID'), release.get('VERSION_CODENAME')) != ('ubuntu', '24.04', 'noble'):
        raise ValueError('only reviewed Ubuntu 24.04 noble is supported')
    if machine != 'x86_64' or architecture != 'amd64':
        raise ValueError('only reviewed amd64 is supported')
    if 'APT_CONFIG' in env:
        raise ValueError('APT_CONFIG override is not supported')


def commands(root, group):
    packages = PACKAGES[group]  # No user-controlled package/option arguments.
    options = [
        '-o', f'Dir::Etc::sourcelist={root / "ubuntu.sources"}',
        '-o', 'Dir::Etc::sourceparts=-',
        '-o', f'Dir::State::lists={root / "lists"}',
        '-o', 'Dir::Cache::pkgcache=', '-o', 'Dir::Cache::srcpkgcache=',
        '-o', 'APT::Update::Error-Mode=any',
        '-o', 'Acquire::Retries=2',
        '-o', 'Acquire::http::Timeout=20', '-o', 'Acquire::https::Timeout=20',
    ]
    def command(seconds, args):
        # Root timeout can terminate root apt/dpkg descendants, unlike a timeout
        # in this unprivileged Python process. Never retry a timed-out install.
        return ['/usr/bin/sudo', '--non-interactive', '/usr/bin/timeout',
                '--signal=TERM', '--kill-after=15s', f'{seconds}s',
                '/usr/bin/apt-get', *options, *args]
    return [command(180, ['update']),
            command(600, ['install', '--no-install-recommends', '-y', *packages])]


def install(root, group, run=subprocess.run):
    root.chmod(0o755)  # apt's _apt downloader needs to traverse this directory.
    (root / 'lists').mkdir(mode=0o755)
    (root / 'ubuntu.sources').write_text(SOURCES, encoding='utf-8')
    (root / 'ubuntu.sources').chmod(0o644)
    for command in commands(root, group):
        result = run(command, check=False)
        if result.returncode != 0:
            # In particular, never proceed after partial update / stale-index
            # fallback, signature failure, or a timed-out package transaction.
            return result.returncode
    return 0


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) != 1 or argv[0] not in PACKAGES:
        raise ValueError('expected one fixed dependency group: tracer, glib, webview')
    check_context(os.environ, platform.freedesktop_os_release(), platform.machine(),
                  subprocess.check_output(['/usr/bin/dpkg', '--print-architecture'], text=True).strip())
    if not Path(KEYRING).is_file():
        raise ValueError('Ubuntu archive signing keyring is missing')
    # apt owns some generated list files. Retain this unique directory for the
    # ephemeral runner's lifetime instead of adding a privileged cleanup command.
    root = Path(tempfile.mkdtemp(prefix='agentdeck-ci-ubuntu-'))
    print(f'Installing {argv[0]} from signed Ubuntu noble sources; indexes: {root}', flush=True)
    return install(root, argv[0])


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f'Ubuntu CI dependency setup failed: {error}', file=sys.stderr)
        sys.exit(1)
