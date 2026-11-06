#!/usr/bin/env python3
"""Verify the narrowly scoped GLib backport, without building or using the network.

Python 3.11+ only; no third-party modules. CI must supply the official --archive.
--metadata takes full `cargo metadata --locked --format-version 1` output, never
--no-deps. --audit-lock-output creates a NEW, private, audit-only lockfile: cargo
audit 0.22.2 excludes path dependencies, so its GLib identity is restored to the
original registry release solely for advisory visibility. Never build with this
projection or replace Cargo.lock with it. --audit-json requires the advisory to
remain visible; it is not an advisory suppression or a clean-audit assertion.
"""

from __future__ import annotations

import argparse
from collections import deque
import difflib
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tarfile
import tomllib


VERSION = "0.18.5"
ADVISORY = "RUSTSEC-2024-0429"
ARCHIVE_URL = "https://static.crates.io/crates/glib/glib-0.18.5.crate"
ARCHIVE_SHA256 = "233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5"
UPSTREAM_FIX = "https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34"
PATCHED_FILE = "src/variant_iter.rs"
PATCHED_SHA256 = "a0f5ee8acb8faa089bcdfbc9a57372609fce7654026ccef7d9a224d05a654ccc"
# Digest of the sorted, compact JSON original_files map from the official crate.
# This also anchors the inventory when --archive is deliberately not supplied.
INVENTORY_SHA256 = "d59db0cd23975279aa057124dd43c47d26ba742df34e9dbfba0e72099d71e217"
FILE_COUNT = 121
REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
ORIGINAL_DECLARATION = b"            let p: *mut libc::c_char = std::ptr::null_mut();\n"
PATCHED_DECLARATION = b"            let mut p: *mut libc::c_char = std::ptr::null_mut();\n"
ORIGINAL_ARGUMENT = b"                &p,\n"
PATCHED_ARGUMENT = b"                &mut p,\n"


class VerificationError(ValueError):
    """An input does not satisfy the pinned backport contract."""


def _require(condition, message):
    if not condition:
        raise VerificationError(message)


def _sha256(data):
    return hashlib.sha256(data).hexdigest()


def _json(value):
    if isinstance(value, dict):
        return value
    return json.loads(Path(value).read_text(encoding="utf-8"))


def _safe_relative(name):
    return (
        isinstance(name, str)
        and bool(name)
        and "\\" not in name
        and "\x00" not in name
        and not PurePosixPath(name).is_absolute()
        and all(part not in ("", ".", "..") for part in name.split("/"))
    )


def _provenance(root):
    data = _json(root / "vendor/glib-backport.json")
    pinned = {
        "schema_version": 1,
        "name": "glib",
        "version": VERSION,
        "archive_url": ARCHIVE_URL,
        "archive_sha256": ARCHIVE_SHA256,
        "advisory": ADVISORY,
        "upstream_fix": UPSTREAM_FIX,
        "patched_file": PATCHED_FILE,
        "patched_sha256": PATCHED_SHA256,
    }
    _require(set(data) == set(pinned) | {"original_files"}, "unexpected provenance fields")
    for key, expected in pinned.items():
        _require(type(data[key]) is type(expected) and data[key] == expected,
                 f"provenance {key} does not match the pinned value")
    originals = data["original_files"]
    _require(isinstance(originals, dict) and len(originals) == FILE_COUNT,
             f"original inventory must contain exactly {FILE_COUNT} files")
    for name, digest in originals.items():
        _require(_safe_relative(name), f"unsafe inventory path: {name!r}")
        _require(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest),
                 f"invalid original hash: {name}")
    canonical = json.dumps(originals, sort_keys=True, separators=(",", ":")).encode()
    _require(_sha256(canonical) == INVENTORY_SHA256, "original inventory does not match its pinned digest")
    return originals


