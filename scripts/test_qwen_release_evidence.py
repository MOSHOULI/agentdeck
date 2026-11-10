#!/usr/bin/env python3
"""Offline verifier regressions, not evidence of an official package runtime."""
import hashlib
import importlib.util
import pathlib
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('evidence', pathlib.Path(__file__).with_name('verify_qwen_release_evidence.py'))
evidence = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evidence)

class EvidenceTests(unittest.TestCase):
    def test_both_explicit_versions_share_the_exact_reviewed_reader(self):
        releases = evidence.pinned_releases()
        self.assertEqual(set(releases), {'0.27.0', '0.27.2'})
        for release in releases.values():
            self.assertEqual(hashlib.sha256(evidence.READER.read_bytes()).hexdigest(), release['readerSha256'])
            self.assertEqual(set(release['members']), {'manifest', 'loader', 'schema'})

    def test_bad_archive_rejected_before_opening_tar(self):
        for release in evidence.pinned_releases().values():
            with patch.object(evidence.tarfile, 'open') as opened:
                with self.assertRaisesRegex(ValueError, 'integrity'):
                    evidence.verify_archive(b'not a package', '0.27.2', release)
                opened.assert_not_called()

    def test_wrong_full_hash_rejected_before_members(self):
        release = dict(evidence.pinned_releases()['0.27.2'], archiveBytes=3)
        with patch.object(evidence.tarfile, 'open') as opened:
            with self.assertRaisesRegex(ValueError, 'integrity'):
                evidence.verify_archive(b'bad', '0.27.2', release)
            opened.assert_not_called()

    def test_download_redirect_is_rejected(self):
        with patch.object(evidence.urllib.request, 'urlopen') as opened:
            opened.return_value.__enter__.return_value.geturl.return_value = 'https://unrelated.invalid/'
            with self.assertRaisesRegex(ValueError, 'redirect'):
                evidence.download('https://registry.npmjs.org/fixed', 12)

    def test_download_size_is_bounded(self):
        with patch.object(evidence.urllib.request, 'urlopen') as opened:
            response = opened.return_value.__enter__.return_value
            response.geturl.return_value = 'https://registry.npmjs.org/fixed'
            response.read.return_value = b'1234'
            with self.assertRaisesRegex(ValueError, 'bound'):
                evidence.download('https://registry.npmjs.org/fixed', 3)
            response.read.assert_called_once_with(4)

if __name__ == '__main__':
    unittest.main()
