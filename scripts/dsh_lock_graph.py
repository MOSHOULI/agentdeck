#!/usr/bin/env python3
"""Independent, bounded npm v3 lock-location audit; no IO or npm execution.

This is a partial evidence gate, not a resolver, install plan or lock approval.
"""
from collections import deque

from dsh_platform_plan import (
    EDGE_FIELDS, MAX_EDGES, MAX_PACKAGES, PlanError, VERSION,
    _name, _string, _version, plan,
)

LOCK_FLAGS = ('dev', 'optional', 'devOptional', 'peer', 'inBundle')


def _location(path):
    """Accept only canonical installed-package locations, never filesystem paths."""
    if path == '':
        return ()
    _string(path)
    parts = path.split('/')
    names = []
    index = 0
    while index < len(parts):
        if parts[index] != 'node_modules' or index + 1 == len(parts):
            raise PlanError('Unsupported lock location; workspace/link paths are not accepted')
        index += 1
        name = parts[index]
        index += 1
        # Consume a scoped name atomically: its package component can itself be
        # "node_modules", so splitting on that directory name is ambiguous.
        if name.startswith('@'):
            if index == len(parts):
                raise PlanError('Incomplete scoped lock location')
            name += '/' + parts[index]
            index += 1
        _name(name)
        names.append(name)
    return tuple(names)


def _path(names):
    return 'node_modules/' + '/node_modules/'.join(names) if names else ''


def _declarations(package, remaining):
    result = {}
    count = 0
    for field in EDGE_FIELDS:
        entries = package.get(field, {})
        if not isinstance(entries, dict):
            raise PlanError('Malformed lock dependency declarations')
        if len(entries) > remaining - count:
            raise PlanError('Lock edge count out of bounds')
        count += len(entries)
        result[field] = dict(sorted((_name(name), _string(spec)) for name, spec in entries.items()))
    return result, count


def _resolve(nodes, location, name, peer):
    # Dependencies search within the declaring package, then ancestor packages.
    # Peers start beside the declaring package, not inside its node_modules.
    prefix = location[:-1] if peer and location else location
    while True:
        candidate = _path((*prefix, name))
        if candidate in nodes:
            return candidate
        if not prefix:
            return None
        prefix = prefix[:-1]


