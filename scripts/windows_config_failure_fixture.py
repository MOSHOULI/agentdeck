#!/usr/bin/env python3
"""Authored version-only packages; no registry, installation or official agent code.

Fixture setup is outside the product. Installed-state evidence comes from actual
production PATH/package inspection and version probes, never injected UI state.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
from windows_install_fixture import prepare, shim, write_new, safe_file

AGENTS = (('claude', '@anthropic-ai/claude-code', '2.0.30 (Claude Code)', '.claude/CLAUDE.md'),
          ('gemini', '@google/gemini-cli', '1.2.3', '.gemini/GEMINI.md'))


def program(log, version):
    # Required by the production Windows npm entrypoint inspector, even though
    # invoking Node directly would accept a script without this interpreter line.
    return ('#!/usr/bin/env node\n'
            '"use strict";const fs=require("node:fs");\n'
            'const args=process.argv.slice(2);\n'
            'fs.appendFileSync(' + json.dumps(str(log)) + ',JSON.stringify(args)+"\\n");\n'
            'if(JSON.stringify(args)!==\'["--version"]\'){process.exit(64);}\n'
            'console.log(' + json.dumps(version) + ');\n').encode()


def setup(root, node, home):
    manifest = prepare(root, node, home, 'success')
    m = json.loads(manifest.read_text(encoding='utf-8'))
    authored = {}
    targets = []
    for binary, package, version, instruction in AGENTS:
        folder = Path(m['globalRoot']) / package
        folder.mkdir(parents=True)
        log = root / (binary + '-versions.jsonl')
        write_new(log, b'')
        files = {
            folder / 'package.json': json.dumps({'name': package, 'version': version.split()[0], 'bin': {binary: 'cli.js'}}).encode(),
            folder / 'cli.js': program(log, version),
            Path(m['prefix']) / (binary + '.cmd'): shim('node_modules/' + package + '/cli.js'),
        }
        for path, data in files.items():
            write_new(path, data)
            authored[str(path)] = hashlib.sha256(data).hexdigest()
        target = home / instruction
        target.parent.mkdir(exist_ok=False)
        settings = target.parent / 'settings.json'
        write_new(target, b'# Authored instructions\r\n<!-- agentdeck:personalization:start -->\r\nIncomplete block must survive.\r\n')
        write_new(settings, b'{ "fixture": "SYNTHETIC-NOT-A-CREDENTIAL", "preserve": true }\r\n')
        for path in (target, settings):
            os.utime(path, ns=(946684800000000000, 946684800000000000))
            targets.append(str(path))
    authored[m['node']] = m['nodeSha256']
    authored[m['manager']] = m['managerSha256']
    m.update(configurationTargets=targets, authored=authored)
    # This private manifest is setup data, never a product Installed-state override.
    result = root / 'configuration-fixture.json'
    write_new(result, json.dumps(m).encode())
    return result


def prepare_hardlinks(m):
    """After malformed-marker stages, author valid content and aliases while App is stopped."""
    targets = list(m['configurationTargets'])
    aliases = []
    for _, _, _, instruction in AGENTS:
        target = Path(m['home']) / instruction
        safe_file(target, 65536)
        alias = Path(m['root']) / (target.parent.name + '-instructions-alias.md')
        if os.path.lexists(alias):
            raise ValueError('alias_exists')
        target.write_bytes(b'# Valid authored instructions\r\nPreserve this shared file.\r\n')
        os.link(target, alias)
        os.utime(target, ns=(946684800000000000, 946684800000000000))
        aliases.append(str(alias))
    m['configurationTargets'] = targets + aliases
    m['hardlinkAliases'] = aliases
    return m


def snapshot(paths):
    result = []
    for value in paths:
        path = Path(value)
        s = path.lstat()
        if not stat.S_ISREG(s.st_mode) or s.st_size > 65536 or getattr(s, 'st_file_attributes', 0) & 0x400:
            raise ValueError('unsafe_configuration_fixture')
        data = path.read_bytes()
        if not s.st_ino or s.st_nlink not in (1, 2):
            raise ValueError('file_identity_unavailable')
        result.append(dict(hash=hashlib.sha256(data).hexdigest(), size=s.st_size,
                           device=s.st_dev, fileId=s.st_ino, links=s.st_nlink,
                           createdNs=s.st_ctime_ns, modifiedNs=s.st_mtime_ns,
                           mode=stat.S_IMODE(s.st_mode), attributes=getattr(s, 'st_file_attributes', 0)))
    return result


def verify(m):
    for path, digest in m['authored'].items():
        if hashlib.sha256(safe_file(Path(path), 256*1024*1024)).hexdigest() != digest:
            raise ValueError('authored_runtime_changed')
    if safe_file(Path(m['installCalls']), 4096):
        raise ValueError('unexpected_install')
    counts = {}
    for binary, *_ in AGENTS:
        rows = safe_file(Path(m['root']) / (binary + '-versions.jsonl'), 65536).splitlines()
        if not rows or any(json.loads(row) != ['--version'] for row in rows):
            raise ValueError('missing_or_nonversion_invocation')
        counts[binary] = len(rows)
    return counts


def main():
    p = argparse.ArgumentParser()
    p.add_argument('action', choices=('prepare', 'prepare-hardlinks', 'snapshot', 'verify'))
    p.add_argument('--root', type=Path)
    p.add_argument('--node', type=Path)
    p.add_argument('--home', type=Path)
    p.add_argument('--manifest', type=Path)
    args = p.parse_args()
    if os.name != 'nt':
        raise ValueError('requires_windows')
    if args.action == 'prepare':
        print(setup(args.root, args.node, args.home))
    else:
        m = json.loads(safe_file(args.manifest, 65536))
        if args.action == 'prepare-hardlinks':
            m = prepare_hardlinks(m)
            args.manifest.write_text(json.dumps(m), encoding='utf-8')
            print(json.dumps({'prepared': True}))
        else:
            print(json.dumps(snapshot(m['configurationTargets']) if args.action == 'snapshot' else verify(m)))


if __name__ == '__main__':
    try:
        main()
    except Exception:
        # Never expose paths or authored contents on failure.
        raise SystemExit('Configuration fixture validation failed') from None
