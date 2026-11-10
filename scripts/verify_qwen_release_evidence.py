#!/usr/bin/env python3
"""Hash-pinned static distribution checks only. Never extract or execute package code."""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / 'tests/fixtures/qwen-reviewed-releases/evidence.json'
READER = ROOT / 'tests/fixtures/qwen-0.27.0/read-config-file.ts'

def pinned_releases():
    data = json.loads(EVIDENCE.read_text(encoding='utf-8'))
    if data['schemaVersion'] != 1 or set(data['releases']) != {'0.27.0', '0.27.2'}:
        raise ValueError('unsupported evidence contract')
    for version, item in data['releases'].items():
        if item['tarball'] != f'https://registry.npmjs.org/@qwen-code/qwen-code/-/qwen-code-{version}.tgz':
            raise ValueError('unapproved download URL')
        if hashlib.sha256(READER.read_bytes()).hexdigest() != item['readerSha256']:
            raise ValueError('reader fixture hash mismatch')
    return data['releases']

def verify_archive(content, version, evidence):
    if len(content) != evidence['archiveBytes'] or base64.b64encode(hashlib.sha512(content).digest()).decode() != evidence['sha512']:
        raise ValueError('archive integrity mismatch')
    payloads = {}
    with tarfile.open(fileobj=io.BytesIO(content), mode='r:gz') as archive:
        # No filesystem extraction, package import, process or npm execution.
        for label, expected in evidence['members'].items():
            matches = [m for m in archive.getmembers() if m.name == expected['path']]
            if len(matches) != 1 or not matches[0].isfile() or matches[0].size != expected['bytes']:
                raise ValueError('unexpected selected member metadata')
            payload = archive.extractfile(matches[0]).read(expected['bytes'] + 1)
            if len(payload) != expected['bytes'] or hashlib.sha256(payload).hexdigest() != expected['sha256']:
                raise ValueError('selected member integrity mismatch')
            payloads[label] = payload
    manifest = json.loads(payloads['manifest'])
    if manifest['name'] != '@qwen-code/qwen-code' or manifest['version'] != version or manifest['engines']['node'] != '>=22.0.0':
        raise ValueError('unexpected package declaration')
    schema = b'vimMode:{type:"boolean",label:"Vim Mode",category:"General",requiresRestart:false,default:false,description:"Enable Vim keybindings",showInDialog:true}'
    loader = b'JSON.parse((0,import_strip_json_comments.default)(stripUtf8Bom(content)))'
    if schema not in payloads['schema'] or b'SETTINGS_VERSION=4;' not in payloads['loader'] or loader not in payloads['loader'] or b'function readConfigFile(' not in payloads['loader']:
        raise ValueError('reviewed static contract changed')
    return {'version': version, 'archiveSha512': evidence['sha512'], 'selectedMembersVerified': True,
            'agentExecuted': False, 'fullLoaderExecuted': False, 'effectiveSettingsVerified': False}

def download(url, limit):
    with urllib.request.urlopen(url, timeout=45) as response:
        if response.geturl() != url:
            raise ValueError('unexpected redirect')
        content = response.read(limit + 1)
        if len(content) > limit:
            raise ValueError('download exceeds pinned bound')
        return content

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--download', action='store_true', help='download exact pinned official archives and reader sources; never execute')
    parser.add_argument('--archive-directory', type=Path, help='directory containing qwen-code-VERSION-review.tgz')
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    if args.download == bool(args.archive_directory):
        parser.error('choose exactly one of --download or --archive-directory')
    reports = []
    for version, evidence in pinned_releases().items():
        if args.download:
            content = download(evidence['tarball'], evidence['archiveBytes'])
        else:
            with (args.archive_directory / f'qwen-code-{version}-review.tgz').open('rb') as archive:
                content = archive.read(evidence['archiveBytes'] + 1)
        result = verify_archive(content, version, evidence)
        result['officialReaderSourceVerified'] = False
        if args.download:
            url = f"https://raw.githubusercontent.com/QwenLM/qwen-code/{evidence['gitHead']}/packages/cli/src/config/read-config-file.ts"
            source = download(url, 8192)
            if hashlib.sha256(source).hexdigest() != evidence['readerSha256'] or source != READER.read_bytes():
                raise ValueError('official reader source differs from tested fixture')
            result['officialReaderSourceVerified'] = True
        reports.append(result)
    output = json.dumps({'staticEvidenceOnly': True, 'releases': reports}, indent=2) + '\n'
    if args.report:
        args.report.write_text(output, encoding='utf-8')
    print(output, end='')

if __name__ == '__main__':
    main()
