#!/usr/bin/env python3
"""Exercise the unmodified official Qwen settings snapshot with synthetic layers.

Requires Linux bubblewrap network namespaces; no non-isolated fallback exists.
Never invokes Qwen main, authentication, hooks, MCP or a model prompt.
"""
import argparse
import importlib.util
import json
import pathlib
import shutil
import subprocess
import tempfile

spec = importlib.util.spec_from_file_location('distribution', pathlib.Path(__file__).with_name('test-qwen-distribution.py'))
distribution = importlib.util.module_from_spec(spec)
spec.loader.exec_module(distribution)


def run(command, home, env):
    result = subprocess.run(command, cwd=home, env=env, capture_output=True, text=True, timeout=45)
    if result.returncode or result.stderr:
        raise RuntimeError(f'Isolated test command failed: {result.stderr[:2000]}')
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=pathlib.Path, required=True)
    parser.add_argument('--aisync', type=pathlib.Path, required=True)
    args = parser.parse_args()
    bwrap = shutil.which('bwrap')
    node = shutil.which('node')
    if not bwrap or not node:
        raise RuntimeError('Linux bubblewrap and Node 24+ required')
    node = str(pathlib.Path(node).resolve())
    binary = str(args.aisync.resolve(strict=True))
    runner = pathlib.Path(__file__).with_name('qwen-settings-snapshot.mjs').resolve()
    with tempfile.TemporaryDirectory(prefix='agentdeck-qwen-settings-') as directory:
        root = pathlib.Path(directory)
        home = root / 'home'
        home.mkdir()
        env = distribution.environment(home)
        prefix = [bwrap, '--unshare-net', '--die-with-parent', '--ro-bind', '/', '/', '--bind', str(home), str(home)]
        # Fail before extracting/importing the distribution if namespace setup is denied.
        version = run(prefix + [node, '--version'], home, env).strip()
        if int(version.lstrip('v').split('.')[0]) < 24:
            raise RuntimeError('Node 24+ required')
        install = root / 'install'
        install.mkdir()
        package = distribution.unpack(args.archive.resolve(), install)
        workspace = home / 'workspace'
        (workspace / '.qwen').mkdir(parents=True)
        user = home / '.qwen'
        user.mkdir()
        settings = user / 'settings.json'
        system = home / 'system.json'
        defaults = home / 'defaults.json'
        env.update({'QWEN_CODE_SYSTEM_SETTINGS_PATH': str(system), 'QWEN_CODE_SYSTEM_DEFAULTS_PATH': str(defaults)})
        original = {'$version': 4, 'general': {'vimMode': False, 'language': 'en'}}
        system.write_text('{"$version":4}')
        defaults.write_text('{"$version":4,"general":{"vimMode":true}}')
        manifest = package / 'package.json'

        def snapshot(expected, trusted=True):
            before = {p: p.read_bytes() for p in home.rglob('*') if p.is_file()}
            command = prefix + [node, '--permission', '--allow-fs-read=' + str(install),
                                '--allow-fs-read=' + str(home), '--allow-fs-read=' + str(runner),
                                str(runner), str(package), str(workspace), str(expected).lower(), str(trusted).lower()]
            value = json.loads(run(command, home, env))
            after = {p: p.read_bytes() for p in home.rglob('*') if p.is_file()}
            if before != after:
                raise AssertionError('Settings snapshot changed synthetic files')
            return value

        def aisync(action, extra=()):
            return json.loads(run([binary, 'qwen', action, '--settings', str(settings),
                                   '--runtime-manifest', str(manifest), *extra], home, env))

        snapshot(True)  # With no user file, the real default layer must be loaded.
        settings.write_text(json.dumps(original))
        snapshot(False)  # User false overrides the observed system default true.
        plan = aisync('vim-preview', ['--enabled', 'true'])
        aisync('vim-apply', ['--enabled', 'true', '--preview-token', plan['token'], '--yes'])
        snapshot(True)  # New process, actual official loader, no reconstructed parser.
        (workspace / '.qwen/settings.json').write_text('{"$version":4,"general":{"vimMode":false}}')
        snapshot(False)  # Trusted workspace overrides user.
        snapshot(True, False)  # Untrusted workspace is excluded.
        aisync('vim-restore', ['--yes'])
        snapshot(False, False)
        if json.loads(settings.read_text()) != original:
            raise AssertionError('Restored user settings differ')
        print(json.dumps({'package': '@qwen-code/qwen-code', 'version': distribution.VERSION,
                          'node': version, 'network_isolation': 'bwrap',
                          'settings_snapshot_apply_restore': True, 'layers_verified': True,
                          'interactive_vim_tested': False, 'authenticated': False, 'model_called': False}))


if __name__ == '__main__':
    main()
