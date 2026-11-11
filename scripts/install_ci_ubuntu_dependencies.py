#!/usr/bin/env python3
"""Install fixed hosted-CI dependencies using the hosted runner's configured sources.

Never edits system sources. Network retries are apt's bounded acquisition retries;
an interrupted/failed package transaction is never repeated by this helper.
"""
import os
from pathlib import Path
import platform
import subprocess
import sys

PACKAGES = {
    'tracer': ('strace',),
    'glib': ('libglib2.0-dev', 'pkg-config'),
    'webview': ('libwebkit2gtk-4.1-dev', 'libgtk-3-dev',
                'libayatana-appindicator3-dev', 'webkit2gtk-driver',
                'xvfb', 'xauth', 'dbus-x11', 'strace'),
}
KEYRING = '/usr/share/keyrings/ubuntu-archive-keyring.gpg'


def check_context(env, release, machine, architecture):
    if (env.get('GITHUB_ACTIONS'), env.get('RUNNER_ENVIRONMENT'), env.get('RUNNER_OS')) != ('true', 'github-hosted', 'Linux'):
        raise ValueError('requires GitHub-hosted Linux CI')
    if (release.get('ID'), release.get('VERSION_ID'), release.get('VERSION_CODENAME')) != ('ubuntu', '24.04', 'noble'):
        raise ValueError('only reviewed Ubuntu 24.04 noble is supported')
    if machine != 'x86_64' or architecture != 'amd64':
        raise ValueError('only reviewed amd64 is supported')
    if 'APT_CONFIG' in env:
        raise ValueError('APT_CONFIG override is not supported')


def commands(group):
    packages = PACKAGES[group]  # No user-controlled package/option arguments.
    options = [
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


def install(group, run=subprocess.run):
    for command in commands(group):
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
    print(f'Installing {argv[0]} using the hosted Ubuntu runner sources and trust configuration', flush=True)
    return install(argv[0])


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f'Ubuntu CI dependency setup failed: {error}', file=sys.stderr)
        sys.exit(1)