def _vendor_files(vendor, originals):
    _require(vendor.is_dir() and not vendor.is_symlink(), "vendor/glib must be a real directory")
    expected_dirs = {str(parent) for name in originals for parent in PurePosixPath(name).parents
                     if str(parent) != "."}
    files = {}
    for path in vendor.rglob("*"):
        relative = path.relative_to(vendor).as_posix()
        mode = path.lstat().st_mode
        _require(not stat.S_ISLNK(mode), f"symlink in vendored source: {relative}")
        if stat.S_ISDIR(mode):
            _require(relative in expected_dirs, f"extra vendor directory: {relative}")
        else:
            _require(stat.S_ISREG(mode), f"non-regular vendor file: {relative}")
            files[relative] = path.read_bytes()
    _require(set(files) == set(originals),
             f"vendor inventory mismatch; missing={sorted(set(originals) - set(files))}, "
             f"extra={sorted(set(files) - set(originals))}")
    patched = files[PATCHED_FILE]
    _require(_sha256(patched) == PATCHED_SHA256, "patched source hash mismatch")
    reversed_source = patched
    for fixed, original in ((PATCHED_DECLARATION, ORIGINAL_DECLARATION),
                            (PATCHED_ARGUMENT, ORIGINAL_ARGUMENT)):
        _require(reversed_source.count(fixed) == 1 and original not in reversed_source,
                 "the two exact impl_get replacements must each occur once")
        reversed_source = reversed_source.replace(fixed, original, 1)
    reverted = dict(files, **{PATCHED_FILE: reversed_source})
    for name, original in reverted.items():
        _require(_sha256(original) == originals[name], f"original source hash mismatch: {name}")
    for name in ("Cargo.toml", "Cargo.toml.orig"):
        package = tomllib.loads(files[name].decode())["package"]
        _require(package.get("name") == "glib" and package.get("version") == VERSION
                 and package.get("license") == "MIT", f"original identity/license changed: {name}")
    return reverted, patched


def _verify_patch(root, original, patched):
    filename = f"glib-{VERSION}/{PATCHED_FILE}"
    expected = "".join(difflib.unified_diff(
        original.decode().splitlines(keepends=True), patched.decode().splitlines(keepends=True),
        fromfile=filename, tofile=filename,
    )).encode()
    actual = (root / f"vendor/glib-{ADVISORY}.patch").read_bytes()
    _require(actual == expected, "patch artifact differs from the exact derived two-line diff")


def _tar_files(archive):
    """Read members, never extract. Strict even before inventory comparison."""
    files = {}
    seen = set()
    prefix = f"glib-{VERSION}/"
    for member in archive:
        _require(_safe_relative(member.name), f"unsafe archive path: {member.name!r}")
        _require(member.name not in seen, f"duplicate archive member: {member.name}")
        seen.add(member.name)
        _require(member.isfile(), f"non-regular archive member: {member.name}")
        _require(member.name.startswith(prefix), f"wrong archive root: {member.name}")
        relative = member.name[len(prefix):]
        _require(_safe_relative(relative), f"unsafe archive relative path: {relative!r}")
        with archive.extractfile(member) as stream:
            files[relative] = stream.read()
    return files


def _verify_archive(path, reverted):
    # Hash and parse the same open file, rather than reopening a replaceable path.
    with Path(path).open("rb") as stream:
        _require(hashlib.file_digest(stream, "sha256").hexdigest() == ARCHIVE_SHA256,
                 "archive SHA-256 does not match the pinned official crate")
        stream.seek(0)
        with tarfile.open(fileobj=stream, mode="r:gz") as archive:
            originals = _tar_files(archive)
    _require(set(originals) == set(reverted), "archive inventory mismatch")
    for name in reverted:
        _require(originals[name] == reverted[name], f"archive bytes differ after reversal: {name}")


def _verify_lock_and_patch(root):
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    patch = manifest.get("patch", {}).get("crates-io", {}).get("glib")
    _require(patch == {"path": "vendor/glib"}, "root GLib patch must be exactly path = vendor/glib")
    raw = (root / "Cargo.lock").read_bytes()
    lock = tomllib.loads(raw.decode())
    glibs = [p for p in lock.get("package", []) if p.get("name") == "glib"]
    _require(len(glibs) == 1 and glibs[0].get("version") == VERSION,
             "Cargo.lock must contain exactly one GLib, version 0.18.5")
    _require("source" not in glibs[0] and "checksum" not in glibs[0],
             "vendored GLib lock entry must have no source or checksum")
    return raw, lock


