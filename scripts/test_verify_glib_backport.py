#!/usr/bin/env python3
"""Isolated verifier tests, not Rust regressions or evidence of a real Cargo graph.

Run: python -B -m unittest discover -s scripts -p 'test_verify_glib_backport.py' -v
Optional: set GLIB_BACKPORT_ARCHIVE to the official downloaded crate to also test
its actual bytes. Graph and audit JSON below are deliberately synthetic fixtures.
No cargo, network, system installs, application home, or external mutation.
"""

import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("verify_glib_backport", ROOT / "scripts/verify-glib-backport.py")
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)


class BackportVerifierTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="agentdeck-glib-verifier-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "repo"
        self.root.mkdir()
        shutil.copytree(ROOT / "vendor", self.root / "vendor", symlinks=True)
        for name in ("Cargo.toml", "Cargo.lock", "src-tauri/Cargo.toml"):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)

    def reject(self, **kwargs):
        with self.assertRaises((VERIFIER.VerificationError, OSError, ValueError)):
            VERIFIER.verify(self.root, **kwargs)

    def rewrite(self, relative, old, new):
        path = self.root / relative
        original = path.read_bytes()
        self.assertIn(old, original)
        path.write_bytes(original.replace(old, new, 1))

    def synthetic_metadata(self):
        app = "path+file:///synthetic#agentdeck@1.1.0"
        tauri = "registry+https://github.com/rust-lang/crates.io-index#tauri@2.10.0"
        gtk = "registry+https://github.com/rust-lang/crates.io-index#gtk@0.18.2"
        glib = "path+file:///synthetic/vendor/glib#glib@0.18.5"
        packages = [
            {"id": app, "name": "agentdeck", "version": "1.1.0", "source": None,
             "manifest_path": str(self.root / "src-tauri/Cargo.toml")},
            {"id": tauri, "name": "tauri", "version": "2.10.0", "source": VERIFIER.REGISTRY_SOURCE},
            {"id": gtk, "name": "gtk", "version": "0.18.2", "source": VERIFIER.REGISTRY_SOURCE},
            {"id": glib, "name": "glib", "version": "0.18.5", "source": None,
             "manifest_path": str(self.root / "vendor/glib/Cargo.toml")},
        ]
        nodes = []
        for package, dependencies in ((app, [("tauri", tauri)]), (tauri, [("gtk", gtk)]),
                                      (gtk, [("glib", glib)]), (glib, [])):
            nodes.append({"id": package, "dependencies": [p for _, p in dependencies],
                          "deps": [{"name": name, "pkg": p, "dep_kinds": [{"kind": None, "target": None}]}
                                   for name, p in dependencies], "features": []})
        return {"version": 1, "workspace_root": str(self.root), "workspace_members": [app],
                "packages": packages, "resolve": {"root": None, "nodes": nodes}}

    def synthetic_finding(self):
        return {"advisory": {"id": VERIFIER.ADVISORY},
                "package": {"name": "glib", "version": "0.18.5", "source": VERIFIER.REGISTRY_SOURCE}}

    def test_static_success_does_not_claim_optional_or_runtime_checks(self):
        result = VERIFIER.verify(self.root)
        self.assertTrue(result["source_verified"])
        self.assertEqual(result["inventory_files"], 121)
        for key in ("archive_verified", "graph_verified", "audit_verified"):
            self.assertIs(result[key], False)
        self.assertEqual(result["runtime_regressions"], "not_run_by_this_script")

    @unittest.skipUnless(os.environ.get("GLIB_BACKPORT_ARCHIVE"), "official archive not supplied")
    def test_official_archive_when_explicitly_supplied(self):
        result = VERIFIER.verify(self.root, archive=os.environ["GLIB_BACKPORT_ARCHIVE"])
        self.assertTrue(result["archive_verified"])
        self.assertFalse(result["graph_verified"])
        self.assertFalse(result["audit_verified"])

    def test_tampered_unpatched_source(self):
        with (self.root / "vendor/glib/src/lib.rs").open("ab") as stream:
            stream.write(b"\n// unexpected edit\n")
        self.reject()

    def test_either_missing_fix_is_rejected(self):
        path = self.root / "vendor/glib/src/variant_iter.rs"
        pristine = path.read_bytes()
        for fixed, original in ((VERIFIER.PATCHED_DECLARATION, VERIFIER.ORIGINAL_DECLARATION),
                                (VERIFIER.PATCHED_ARGUMENT, VERIFIER.ORIGINAL_ARGUMENT)):
            with self.subTest(fixed=fixed):
                path.write_bytes(pristine.replace(fixed, original, 1))
                self.reject()

    def test_extra_change_in_fixed_source(self):
        self.rewrite("vendor/glib/src/variant_iter.rs", b"p.to_str().unwrap()", b"p.to_str().expect(\"UTF-8\")")
        self.reject()

    def test_license_and_copyright_are_unchanged(self):
        for name in ("LICENSE", "COPYRIGHT"):
            with self.subTest(file=name):
                path = self.root / "vendor/glib" / name
                original = path.read_bytes()
                path.write_bytes(original + b"\nmodified\n")
                self.reject()
                path.write_bytes(original)

    def test_normalized_manifest_version_is_not_relabeled(self):
        self.rewrite("vendor/glib/Cargo.toml", b'version = "0.18.5"', b'version = "0.18.6"')
        self.reject()

    def test_original_manifest_license_is_not_changed(self):
        self.rewrite("vendor/glib/Cargo.toml.orig", b'license = "MIT"', b'license = "Apache-2.0"')
        self.reject()

    def test_extra_vendor_file(self):
        (self.root / "vendor/glib/extra.rs").write_text("unexpected")
        self.reject()

    def test_extra_empty_vendor_directory(self):
        (self.root / "vendor/glib/extra").mkdir()
        self.reject()

    def test_missing_vendor_file(self):
        (self.root / "vendor/glib/README.md").unlink()
        self.reject()

    def test_vendor_file_symlink(self):
        path = self.root / "vendor/glib/README.md"
        path.unlink()
        path.symlink_to("LICENSE")
        self.reject()

    def test_vendor_directory_symlink(self):
        path = self.root / "vendor/glib"
        path.rename(self.root / "vendor/elsewhere")
        path.symlink_to("elsewhere", target_is_directory=True)
        self.reject()

    def test_patch_artifact_must_be_exact(self):
        self.rewrite("vendor/glib-RUSTSEC-2024-0429.patch", b"@@ -117,13 +117,13 @@", b"@@ -118,13 +118,13 @@")
        self.reject()

    def test_patch_artifact_cannot_target_another_file(self):
        self.rewrite("vendor/glib-RUSTSEC-2024-0429.patch", b"+++ glib-0.18.5/src/variant_iter.rs",
                     b"+++ glib-0.18.5/src/variant.rs")
        self.reject()

    def test_pinned_provenance_fields(self):
        path = self.root / "vendor/glib-backport.json"
        original = json.loads(path.read_text())
        for key in ("schema_version", "archive_url", "archive_sha256", "upstream_fix", "advisory",
                    "version", "patched_file", "patched_sha256"):
            with self.subTest(field=key):
                changed = dict(original, **{key: "wrong"})
                path.write_text(json.dumps(changed))
                self.reject()

    def test_inventory_cannot_be_rewritten_to_accept_tampering(self):
        path = self.root / "vendor/glib/LICENSE"
        path.write_bytes(b"different license")
        manifest = self.root / "vendor/glib-backport.json"
        provenance = json.loads(manifest.read_text())
        provenance["original_files"]["LICENSE"] = VERIFIER._sha256(path.read_bytes())
        manifest.write_text(json.dumps(provenance))
        self.reject()

    def test_missing_archive(self):
        self.reject(archive=self.root / "missing.crate")

    def test_wrong_archive(self):
        path = self.root / "wrong.crate"
        path.write_bytes(b"not the official crate")
        self.reject(archive=path)

    def test_archive_member_security_without_extraction(self):
        # Test member validation directly: the public entry point rejects any
        # synthetic archive at its pinned hash check before member inspection.
        for names, member_type in (
            (["/glib-0.18.5/file"], tarfile.REGTYPE),
            (["glib-0.18.5/../outside"], tarfile.REGTYPE),
            (["glib-0.18.5/./file"], tarfile.REGTYPE),
            (["glib-0.18.5//file"], tarfile.REGTYPE),
            (["glib-0.18.5\\file"], tarfile.REGTYPE),
            (["other/file"], tarfile.REGTYPE),
            (["glib-0.18.5/file", "glib-0.18.5/file"], tarfile.REGTYPE),
            (["glib-0.18.5/file"], tarfile.SYMTYPE),
            (["glib-0.18.5/file"], tarfile.LNKTYPE),
            (["glib-0.18.5/file"], tarfile.FIFOTYPE),
        ):
            with self.subTest(names=names, member_type=member_type):
                buffer = io.BytesIO()
                with tarfile.open(fileobj=buffer, mode="w") as archive:
                    for name in names:
                        member = tarfile.TarInfo(name)
                        member.type = member_type
                        member.linkname = "../../outside" if member_type in (tarfile.SYMTYPE, tarfile.LNKTYPE) else ""
                        archive.addfile(member)
                buffer.seek(0)
                with tarfile.open(fileobj=buffer, mode="r:") as archive:
                    with self.assertRaises(VERIFIER.VerificationError):
                        VERIFIER._tar_files(archive)
                self.assertFalse((self.root / "outside").exists())

    def test_duplicate_glib_in_lock(self):
        with (self.root / "Cargo.lock").open("ab") as stream:
            stream.write(b'\n[[package]]\nname = "glib"\nversion = "0.18.4"\n')
        self.reject()

    def test_registry_source_or_checksum_in_real_lock_rejected(self):
        path = self.root / "Cargo.lock"
        original = path.read_bytes()
        for line in (f'source = "{VERIFIER.REGISTRY_SOURCE}"\n', f'checksum = "{VERIFIER.ARCHIVE_SHA256}"\n'):
            with self.subTest(line=line):
                path.write_bytes(original.replace(b'name = "glib"\nversion = "0.18.5"\n',
                                 b'name = "glib"\nversion = "0.18.5"\n' + line.encode(), 1))
                self.reject()

    def test_wrong_root_patch_target_or_version_override(self):
        path = self.root / "Cargo.toml"
        original = path.read_bytes()
        for replacement in (b'glib = { path = "vendor/other" }',
                            b'glib = { path = "vendor/glib", version = "0.18.6" }'):
            with self.subTest(patch=replacement):
                path.write_bytes(original.replace(b'glib = { path = "vendor/glib" }', replacement))
                self.reject()

    def test_synthetic_complete_graph_accepted(self):
        result = VERIFIER.verify(self.root, metadata=self.synthetic_metadata())
        self.assertTrue(result["graph_verified"])
        self.assertFalse(result["audit_verified"])
        self.assertEqual(result["runtime_regressions"], "not_run_by_this_script")

    def test_synthetic_metadata_json_file(self):
        path = self.root / "metadata.json"
        path.write_text(json.dumps(self.synthetic_metadata()))
        self.assertTrue(VERIFIER.verify(self.root, metadata=path)["graph_verified"])

    def test_synthetic_disconnected_app_not_just_gtk_edge(self):
        metadata = self.synthetic_metadata()
        metadata["resolve"]["nodes"][0].update(deps=[], dependencies=[])
        self.reject(metadata=metadata)

    def test_no_deps_metadata(self):
        metadata = self.synthetic_metadata()
        metadata["resolve"] = None
        self.reject(metadata=metadata)

    def test_wrong_or_duplicate_glib_package(self):
        for field, value in (("version", "0.18.6"), ("source", VERIFIER.REGISTRY_SOURCE),
                             ("manifest_path", str(self.root / "src-tauri/Cargo.toml"))):
            with self.subTest(field=field):
                metadata = self.synthetic_metadata()
                metadata["packages"][-1][field] = value
                self.reject(metadata=metadata)
        metadata = self.synthetic_metadata()
        duplicate = copy.deepcopy(metadata["packages"][-1])
        duplicate.update(id="second-glib", version="0.18.4")
        metadata["packages"].append(duplicate)
        self.reject(metadata=metadata)

    def test_fake_app_or_wrong_workspace(self):
        metadata = self.synthetic_metadata()
        metadata["packages"][0]["manifest_path"] = str(self.root / "vendor/glib/Cargo.toml")
        self.reject(metadata=metadata)
        metadata = self.synthetic_metadata()
        metadata["workspace_root"] = str(self.root.parent)
        self.reject(metadata=metadata)

    def test_gtk_must_select_glib_directly(self):
        metadata = self.synthetic_metadata()
        glib_id = metadata["packages"][-1]["id"]
        metadata["resolve"]["nodes"][0]["deps"].append({"name": "glib", "pkg": glib_id})
        metadata["resolve"]["nodes"][0]["dependencies"].append(glib_id)
        metadata["resolve"]["nodes"][2].update(deps=[], dependencies=[])
        self.reject(metadata=metadata)

    def test_partial_or_inconsistent_graph(self):
        metadata = self.synthetic_metadata()
        metadata["resolve"]["nodes"].pop()
        self.reject(metadata=metadata)
        metadata = self.synthetic_metadata()
        metadata["resolve"]["nodes"][0]["dependencies"] = []
        self.reject(metadata=metadata)

    def test_synthetic_audit_finding_in_vulnerabilities_or_warning_group(self):
        for report in ({"vulnerabilities": {"list": [self.synthetic_finding()]}, "warnings": {}},
                       {"vulnerabilities": {"list": []}, "warnings": {"unsound": [self.synthetic_finding()]}}):
            with self.subTest(report=report):
                result = VERIFIER.verify(self.root, audit_json=report)
                self.assertTrue(result["audit_verified"])
                self.assertFalse(result["graph_verified"])

    def test_empty_audit_report_is_not_accepted(self):
        self.reject(audit_json={"vulnerabilities": {"list": []}, "warnings": {}})
        self.reject(audit_json={})

    def test_wrong_audit_advisory_package_or_version(self):
        for section, field, value in (("advisory", "id", "RUSTSEC-0000-0000"),
                                      ("package", "name", "other"), ("package", "version", "0.18.6")):
            with self.subTest(field=field):
                finding = self.synthetic_finding()
                finding[section][field] = value
                self.reject(audit_json={"warnings": {"unsound": [finding]}})

    def test_audit_projection_only_inserts_original_registry_identity(self):
        before = (self.root / "Cargo.lock").read_bytes()
        output = self.root / "audit-only.lock"
        result = VERIFIER.verify(self.root, audit_lock_output=output)
        inserted = (f'source = "{VERIFIER.REGISTRY_SOURCE}"\n'
                    f'checksum = "{VERIFIER.ARCHIVE_SHA256}"\n').encode()
        after = output.read_bytes()
        marker = b'name = "glib"\nversion = "0.18.5"\n'
        self.assertEqual(after, before.replace(marker, marker + inserted, 1))
        self.assertEqual((self.root / "Cargo.lock").read_bytes(), before)
        self.assertFalse(result["audit_verified"])
        parsed = tomllib.loads(after.decode())
        glib = [p for p in parsed["package"] if p["name"] == "glib"][0]
        self.assertEqual(glib["source"], VERIFIER.REGISTRY_SOURCE)
        self.assertEqual(glib["checksum"], VERIFIER.ARCHIVE_SHA256)
        if os.name != "nt":
            self.assertEqual(output.stat().st_mode & 0o777, 0o600)

    def test_audit_projection_preserves_crlf_and_comments(self):
        path = self.root / "Cargo.lock"
        raw = b"# keep this comment\n" + path.read_bytes()
        raw = raw.replace(b"\n", b"\r\n")
        path.write_bytes(raw)
        output = self.root / "crlf.lock"
        VERIFIER.verify(self.root, audit_lock_output=output)
        inserted = (f'source = "{VERIFIER.REGISTRY_SOURCE}"\r\n'
                    f'checksum = "{VERIFIER.ARCHIVE_SHA256}"\r\n').encode()
        marker = b'name = "glib"\r\nversion = "0.18.5"\r\n'
        self.assertEqual(output.read_bytes(), raw.replace(marker, marker + inserted, 1))

    def test_audit_projection_never_overwrites_existing_file_or_cargo_lock(self):
        path = self.root / "existing.lock"
        path.write_bytes(b"do not overwrite")
        self.reject(audit_lock_output=path)
        self.assertEqual(path.read_bytes(), b"do not overwrite")
        lock = self.root / "Cargo.lock"
        before = lock.read_bytes()
        self.reject(audit_lock_output=lock)
        self.assertEqual(lock.read_bytes(), before)

    def test_audit_projection_rejects_existing_symlink(self):
        output = self.root / "symlink.lock"
        target = self.root / "must-not-be-created.lock"
        output.symlink_to(target)
        self.reject(audit_lock_output=output)
        self.assertFalse(target.exists())

    def test_failed_validation_does_not_create_audit_output(self):
        output = self.root / "audit-only.lock"
        self.reject(audit_json={"warnings": {}}, audit_lock_output=output)
        self.assertFalse(output.exists())

    def test_cli_requires_archive_in_ci(self):
        result = subprocess.run([sys.executable, "-B", str(ROOT / "scripts/verify-glib-backport.py"),
                                 "--root", str(self.root)], env=dict(os.environ, CI="true"),
                                capture_output=True, text=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("--archive is mandatory in CI", result.stderr)

    def test_cli_missing_optional_evidence_is_explicit(self):
        result = subprocess.run([sys.executable, "-B", str(ROOT / "scripts/verify-glib-backport.py"),
                                 "--root", str(self.root)], env=dict(os.environ, CI="false"),
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertFalse(report["archive_verified"])
        self.assertFalse(report["graph_verified"])
        self.assertFalse(report["audit_verified"])


if __name__ == "__main__":
    unittest.main()
