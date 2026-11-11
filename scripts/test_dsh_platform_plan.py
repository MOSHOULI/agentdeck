#!/usr/bin/env python3
"""Synthetic offline graph regressions; never npm/package execution."""
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch

from dsh_platform_plan import plan, PlanError


def package(name, **fields):
    return {"name": name, "version": "1.0.0", **fields}


def graph(*packages, **target):
    return {"schemaVersion": 1, "root": "root@1.0.0", "packages": list(packages),
            "target": {"os": "linux", "cpu": "x64", "libc": "glibc", **target}}


def nodes(report):
    return {node["id"]: node for node in report["nodes"]}


def codes(report):
    return {item["code"] for item in report["findings"]}


class PlatformPlanTests(unittest.TestCase):
    def test_targets_are_explicit_and_independent_of_host(self):
        for os_name, cpu, libc in [("linux", "x64", "glibc"),
                                   ("linux", "arm64", "musl"),
                                   ("win32", "x64", None), ("darwin", "arm64", None)]:
            with self.subTest(os=os_name, cpu=cpu):
                document = graph(package("root", os=[os_name], cpu=[cpu]),
                                 os=os_name, cpu=cpu, libc=libc)
                with patch("platform.system", side_effect=AssertionError("host used")), \
                     patch("platform.machine", side_effect=AssertionError("host used")):
                    self.assertNotIn("mandatory_platform_mismatch", codes(plan(document)))
        document["target"].pop("libc")
        with self.assertRaises(PlanError):
            plan(document)

    def test_allow_deny_any_empty_and_string_constraints(self):
        cases = [(["linux"], True), (["win32"], False), (["!win32"], True),
                 (["linux", "!linux"], False), (["any"], True), ([], True),
                 (["any", "win32"], False), ("linux", True)]
        for constraint, match in cases:
            with self.subTest(constraint=constraint):
                result = plan(graph(package("root", os=constraint)))
                self.assertEqual("mandatory_platform_mismatch" not in codes(result), match)

    def test_libc_unknown_never_defaults_to_host(self):
        for constraint in (["glibc"], ["any"], ["!musl"], []):
            with self.subTest(constraint=constraint):
                report = plan(graph(package("root", libc=constraint), libc=None))
                self.assertIn("platform_unknown", codes(report))
        self.assertIn("mandatory_platform_mismatch", codes(
            plan(graph(package("root", libc=["glibc"]), libc="musl"))))
        # An undeclared constraint is not a claim about runtime support.
        report = plan(graph(package("root"), libc=None))
        self.assertEqual(nodes(report)["root@1.0.0"]["platform"]["dimensions"]["libc"], "undeclared")
        self.assertFalse(report["runtime_verified"])

    def test_mandatory_mismatch_is_not_removed(self):
        report = plan(graph(package("root", dependencies={"native": "1.0.0"}),
                            package("native", os=["win32"])))
        self.assertIn("mandatory_platform_mismatch", codes(report))
        self.assertEqual(nodes(report)["native@1.0.0"]["target_path_kinds"], ["mandatory"])
        self.assertEqual(report["excluded_optional_paths"], [])

    def test_optional_subtree_preserved_but_not_target_reachable(self):
        report = plan(graph(package("root", optionalDependencies={"native": "1.0.0"}),
                            package("native", os=["win32"], dependencies={"child": "1.0.0"}),
                            package("child")))
        self.assertEqual(len(report["nodes"]), 3)
        self.assertTrue(nodes(report)["child@1.0.0"]["declared_reachable"])
        self.assertEqual(nodes(report)["child@1.0.0"]["target_path_kinds"], [])
        self.assertEqual(report["unreachable_metadata"], [])
        self.assertEqual(len(report["excluded_optional_paths"]), 1)

    def test_optional_ancestor_keeps_required_descendants_optional(self):
        report = plan(graph(package("root", optionalDependencies={"parent": "1.0.0"}),
                            package("parent", dependencies={"child": "1.0.0"}), package("child")))
        self.assertEqual(nodes(report)["child@1.0.0"]["target_path_kinds"], ["optional"])
        document = graph(package("root", optionalDependencies={"parent": "1.0.0"}),
                         package("parent", dependencies={"child": "1.0.0"}),
                         package("child", os=["win32"]))
        # Required failure inside an optional subtree is reported, not silently
        # reclassified as an independently omittable optional declaration.
        self.assertIn("optional_subtree_platform_mismatch", codes(plan(document)))

    def test_shared_mandatory_path_survives_optional_pruning(self):
        report = plan(graph(package("root", dependencies={"required": "1.0.0"},
                                    optionalDependencies={"optional": "1.0.0"}),
                            package("required", dependencies={"shared": "1.0.0"}),
                            package("optional", dependencies={"shared": "1.0.0"}),
                            package("shared", os=["win32"])))
        self.assertEqual(nodes(report)["shared@1.0.0"]["target_path_kinds"], ["mandatory"])
        self.assertIn("mandatory_platform_mismatch", codes(report))
        self.assertIn("optional_subtree_platform_mismatch", codes(report))

    def test_shared_compatible_node_records_both_modes(self):
        report = plan(graph(package("root", dependencies={"required": "1.0.0"},
                                    optionalDependencies={"shared": "1.0.0"}),
                            package("required", dependencies={"shared": "1.0.0"}), package("shared")))
        self.assertEqual(nodes(report)["shared@1.0.0"]["target_path_kinds"], ["mandatory", "optional"])

    def test_optional_override_keeps_original_declaration_without_following_it(self):
        report = plan(graph(package("root", dependencies={"child": "2.0.0"},
                                    optionalDependencies={"child": "1.0.0"}), package("child")))
        self.assertEqual(len(report["edges"]), 2)
        self.assertTrue(report["edges"][0]["shadowed_by_optional"])
        self.assertEqual(nodes(report)["child@1.0.0"]["target_path_kinds"], ["optional"])
        self.assertNotIn("metadata_missing", codes(report))

    def test_missing_metadata_ranges_aliases_and_workspace_edges_stay_unresolved(self):
        specs = {"missing": "1.0.0", "range": "^1.0.0", "alias": "npm:other@1.0.0",
                 "workspace": "workspace:*", "file": "file:../pkg", "tag": "latest",
                 "url": "https://example.invalid/package.tgz"}
        report = plan(graph(package("root", dependencies=specs)))
        self.assertEqual(codes(report), {"metadata_missing", "spec_unresolved"})
        self.assertEqual(len(report["findings"]), len(specs))
        self.assertFalse(report["dependencies_resolved"])

    def test_peer_contexts_are_not_production_paths_or_assumed_satisfied(self):
        report = plan(graph(package("root", peerDependencies={"peer": "1.0.0"},
                                    peerDependenciesMeta={"peer": {"optional": True}}), package("peer")))
        self.assertEqual(report["unreachable_metadata"], ["peer@1.0.0"])
        self.assertEqual(nodes(report)["peer@1.0.0"]["target_path_kinds"], [])
        self.assertEqual(codes(report), {"peer_context_unresolved"})
        self.assertTrue(report["edges"][0]["optional_peer"])

    def test_unbound_peer_metadata_is_retained_without_inventing_an_edge(self):
        report = plan(graph(package("root", peerDependenciesMeta={"supports-color": {"optional": True}})))
        self.assertEqual(report["edges"], [])
        self.assertEqual(nodes(report)["root@1.0.0"]["unbound_peer_metadata"], ["supports-color"])
        self.assertTrue(nodes(report)["root@1.0.0"]["peer_metadata"]["supports-color"]["optional"])
        self.assertFalse(report["dependencies_resolved"])

    def test_cycles_and_unreachable_nodes_are_bounded_and_retained(self):
        report = plan(graph(package("root", dependencies={"child": "1.0.0"}),
                            package("child", dependencies={"root": "1.0.0"}), package("orphan")))
        self.assertEqual(len(report["nodes"]), 3)
        self.assertEqual(report["unreachable_metadata"], ["orphan@1.0.0"])
        self.assertFalse(report["closure_complete"])

    def test_prerelease_and_build_identity_are_exact_without_semver_guessing(self):
        for version in ("1.2.3-rc.1", "1.2.3+build.1", "1.2.3-alpha.0"):
            with self.subTest(version=version):
                report = plan(graph(package("root", dependencies={"child": version}),
                                    package("child", version=version)))
                self.assertEqual(report["findings"], [])
        for version in ("01.2.3", "1.2.3-01", "v1.2.3", "1.2", "1.2.3-"):
            with self.subTest(version=version), self.assertRaises(PlanError):
                plan(graph(package("root", version=version)))

    def test_invalid_shapes_fail_even_in_excluded_or_unreachable_nodes(self):
        invalid = [{"os": None}, {"os": [1]}, {"cpu": ["!!x64"]}, {"libc": {}},
                   {"dependencies": None}, {"dependencies": {"x": 1}},
                   {"dependencies": {"../x": "1.0.0"}},
                   {"peerDependenciesMeta": {"x": []}},
                   {"peerDependencies": {"x": "1.0.0"},
                    "peerDependenciesMeta": {"x": {"optional": "true"}}}]
        for fields in invalid:
            with self.subTest(fields=fields), self.assertRaises(PlanError):
                plan(graph(package("root"), package("orphan", **fields)))
        with self.assertRaises(PlanError):
            plan(graph(package("root"), package("root")))
        document = graph(package("other"))
        with self.assertRaises(PlanError):
            plan(document)
        for value in (True, "1", 2):
            document = graph(package("root"))
            document["schemaVersion"] = value
            with self.assertRaises(PlanError):
                plan(document)

    def test_input_bounds(self):
        class MustNotTraverse(dict):
            def items(self):
                raise AssertionError("Oversized map was traversed")

        hints = MustNotTraverse({"a": {"optional": True}, "b": {"optional": True}})
        with patch("dsh_platform_plan.MAX_EDGES", 1), self.assertRaises(PlanError):
            plan(graph(package("root", peerDependenciesMeta=hints)))
        oversized = MustNotTraverse({"a": "1.0.0", "b": "1.0.0"})
        with patch("dsh_platform_plan.MAX_EDGES", 1), self.assertRaises(PlanError):
            plan(graph(package("root", dependencies=oversized)))
        with patch("dsh_platform_plan.MAX_PACKAGES", 1), self.assertRaises(PlanError):
            plan(graph(package("root"), package("child")))
        with patch("dsh_platform_plan.MAX_EDGES", 1), self.assertRaises(PlanError):
            plan(graph(package("root", dependencies={"a": "1.0.0", "b": "1.0.0"})))
        with self.assertRaises(PlanError):
            plan(graph(package("root", dependencies={"a": "x" * 513})))

    def test_deterministic_json_no_input_mutation_and_no_readiness_claims(self):
        document = graph(package("root", dependencies={"b": "1.0.0", "a": "1.0.0"}),
                         package("a"), package("b"))
        original = copy.deepcopy(document)
        report = plan(document)
        self.assertEqual(document, original)
        document["packages"].reverse()
        document["packages"][-1]["dependencies"] = {"a": "1.0.0", "b": "1.0.0"}
        document["target"] = dict(reversed(list(document["target"].items())))
        self.assertEqual(json.dumps(report), json.dumps(plan(document)))
        for key in ("input_graph_pruned", "npm_invoked", "lockfile_validated", "closure_complete",
                    "install_ready", "dependencies_resolved", "runtime_verified", "archive_verified", "signature_verified"):
            self.assertIs(report[key], False, key)
        self.assertIsNone(report["npm_version"])
        self.assertEqual(report["peer_resolution"], "unresolved")

    def test_embedded_koffi_metadata_never_becomes_complete_or_runtime_evidence(self):
        # These are existing reviewed metadata bytes, not a refreshed registry or
        # downloaded archive. Missing platform packages must remain visible.
        path = Path(__file__).resolve().parents[1] / "docs/upstream/dsh/native-evidence/koffi/metadata.json"
        metadata = json.loads(path.read_bytes())
        document = graph(metadata)
        document["root"] = metadata["name"] + "@" + metadata["version"]
        report = plan(document)
        self.assertEqual(len(report["edges"]), 15)
        self.assertEqual(codes(report), {"metadata_missing"})
        self.assertFalse(report["closure_complete"])
        self.assertFalse(report["runtime_verified"])


if __name__ == "__main__":
    unittest.main()
