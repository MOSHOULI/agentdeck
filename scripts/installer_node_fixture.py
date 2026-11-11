"""Measure an explicitly supplied trusted Node for authored install acceptance.

This is the fixture's independent expected value, not an agent compatibility test.
Call only after the platform fixture has checked its native executable path/header.
"""
import os
import re
import subprocess


def measured_version(node):
    env = {key: value for key, value in os.environ.items()
           if key.lower() not in ('node_options', 'node_path', 'npm_config_node_options')}
    result = subprocess.run([str(node), '--version'], env=env, capture_output=True,
                            timeout=5, check=False)
    # The selected trusted native program emits a tiny single line; don't retain
    # its diagnostics or accept extra stdout as a version.
    if result.returncode != 0 or len(result.stdout) > 64:
        raise ValueError('installer_node_version_unavailable')
    value = result.stdout[:-2] if result.stdout.endswith(b'\r\n') else result.stdout.removesuffix(b'\n')
    if not re.fullmatch(rb'v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', value):
        raise ValueError('installer_node_version_invalid')
    numbers = tuple(map(int, value[1:].split(b'.')))
    if any(n > 4294967295 for n in numbers) or numbers < (18, 0, 0):
        raise ValueError('installer_node_below_fixture_policy')
    return value.decode('ascii')
