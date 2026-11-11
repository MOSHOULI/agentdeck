#!/usr/bin/env python3
"""Bounded offline platform planning. No file/network/process access or resolver.

Call plan() with loaded exact-version metadata. Caller data is never a lock.
"""
from collections import deque
import re

MAX_PACKAGES = 2048
MAX_EDGES = 16384
MAX_STRING = 512
NAME = re.compile(r"(?:@[a-z0-9][a-z0-9._-]*/)?[a-z0-9][a-z0-9._-]*\Z")
VERSION = re.compile(
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?\Z"
)
TOKEN = re.compile(r"[a-z0-9][a-z0-9._-]*\Z")
EDGE_FIELDS = ("dependencies", "optionalDependencies", "peerDependencies")


class PlanError(ValueError):
    """Malformed or unsupported graph shape; no partial plan is returned."""


def _string(value):
    if not isinstance(value, str) or not 0 < len(value) <= MAX_STRING:
        raise PlanError("Expected a bounded nonempty string")
    return value


def _name(value):
    if not NAME.fullmatch(_string(value)):
        raise PlanError("Unsupported package name")
    return value


def _version(value):
    if not VERSION.fullmatch(_string(value)):
        raise PlanError("Package identity needs an exact semver version")
    return value


def _constraints(value):
    if isinstance(value, str):
        value = [value]
    if not isinstance(value, list) or len(value) > 128:
        raise PlanError("Malformed platform declaration")
    for item in value:
        _string(item)
        token = item[1:] if item.startswith("!") else item
        if not TOKEN.fullmatch(token):
            raise PlanError("Malformed platform token")
    return list(value)


def _matches(value, choices):
    # npm-install-checks allow/deny semantics, deliberately without host fallback.
    if value is None:
        return "unknown"
    if choices == ["any"]:
        return "match"
    if "!" + value in choices:
        return "mismatch"
    positives = [item for item in choices if not item.startswith("!")]
    return "match" if not positives or value in positives else "mismatch"


def _platform(manifest, target):
    dimensions = {
        key: _matches(target[key], _constraints(manifest[key]))
        if key in manifest else "undeclared"
        for key in ("os", "cpu", "libc")
    }
    if "mismatch" in dimensions.values():
        result = "mismatch"
    elif "unknown" in dimensions.values():
        result = "unknown"
    else:
        result = "declared_constraints_match"
    return {"state": result, "dimensions": dimensions,
            "declarations": {key: _constraints(manifest[key])
                             for key in dimensions if key in manifest}}


