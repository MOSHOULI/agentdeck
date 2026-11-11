#!/usr/bin/env python3
"""Offline lock-location regressions; authored graphs and checked-in UI lock."""
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch

from dsh_lock_graph import audit
from dsh_platform_plan import PlanError


def manifest(name, version='1.0.0', **fields):
    return {'name': name, 'version': version, **fields}


def graph(*packages, **target):
    return {'schemaVersion': 1, 'root': 'root@1.0.0', 'packages': list(packages),
            'target': {'os': 'linux', 'cpu': 'x64', 'libc': 'glibc', **target}}


def lock(root, **nodes):
    return {'name': root['name'], 'version': root['version'], 'lockfileVersion': 3,
            'packages': {'': root, **nodes}}


def locations(report):
    return {node['location']: node for node in report['nodes']}


def codes(report):
    return {finding['code'] for finding in report['findings']}


class LockGraphTests(unittest.TestCase):
    def test_nested_overrides_hoisted_node_and_keeps_distinct_locations(self):
        root = manifest('root', dependencies={'a': '1.0.0', 'b': '1.0.0'})
        a = manifest('a', dependencies={'shared': '1.0.0'})
        b = manifest('b', dependencies={'shared': '2.0.0'})
        one, two = manifest('shared'), manifest('shared', '2.0.0')
        candidate = lock(root, **{'node_modules/a': a, 'node_modules/b': b,
                                 'node_modules/shared': one, 'node_modules/b/node_modules/shared': two})
        report = audit(candidate, graph(root, a, b, one, two), npm_version='11.9.0')
        self.assertEqual(report['findings'], [])
        edges = {edge['from']: edge for edge in report['edges'] if edge['name'] == 'shared'}
        self.assertEqual(edges['node_modules/a']['to'], 'node_modules/shared')
        self.assertEqual(edges['node_modules/b']['to'], 'node_modules/b/node_modules/shared')
        self.assertEqual(len(report['nodes']), 5)

    def test_scoped_paths_and_ancestor_search(self):
        root = manifest('root', dependencies={'@scope/a': '1.0.0', '@other/dep': '1.0.0'})
        a = manifest('@scope/a', dependencies={'b': '1.0.0'})
        b = manifest('b', dependencies={'@other/dep': '1.0.0'})
        dep = manifest('@other/dep')
        candidate = lock(root, **{'node_modules/@scope/a': a, 'node_modules/@scope/a/node_modules/b': b,
                                 'node_modules/@other/dep': dep})
        report = audit(candidate, graph(root, a, b, dep), npm_version='11.9.0')
        self.assertEqual(report['findings'], [])
        self.assertEqual(report['edges'][-1]['to'], 'node_modules/@other/dep')

    def test_missing_nearest_incompatible_and_unverified_range_edges(self):
        root = manifest('root', dependencies={'missing': '1.0.0', 'wrong': '1.0.0', 'range': '^1.0.0'})
        wrong, ranged = manifest('wrong', '2.0.0'), manifest('range')
        candidate = lock(root, **{'node_modules/wrong': wrong, 'node_modules/range': ranged})
        report = audit(candidate, graph(root, wrong, ranged))
        self.assertEqual(codes(report), {'producer_npm_version_missing', 'target_missing',
                                        'exact_version_mismatch', 'spec_unverified'})
        self.assertFalse(report['dependencies_resolved'])

    def test_nearest_wrong_version_does_not_fall_back_to_compatible_ancestor(self):
        root = manifest('root', dependencies={'a': '1.0.0', 'dep': '1.0.0'})
        a = manifest('a', dependencies={'dep': '1.0.0'})
        one, two = manifest('dep'), manifest('dep', '2.0.0')
        candidate = lock(root, **{'node_modules/a': a, 'node_modules/dep': one,
                                 'node_modules/a/node_modules/dep': two})
        report = audit(candidate, graph(root, a, one, two))
        edge = next(edge for edge in report['edges'] if edge['from'] == 'node_modules/a')
        self.assertEqual((edge['to'], edge['state']), ('node_modules/a/node_modules/dep', 'exact_version_mismatch'))

    def test_removed_lock_declaration_cannot_hide_source_edge(self):
        root = manifest('root', dependencies={'child': '1.0.0'})
        report = audit(lock(manifest('root')), graph(root))
        self.assertIn('lock_declarations_disagree', codes(report))
        self.assertIn('target_missing', codes(report))
        self.assertEqual(report['edges'][0]['declaration_basis'], 'source_metadata')

    def test_extra_lock_declaration_is_reported_and_retained(self):
        source = manifest('root')
        recorded = manifest('root', dependencies={'extra': '1.0.0'})
        report = audit(lock(recorded), graph(source))
        self.assertIn('lock_declarations_disagree', codes(report))
        self.assertEqual(locations(report)['']['lock_declarations']['dependencies'], {'extra': '1.0.0'})
        self.assertEqual(report['edges'], [])

    def test_missing_metadata_uses_labelled_lock_only_edges_and_never_lock_libc(self):
        root = manifest('root', optionalDependencies={'a': '1.0.0'})
        a = manifest('a', dependencies={'missing': '1.0.0'}, os=['win32'], libc=['musl'])
        report = audit(lock(root, **{'node_modules/a': a}), graph(root))
        self.assertIn('source_metadata_missing', codes(report))
        self.assertEqual(locations(report)['node_modules/a']['candidate_path_kinds'], ['optional'])
        self.assertEqual(report['excluded_optional_paths'], [])
        self.assertEqual(report['edges'][-1]['declaration_basis'], 'lock_only')
        self.assertFalse(report['lock_platform_constraints_used'])

    def test_lock_optional_flags_cannot_hide_mandatory_platform_mismatch(self):
        root = manifest('root', dependencies={'native': '1.0.0'})
        native = manifest('native', os=['win32'])
        recorded = {**native, 'optional': True, 'dev': True, 'peer': True}
        report = audit(lock(root, **{'node_modules/native': recorded}), graph(root, native))
        self.assertIn('mandatory_platform_mismatch', codes(report))
        node = locations(report)['node_modules/native']
        self.assertEqual(node['candidate_path_kinds'], ['mandatory'])
        self.assertTrue(node['observed_lock_flags']['optional'])

    def test_source_libc_missing_from_v3_lock_still_governs_target(self):
        root = manifest('root', optionalDependencies={'native': '1.0.0'})
        native = manifest('native', libc=['musl'])
        recorded = manifest('native')
        report = audit(lock(root, **{'node_modules/native': recorded}), graph(root, native))
        self.assertEqual(len(report['excluded_optional_paths']), 1)
        unknown = audit(lock(root, **{'node_modules/native': recorded}), graph(root, native, libc=None))
        self.assertEqual(unknown['excluded_optional_paths'], [])
        self.assertIn('platform_unverified', codes(unknown))

    def test_shared_identity_at_two_locations_has_independent_path_modes(self):
        root = manifest('root', dependencies={'mandatory': '1.0.0'}, optionalDependencies={'opt': '1.0.0'})
        mandatory = manifest('mandatory', dependencies={'shared': '1.0.0'})
        opt = manifest('opt', dependencies={'shared': '1.0.0'})
        shared = manifest('shared', os=['win32'])
        candidate = lock(root, **{'node_modules/mandatory': mandatory, 'node_modules/opt': opt,
                                 'node_modules/mandatory/node_modules/shared': shared,
                                 'node_modules/opt/node_modules/shared': shared})
        report = audit(candidate, graph(root, mandatory, opt, shared))
        self.assertEqual(locations(report)['node_modules/mandatory/node_modules/shared']['candidate_path_kinds'], ['mandatory'])
        self.assertEqual(locations(report)['node_modules/opt/node_modules/shared']['candidate_path_kinds'], [])
        self.assertIn('mandatory_platform_mismatch', codes(report))
        self.assertIn('optional_subtree_platform_mismatch', codes(report))

    def test_shared_hoisted_mandatory_wins_and_optional_override_is_preserved(self):
        root = manifest('root', dependencies={'parent': '1.0.0', 'shared': '2.0.0'},
                        optionalDependencies={'shared': '1.0.0'})
        parent = manifest('parent', dependencies={'shared': '1.0.0'})
        shared = manifest('shared', os=['win32'])
        report = audit(lock(root, **{'node_modules/parent': parent, 'node_modules/shared': shared}),
                       graph(root, parent, shared))
        self.assertEqual(locations(report)['node_modules/shared']['candidate_path_kinds'], ['mandatory'])
        self.assertIn('mandatory_platform_mismatch', codes(report))
        self.assertNotIn('exact_version_mismatch', codes(report))
        self.assertTrue(any(edge['shadowed_by_optional'] for edge in report['edges']))

    def test_peer_must_not_resolve_inside_dependent_and_context_stays_unverified(self):
        root = manifest('root', dependencies={'a': '1.0.0'})
        a = manifest('a', peerDependencies={'peer': '1.0.0'}, peerDependenciesMeta={'peer': {'optional': True}})
        peer = manifest('peer')
        candidate = lock(root, **{'node_modules/a': a, 'node_modules/a/node_modules/peer': peer})
        report = audit(candidate, graph(root, a, peer))
        edge = next(edge for edge in report['edges'] if edge['kind'] == 'peerDependencies')
        self.assertIsNone(edge['to']); self.assertTrue(edge['optional_peer'])
        self.assertIn('peer_context_unverified', codes(report))
        candidate['packages']['node_modules/peer'] = peer
        report = audit(candidate, graph(root, a, peer))
        edge = next(edge for edge in report['edges'] if edge['kind'] == 'peerDependencies')
        self.assertEqual(edge['to'], 'node_modules/peer')
        self.assertEqual(edge['state'], 'exact_version_match')
        self.assertEqual(locations(report)['node_modules/peer']['candidate_path_kinds'], [])
        self.assertEqual(edge['peer_context'], 'unverified')

    def test_unbound_optional_peer_hint_never_creates_a_dependency(self):
        root = manifest('root', peerDependenciesMeta={'supports-color': {'optional': True}})
        report = audit(lock(root), graph(root), npm_version='11.9.0')
        self.assertEqual(report['edges'], [])
        self.assertEqual(codes(report), {'peer_hint_without_declaration'})
        self.assertTrue(locations(report)['']['lock_peer_metadata']['supports-color']['optional'])
        tampered = lock(manifest('root'))
        self.assertIn('lock_peer_metadata_disagree', codes(audit(tampered, graph(root))))

    def test_cycles_and_unreachable_locations_are_not_dropped(self):
        root = manifest('root', dependencies={'a': '1.0.0'})
        a = manifest('a', dependencies={'a': '1.0.0'})
        orphan = manifest('orphan')
        report = audit(lock(root, **{'node_modules/a': a, 'node_modules/orphan': orphan}), graph(root, a, orphan))
        self.assertEqual(len(report['nodes']), 3)
        self.assertEqual(report['unreachable_locations'], ['node_modules/orphan'])

    def test_invalid_paths_links_aliases_and_missing_ancestors_fail(self):
        root = manifest('root')
        for path in ('/node_modules/a', 'node_modules/../a', 'node_modules/a/',
                     'node_modules/a\\b', 'packages/a', 'node_modules/@scope',
                     'node_modules/a/node_modules/b'):
            with self.subTest(path=path), self.assertRaises(PlanError):
                audit(lock(root, **{path: manifest('b' if path.endswith('/b') else 'a')}), graph(root))
        for fields in ({'link': True}, {'link': 'false'}, {'optional': 'true'}, {'name': 'alias'},
                       {'version': '^1.0.0'}, {'dependencies': None},
                       {'peerDependenciesMeta': {'peer': []}}):
            with self.subTest(fields=fields), self.assertRaises(PlanError):
                audit(lock(root, **{'node_modules/a': {**manifest('a'), **fields}}), graph(root))

    def test_root_and_version_shapes_fail_closed(self):
        root = manifest('root')
        for change in ({'lockfileVersion': True}, {'lockfileVersion': 2}, {'packages': {}},
                       {'name': 'different'}, {'version': '2.0.0'}):
            with self.subTest(change=change), self.assertRaises(PlanError):
                audit({**lock(root), **change}, graph(root))
        with self.assertRaises(PlanError):
            audit(lock(manifest('other')), graph(root))
        with self.assertRaises(PlanError):
            audit(lock(root), graph(root), npm_version='latest')

    def test_edge_bounds_checked_before_traversal_and_across_repeated_source_nodes(self):
        class MustNotTraverse(dict):
            def items(self): raise AssertionError('Oversized map was traversed')
        root = manifest('root')
        hints = MustNotTraverse({'a': {'optional': True}, 'b': {'optional': True}})
        with patch('dsh_lock_graph.MAX_EDGES', 1), self.assertRaises(PlanError):
            audit(lock(manifest('root', peerDependenciesMeta=hints)), graph(root))
        candidate = lock(root, **{'node_modules/a': manifest('a', dependencies=MustNotTraverse({'a': '1.0.0', 'b': '1.0.0'}))})
        with patch('dsh_lock_graph.MAX_EDGES', 1), self.assertRaises(PlanError):
            audit(candidate, graph(root))
        a = manifest('a', dependencies={'b': '1.0.0'})
        candidate = lock(root, **{'node_modules/a': manifest('a'), 'node_modules/a/node_modules/a': manifest('a')})
        with patch('dsh_lock_graph.MAX_EDGES', 1), self.assertRaises(PlanError):
            audit(candidate, graph(root, a))
        with patch('dsh_lock_graph.MAX_PACKAGES', 1), self.assertRaises(PlanError):
            audit(candidate, graph(root, a))

    def test_determinism_input_immutability_and_permanent_readiness_boundaries(self):
        root = manifest('root', dependencies={'b': '1.0.0', 'a': '1.0.0'})
        a, b = manifest('a'), manifest('b')
        candidate = lock(root, **{'node_modules/a': a, 'node_modules/b': b})
        source = graph(root, a, b); originals = copy.deepcopy((candidate, source))
        report = audit(candidate, source)
        self.assertEqual((candidate, source), originals)
        candidate['packages'] = dict(reversed(list(candidate['packages'].items())))
        source['packages'].reverse()
        self.assertEqual(json.dumps(report), json.dumps(audit(candidate, source)))
        for key in ('input_graph_pruned', 'lock_flags_used_for_reachability', 'lock_platform_constraints_used',
                    'npm_invoked', 'lockfile_validated', 'closure_complete', 'dependencies_resolved',
                    'install_ready', 'runtime_verified', 'archive_verified', 'signature_verified',
                    'source_provenance_verified', 'producer_version_verified'):
            self.assertIs(report[key], False, key)

    def test_real_checked_in_ui_lock_is_structure_only_without_dependency_metadata(self):
        root = Path(__file__).resolve().parents[1] / 'web/ui'
        candidate = json.loads((root / 'package-lock.json').read_bytes())
        manifest_root = json.loads((root / 'package.json').read_bytes())
        source = graph(manifest_root)
        source['root'] = manifest_root['name'] + '@' + manifest_root['version']
        report = audit(candidate, source)
        self.assertEqual(len(report['nodes']), len(candidate['packages']))
        self.assertGreater(len(report['nodes']), 100)
        self.assertIn('source_metadata_missing', codes(report))
        self.assertIn('producer_npm_version_missing', codes(report))
        self.assertIn('spec_unverified', codes(report))
        self.assertFalse(report['lockfile_validated'])


if __name__ == '__main__': unittest.main()