def _manifest_matches(value, expected):
    if not isinstance(value, str) or not Path(value).is_absolute():
        return False
    return Path(value).resolve(strict=True) == expected.resolve(strict=True)


def _verify_metadata(root, value):
    data = _json(value)
    _require(isinstance(data, dict) and data.get("version") == 1,
             "metadata must be full cargo metadata format-version 1 JSON")
    _require(_manifest_matches(data.get("workspace_root"), root), "metadata workspace root mismatch")
    packages = data.get("packages")
    _require(isinstance(packages, list) and packages, "metadata packages missing")
    _require(all(isinstance(p, dict) and isinstance(p.get("id"), str) for p in packages),
             "metadata packages must have IDs")
    by_id = {p["id"]: p for p in packages}
    _require(len(by_id) == len(packages), "duplicate metadata package ID")
    glibs = [p for p in packages if p.get("name") == "glib"]
    _require(len(glibs) == 1, "metadata must contain exactly one GLib package")
    glib = glibs[0]
    _require(glib.get("version") == VERSION and "source" in glib and glib["source"] is None,
             "metadata GLib must retain version 0.18.5 and use a path source")
    _require(_manifest_matches(glib.get("manifest_path"), root / "vendor/glib/Cargo.toml"),
             "metadata GLib manifest does not resolve to the real vendor manifest")
    app_manifest = root / "src-tauri/Cargo.toml"
    _require(tomllib.loads(app_manifest.read_text(encoding="utf-8"))["package"]["name"] == "agentdeck",
             "src-tauri manifest is not AgentDeck")
    apps = [p for p in packages if p.get("name") == "agentdeck"]
    _require(len(apps) == 1 and "source" in apps[0] and apps[0]["source"] is None
             and _manifest_matches(apps[0].get("manifest_path"), app_manifest),
             "metadata must identify the real src-tauri AgentDeck package")
    app = apps[0]
    _require(app["id"] in data.get("workspace_members", []), "AgentDeck is not a metadata workspace member")
    resolve = data.get("resolve")
    _require(isinstance(resolve, dict) and isinstance(resolve.get("nodes"), list) and resolve["nodes"],
             "metadata resolve graph missing; --no-deps is not accepted")
    nodes = resolve["nodes"]
    _require(all(isinstance(n, dict) and isinstance(n.get("id"), str) for n in nodes),
             "invalid metadata resolve node")
    graph = {n["id"]: n for n in nodes}
    _require(len(graph) == len(nodes) and set(graph) == set(by_id),
             "metadata resolve graph is incomplete or has duplicate nodes")
    for node in nodes:
        deps = node.get("deps")
        dependencies = node.get("dependencies")
        _require(isinstance(deps, list) and isinstance(dependencies, list), "incomplete resolve dependencies")
        _require(all(isinstance(d, dict) and isinstance(d.get("name"), str)
                     and d.get("pkg") in graph for d in deps), "unknown resolve dependency package")
        _require(set(dependencies) == {d["pkg"] for d in deps}, "resolve dependency lists disagree")
    reached = set()
    pending = deque([app["id"]])
    while pending:
        current = pending.popleft()
        if current not in reached:
            reached.add(current)
            pending.extend(d["pkg"] for d in graph[current]["deps"])
    gtk_ids = {p["id"] for p in packages if p.get("name") == "gtk"} & reached
    _require(gtk_ids and glib["id"] in reached, "AgentDeck must reach both GTK and the vendored GLib")
    _require(any(d["name"] == "glib" and d["pkg"] == glib["id"]
                 for gtk in gtk_ids for d in graph[gtk]["deps"]),
             "reachable GTK must directly select the vendored GLib")


def _verify_audit(value):
    data = _json(value)
    _require(isinstance(data, dict), "audit report must be a JSON object")
    vulnerabilities = data.get("vulnerabilities", {})
    warnings = data.get("warnings", {})
    _require(isinstance(vulnerabilities, dict) and isinstance(warnings, dict), "invalid audit finding groups")
    groups = [vulnerabilities.get("list", [])] + list(warnings.values())
    _require(all(isinstance(group, list) for group in groups), "invalid audit finding list")
    for group in groups:
        for finding in group:
            if not isinstance(finding, dict):
                continue
            advisory = finding.get("advisory") or {}
            package = finding.get("package") or {}
            if (isinstance(advisory, dict) and isinstance(package, dict)
                    and advisory.get("id") == ADVISORY and package.get("name") == "glib"
                    and package.get("version") == VERSION):
                return
    raise VerificationError(f"audit report must retain {ADVISORY} for glib {VERSION}; "
                            "an empty report is not evidence because cargo-audit excludes path dependencies")