def plan(document):
    """Plan input v1: schemaVersion, root ('name@version'), target, packages.

    Target requires os/cpu/libc; libc=None means unknown, never host detection.
    Only exact semver edges identify metadata nodes. Other specs and all peer
    contexts stay unresolved. This is not npm tree placement or semver solving.
    """
    if not isinstance(document, dict) or set(document) != {
        "schemaVersion", "root", "target", "packages"
    } or type(document["schemaVersion"]) is not int or document["schemaVersion"] != 1:
        raise PlanError("Unsupported input schema")
    target = document["target"]
    if not isinstance(target, dict) or set(target) != {"os", "cpu", "libc"}:
        raise PlanError("Target os, cpu and explicit libc are required")
    for key, value in target.items():
        if key == "libc" and value is None:
            continue
        if not TOKEN.fullmatch(_string(value)) or value == "any":
            raise PlanError("Target must use concrete platform tokens")
    packages = document["packages"]
    if not isinstance(packages, list) or not 0 < len(packages) <= MAX_PACKAGES:
        raise PlanError("Package count out of bounds")
    nodes = {}
    edge_count = 0
    for package in packages:
        if not isinstance(package, dict):
            raise PlanError("Expected package metadata object")
        identity = _name(package.get("name")) + "@" + _version(package.get("version"))
        if identity in nodes:
            raise PlanError("Duplicate package identity")
        # Validate even unreachable and platform-excluded nodes.
        platform = _platform(package, target)
        declarations = {}
        for field in EDGE_FIELDS:
            entries = package.get(field, {})
            if not isinstance(entries, dict):
                raise PlanError("Malformed dependency declarations")
            if edge_count + len(entries) > MAX_EDGES:
                raise PlanError("Edge count out of bounds")
            edge_count += len(entries)
            declarations[field] = {}
            for child, spec in entries.items():
                declarations[field][_name(child)] = _string(spec)
            declarations[field] = dict(sorted(declarations[field].items()))
        peer_meta = package.get("peerDependenciesMeta", {})
        if not isinstance(peer_meta, dict):
            raise PlanError("Malformed peer metadata")
        for peer, info in peer_meta.items():
            if peer not in declarations["peerDependencies"] or not isinstance(info, dict):
                raise PlanError("Peer metadata needs a declared peer")
            if "optional" in info and type(info["optional"]) is not bool:
                raise PlanError("Peer optional flag must be boolean")
        nodes[identity] = {"platform": platform, "declarations": declarations,
                           "peer_meta": peer_meta}
    root = _string(document["root"])
    if root not in nodes:
        raise PlanError("Root metadata is missing")
    edges = []
    outgoing = {identity: [] for identity in nodes}
    for identity, node in sorted(nodes.items()):
        declarations = node["declarations"]
        for field in EDGE_FIELDS:
            for name, spec in sorted(declarations[field].items()):
                peer = field == "peerDependencies"
                shadowed = field == "dependencies" and name in declarations["optionalDependencies"]
                exact = bool(VERSION.fullmatch(spec))
                child = name + "@" + spec if exact else None
                edge = {
                    "from": identity, "name": name, "spec": spec, "kind": field,
                    "to": child, "shadowed_by_optional": shadowed,
                    "optional_peer": peer and node["peer_meta"].get(name, {}).get("optional", False),
                    "state": "peer_context_unresolved" if peer else (
                        "spec_unresolved" if not exact else (
                            "exact_metadata_present" if child in nodes else "metadata_missing")),
                }
                edges.append(edge)
                if not peer and not shadowed:
                    outgoing[identity].append(edge)
    # Preserve unfiltered declaration reachability separately from target paths.
    observed = {root}
    queue = deque([root])
    while queue:
        for edge in outgoing[queue.popleft()]:
            child = edge["to"]
            if child in nodes and child not in observed:
                observed.add(child)
                queue.append(child)
    # Both path kinds can reach a shared node. Mandatory wins for blockers.
    # An optional ancestor keeps all its mandatory descendants optional.
    paths = {identity: set() for identity in nodes}
    paths[root].add("mandatory")
    queue = deque([(root, "mandatory")])
    excluded = set()
    optional_failures = set()
    while queue:
        identity, mode = queue.popleft()
        for edge in outgoing[identity]:
            child = edge["to"]
            if child not in nodes:
                continue
            child_mode = "optional" if mode == "optional" or edge["kind"] == "optionalDependencies" else "mandatory"
            if child_mode == "optional" and nodes[child]["platform"]["state"] == "mismatch":
                excluded.add((identity, edge["kind"], edge["name"], child))
                if edge["kind"] == "dependencies":
                    optional_failures.add((identity, child))
                continue
            if child_mode not in paths[child]:
                paths[child].add(child_mode)
                queue.append((child, child_mode))
    findings = [{"code": "optional_subtree_platform_mismatch", "from": source,
                 "node": child} for source, child in sorted(optional_failures)]
    for edge in edges:
        if not paths[edge["from"]] or edge["shadowed_by_optional"]:
            continue
        if edge["state"] != "exact_metadata_present":
            findings.append({"code": edge["state"], "from": edge["from"],
                             "name": edge["name"], "kind": edge["kind"]})
    result_nodes = []
    for identity, node in sorted(nodes.items()):
        modes = sorted(paths[identity])
        state = node["platform"]["state"]
        if "mandatory" in modes and state == "mismatch":
            findings.append({"code": "mandatory_platform_mismatch", "node": identity})
        if modes and state == "unknown":
            findings.append({"code": "platform_unknown", "node": identity})
        result_nodes.append({"id": identity, "declared_reachable": identity in observed,
                             "target_path_kinds": modes, "platform": node["platform"],
                             "declarations": node["declarations"]})
    return {
        "schema_version": 1, "mode": "offline_exact_metadata_platform_plan",
        "target": {key: target[key] for key in ("os", "cpu", "libc")}, "root": root, "nodes": result_nodes, "edges": edges,
        "excluded_optional_paths": [
            {"from": source, "kind": kind, "name": name, "to": child}
            for source, kind, name, child in sorted(excluded)],
        "findings": findings, "unreachable_metadata": sorted(set(nodes) - observed),
        "input_graph_pruned": False, "npm_invoked": False, "npm_version": None,
        "lockfile_validated": False, "closure_complete": False, "install_ready": False,
        "dependencies_resolved": False, "runtime_verified": False,
        "archive_verified": False, "signature_verified": False, "peer_resolution": "unresolved",
    }
