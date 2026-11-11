"""Offline preparation tests, never network or package execution."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import fetch_dsh_native_ui_archives as subject


class Tests(unittest.TestCase):
    def test_fixed_metadata_and_archive_validation_before_write(self):
        items = subject.pinned.load_review()
        with tempfile.TemporaryDirectory() as temp, patch.object(subject.pinned, 'fetch', return_value=b'fixture') as fetch, patch.object(subject.pinned, 'validate_metadata') as metadata, patch.object(subject.pinned, 'validate_archive') as archive:
            root = Path(temp) / 'new'
            result = subject.prepare(root)
            self.assertEqual(result['packages'], list(subject.pinned.IDS))
            self.assertEqual({p.name for p in root.iterdir()}, {a['archive_name'] for a in items})
            self.assertEqual(fetch.call_args_list, [call for a in items for call in [unittest.mock.call(a['metadata_url'], 262144), unittest.mock.call(a['archive_url'], a['archive_bytes'])]])
            self.assertEqual(metadata.call_count, 4); self.assertEqual(archive.call_count, 4)
            with self.assertRaises(FileExistsError): subject.prepare(root)
    def test_bad_metadata_or_archive_never_written(self):
        for validator in ('validate_metadata', 'validate_archive'):
            with self.subTest(validator=validator), tempfile.TemporaryDirectory() as temp, patch.object(subject.pinned, 'fetch', return_value=b'fixture'), patch.object(subject.pinned, 'validate_metadata'), patch.object(subject.pinned, 'validate_archive'):
                with patch.object(subject.pinned, validator, side_effect=subject.pinned.Failure('invalid')):
                    root = Path(temp) / 'new'
                    with self.assertRaises(subject.pinned.Failure): subject.prepare(root)
                    self.assertEqual(list(root.iterdir()), [])
    def test_unhosted_guard_fails(self):
        with tempfile.TemporaryDirectory() as temp, patch.dict('os.environ', {}, clear=True):
            with self.assertRaises(subject.pinned.Failure): subject.root_download.guard(Path(temp) / 'new')


if __name__ == '__main__': unittest.main()