def _audit_projection(raw, lock):
    headers = list(re.finditer(rb"(?m)^\[\[package\]\][ \t]*\r?\n", raw))
    matches = []
    for index, header in enumerate(headers):
        end = headers[index + 1].start() if index + 1 < len(headers) else len(raw)
        block = raw[header.start():end]
        parsed = tomllib.loads(block.decode())
        if parsed["package"][0].get("name") == "glib":
            matches.append((header.start(), block))
    _require(len(matches) == 1, "cannot locate the unique GLib lock block")
    start, block = matches[0]
    versions = list(re.finditer(rb'(?m)^version[ \t]*=[ \t]*"0\.18\.5"[ \t]*\r?\n', block))
    _require(len(versions) == 1, "cannot locate the GLib version line")
    line = versions[0]
    newline = b"\r\n" if line.group().endswith(b"\r\n") else b"\n"
    inserted = (f'source = "{REGISTRY_SOURCE}"'.encode() + newline
                + f'checksum = "{ARCHIVE_SHA256}"'.encode() + newline)
    offset = start + line.end()
    projection = raw[:offset] + inserted + raw[offset:]
    # Check the semantic projection too; every other byte was retained above.
    projected = tomllib.loads(projection.decode())
    for package in projected["package"]:
        if package.get("name") == "glib":
            _require(package.pop("source") == REGISTRY_SOURCE
                     and package.pop("checksum") == ARCHIVE_SHA256, "invalid audit identity")
    _require(projected == lock, "audit projection changed another lock entry")
    return projection


def _write_audit_lock(path, root, projection):
    destination = Path(path)
    _require(destination.resolve() != (root / "Cargo.lock").resolve(),
             "audit output must never replace Cargo.lock")
    # O_EXCL also rejects an existing symlink, including a dangling symlink.
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(projection)


def verify(root, archive=None, metadata=None, audit_json=None, audit_lock_output=None):
    """Return explicit evidence flags; optional inputs are never assumed verified."""
    root = Path(root).resolve(strict=True)
    originals = _provenance(root)
    reverted, patched = _vendor_files(root / "vendor/glib", originals)
    _verify_patch(root, reverted[PATCHED_FILE], patched)
    raw, lock = _verify_lock_and_patch(root)
    if archive is not None:
        _verify_archive(archive, reverted)
    if metadata is not None:
        _verify_metadata(root, metadata)
    if audit_json is not None:
        _verify_audit(audit_json)
    if audit_lock_output is not None:
        _write_audit_lock(audit_lock_output, root, _audit_projection(raw, lock))
    return {
        "ok": True,
        "name": "glib",
        "version": VERSION,
        "advisory": ADVISORY,
        "inventory_files": FILE_COUNT,
        "source_verified": True,
        "archive_verified": archive is not None,
        "graph_verified": metadata is not None,
        "audit_verified": audit_json is not None,
        "audit_lock_output": str(audit_lock_output) if audit_lock_output is not None else None,
        "runtime_regressions": "not_run_by_this_script",
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--archive", type=Path, help="official glib-0.18.5.crate (required in CI)")
    parser.add_argument("--metadata", type=Path, help="full cargo metadata JSON including resolve graph")
    parser.add_argument("--audit-json", type=Path, help="cargo-audit JSON retaining the GLib advisory")
    parser.add_argument("--audit-lock-output", type=Path, help="NEW private audit-only lockfile; never overwritten")
    args = parser.parse_args(argv)
    if os.environ.get("CI", "").lower() not in ("", "0", "false") and args.archive is None:
        parser.error("--archive is mandatory in CI")
    try:
        result = verify(args.root, args.archive, args.metadata, args.audit_json, args.audit_lock_output)
    except (ValueError, OSError, tarfile.TarError, KeyError, TypeError) as error:
        print(json.dumps({"ok": False, "error": str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