def audit(lock, metadata_graph, *, npm_version=None):
    """Audit already parsed lock v3 against a #76 metadata graph.

    Source root metadata is required. Missing dependency metadata stays visible.
    Exact literal versions are compared; semver ranges and peer contexts are not
    certified. npm_version is a caller record, never inferred from this host.
    """
    source_report = plan(metadata_graph)
    sources = {node['id']: node for node in source_report['nodes']}
    target = source_report['target']
    source_peers = {(edge['from'], edge['name']): edge['optional_peer']
                    for edge in source_report['edges'] if edge['kind'] == 'peerDependencies'}
    if npm_version is not None:
        _version(npm_version)
    if not isinstance(lock, dict) or type(lock.get('lockfileVersion')) is not int or lock['lockfileVersion'] != 3:
        raise PlanError('Only npm lockfileVersion 3 is supported')
    packages = lock.get('packages')
    if not isinstance(packages, dict) or '' not in packages or not 0 < len(packages) <= MAX_PACKAGES:
        raise PlanError('Lock package count or root is invalid')
    nodes = {}
    findings = []
    if npm_version is None:
        findings.append({'code': 'producer_npm_version_missing'})
    edge_count = 0
    peer_hint_count = 0
    # Validate before sorting keys so mixed-type keys fail through PlanError.
    for path, package in packages.items():
        location = _location(path)
        if not isinstance(package, dict):
            raise PlanError('Malformed lock package')
        if 'link' in package and (type(package['link']) is not bool or package['link']):
            raise PlanError('Linked lock nodes are unsupported')
        name = _name(package.get('name')) if not location else location[-1]
        if 'name' in package and package['name'] != name:
            raise PlanError('Alias lock nodes require a separate review')
        version = _version(package.get('version'))
        identity = name + '@' + version
        if not location and identity != source_report['root']:
            raise PlanError('Lock and source root identities disagree')
        flags = {}
        for flag in LOCK_FLAGS:
            if flag in package:
                if type(package[flag]) is not bool:
                    raise PlanError('Malformed lock classification flag')
                flags[flag] = package[flag]
        declarations, count = _declarations(package, MAX_EDGES - edge_count)
        edge_count += count
        peer_meta = package.get('peerDependenciesMeta', {})
        if not isinstance(peer_meta, dict):
            raise PlanError('Malformed lock peer metadata')
        if peer_hint_count + len(peer_meta) > MAX_EDGES:
            raise PlanError('Lock peer metadata count out of bounds')
        peer_hint_count += len(peer_meta)
        for peer, info in peer_meta.items():
            _name(peer)
            if not isinstance(info, dict):
                raise PlanError('Malformed lock peer metadata entry')
            if 'optional' in info and type(info['optional']) is not bool:
                raise PlanError('Lock peer optional flag must be boolean')
        if not location:
            if lock.get('name', name) != name or lock.get('version', version) != version:
                raise PlanError('Conflicting lock root identity fields')
        source = sources.get(identity)
        nodes[path] = {'location': location, 'identity': identity, 'version': version,
                       'lock_declarations': declarations, 'source': source, 'flags': flags,
                       'peer_meta': {peer: {'optional': info.get('optional', False)}
                                     for peer, info in sorted(peer_meta.items())}}
    # Every nested installation must have its package ancestors in the lock.
    for path, node in sorted(nodes.items()):
        location = node['location']
        for depth in range(1, len(location)):
            if _path(location[:depth]) not in nodes:
                raise PlanError('Nested lock location has a missing ancestor')
        source = node['source']
        if source is None:
            findings.append({'code': 'source_metadata_missing', 'location': path, 'identity': node['identity']})
        else:
            for field in EDGE_FIELDS:
                if node['lock_declarations'][field] != source['declarations'][field]:
                    findings.append({'code': 'lock_declarations_disagree', 'location': path, 'field': field})
            if node['peer_meta'] != source['peer_metadata']:
                findings.append({'code': 'lock_peer_metadata_disagree', 'location': path})
        for peer in sorted(set(node['peer_meta']) - set(node['lock_declarations']['peerDependencies'])):
            findings.append({'code': 'peer_hint_without_declaration', 'location': path, 'name': peer})
        if node['flags'].get('inBundle'):
            findings.append({'code': 'bundled_content_unverified', 'location': path})

    edges = []
    outgoing = {path: [] for path in nodes}
    # Source declarations win when present. Deleting a lock edge cannot hide it.
    # The total authoritative output is separately bounded, including repeated
    # copies of the same package identity at different installed locations.
    authoritative_count = 0
    for path, node in sorted(nodes.items()):
        source = node['source']
        declarations = source['declarations'] if source else node['lock_declarations']
        for field in EDGE_FIELDS:
            if authoritative_count + len(declarations[field]) > MAX_EDGES:
                raise PlanError('Audited edge count out of bounds')
            authoritative_count += len(declarations[field])
            for name, spec in declarations[field].items():
                peer = field == 'peerDependencies'
                shadowed = field == 'dependencies' and name in declarations['optionalDependencies']
                destination = _resolve(nodes, node['location'], name, peer)
                if destination is None:
                    state = 'target_missing'
                elif not VERSION.fullmatch(spec):
                    state = 'spec_unverified'
                elif nodes[destination]['version'] != spec:
                    state = 'exact_version_mismatch'
                else:
                    state = 'exact_version_match'
                edge = {'from': path, 'name': name, 'spec': spec, 'kind': field,
                        'to': destination, 'state': state, 'shadowed_by_optional': shadowed,
                        'declaration_basis': 'source_metadata' if source else 'lock_only',
                        'peer_context': 'unverified' if peer else 'not_applicable',
                        'optional_peer': peer and (source_peers.get((node['identity'], name), False)
                                                  if source else node['peer_meta'].get(name, {}).get('optional', False))}
                edges.append(edge)
                if not shadowed:
                    if state != 'exact_version_match':
                        findings.append({'code': state, 'from': path, 'name': name, 'kind': field})
                    if peer:
                        findings.append({'code': 'peer_context_unverified', 'from': path, 'name': name})
                    else:
                        outgoing[path].append(edge)

    # Candidate reachability is not proof that its ranges or metadata are valid.
    observed = {''}
    queue = deque([''])
    while queue:
        for edge in outgoing[queue.popleft()]:
            child = edge['to']
            if child is not None and child not in observed:
                observed.add(child)
                queue.append(child)
    paths = {path: set() for path in nodes}
    paths[''].add('mandatory')
    queue = deque([('', 'mandatory')])
    excluded = set()
    optional_failures = set()
    while queue:
        path, mode = queue.popleft()
        for edge in outgoing[path]:
            child = edge['to']
            if child is None:
                continue
            child_mode = 'optional' if mode == 'optional' or edge['kind'] == 'optionalDependencies' else 'mandatory'
            source = nodes[child]['source']
            if child_mode == 'optional' and source and source['platform']['state'] == 'mismatch':
                excluded.add((path, edge['kind'], edge['name'], child))
                if edge['kind'] == 'dependencies':
                    optional_failures.add((path, child))
                continue
            if child_mode not in paths[child]:
                paths[child].add(child_mode)
                queue.append((child, child_mode))
    for source, child in sorted(optional_failures):
        findings.append({'code': 'optional_subtree_platform_mismatch', 'from': source, 'location': child})
    result_nodes = []
    for path, node in sorted(nodes.items()):
        source = node['source']
        platform = source['platform'] if source else {'state': 'source_metadata_missing'}
        modes = sorted(paths[path])
        if 'mandatory' in modes and platform['state'] == 'mismatch':
            findings.append({'code': 'mandatory_platform_mismatch', 'location': path})
        if modes and platform['state'] in ('unknown', 'source_metadata_missing'):
            findings.append({'code': 'platform_unverified', 'location': path})
        result_nodes.append({
            'location': path, 'identity': node['identity'], 'platform': platform,
            'source_metadata_available': source is not None,
            'observed_lock_flags': node['flags'],
            'candidate_reachable': path in observed, 'candidate_path_kinds': modes,
            'lock_declarations': node['lock_declarations'],
            'lock_peer_metadata': node['peer_meta'],
        })
    return {
        'schema_version': 1, 'mode': 'offline_npm_v3_location_audit',
        'root': source_report['root'], 'target': target,
        'producer_npm_version': npm_version, 'producer_version_verified': False,
        'nodes': result_nodes, 'edges': edges, 'findings': findings,
        'unreachable_locations': sorted(set(nodes) - observed),
        'excluded_optional_paths': [
            {'from': parent, 'kind': kind, 'name': name, 'to': child}
            for parent, kind, name, child in sorted(excluded)],
        'input_graph_pruned': False, 'lock_flags_used_for_reachability': False,
        'lock_platform_constraints_used': False, 'npm_invoked': False,
        'lockfile_validated': False, 'closure_complete': False,
        'dependencies_resolved': False, 'install_ready': False,
        'runtime_verified': False, 'archive_verified': False,
        'signature_verified': False, 'source_provenance_verified': False,
        'peer_resolution': 'unresolved',
    }
