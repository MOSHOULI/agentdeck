"""Offline negative tests of harness bookkeeping, not real payload acceptance."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch, Mock
import qoder_cross_release_acceptance as h

class Tests(unittest.TestCase):
    def tearDown(self): h.complete.select_release('1.1.85')

    def exercise(self, corrupt=False, releases=('1.1.85', '1.1.86')):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); evidence=root/'evidence'; calls=[]
            def prepare(cli, archive, parent, env):
                cache=parent/'unpacked'; cache.mkdir(mode=0o700)
                for name in ('complete.json','incomplete.json','archive.tar.gz','qodercli'):
                    h.deployment.private_file(cache/name, (h.complete.pinned.VERSION+name).encode())
                return {'unpacked':cache}, {}
            def reject(cli, cache, destination, token, env, trace, **kwargs):
                self.assertTrue(kwargs['reject'])
                self.assertEqual(token,'a'*64)
                calls.append((h.complete.pinned.VERSION,cache.name))
                if corrupt: destination.mkdir(mode=0o700)
            with patch.dict(os.environ,{'RUNNER_TEMP':str(root)}), patch.object(h.deployment,'guard'), patch.object(h.complete,'prepare',side_effect=prepare), patch.object(h.deployment,'run_preview',return_value='a'*64), patch.object(h.deployment,'run_deploy',side_effect=reject), patch.object(h,'reject_fresh_preview') as fresh, patch.object(h,'inspect_state') as inspect:
                result=h.run(Path('/cli'),{v: root/v for v in releases},evidence)
            if result:
                self.assertEqual(fresh.call_count, len(releases)*(len(releases)-1)*4)
                self.assertEqual(inspect.call_count, len(releases)*(len(releases)-1)*5)
                whole = [call.args[-1] for call in inspect.call_args_list if call.args[3].name.startswith('foreign-inspect-')]
                self.assertEqual(whole, [h.FOREIGN_STATES[(r, other)] for r in releases for other in releases if r != other])
                mixed = [call.args[-1] for call in inspect.call_args_list if call.args[3].name.startswith('inspect-')]
                self.assertEqual(mixed, ['conflict'] * (len(releases)*(len(releases)-1)*3))
            return result,json.loads((evidence/'result.json').read_text()),calls

    def test_both_directions_all_three_members_required(self):
        ok,report,calls=self.exercise()
        self.assertTrue(ok)
        self.assertEqual(len(calls),6)
        self.assertEqual(len(report['whole_cache_checks']),2)
        self.assertEqual([(x['selected_release'],x['inspection']) for x in report['whole_cache_checks']], [('1.1.85','unknown_ownership'),('1.1.86','conflict')])
        self.assertEqual(len(report['same_release_checks']),2)
        self.assertEqual({(x['release'],x['member']) for x in report['checks']},
                         {(r,m) for r in ('1.1.85','1.1.86') for m in ('receipt','archive','payload')})
        self.assertTrue(all(x['destination_absent'] and x['source_unchanged'] for x in report['checks']))

    def test_three_releases_cover_every_direction_and_member(self):
        releases = ('1.1.85', '1.1.86', '1.1.87')
        ok, report, calls = self.exercise(releases=releases)
        self.assertTrue(ok)
        self.assertEqual(report['releases'], list(releases))
        self.assertEqual(len(calls), 18)
        self.assertEqual(len(report['whole_cache_checks']), 6)
        self.assertEqual(len(report['same_release_checks']), 6)
        self.assertEqual({(x['release'], x['foreign_release'], x['member']) for x in report['checks']},
                         {(r, o, m) for r in releases for o in releases if r != o for m in ('receipt', 'archive', 'payload')})

    def test_whole_cache_pair_states_follow_exact_size_then_intent_order(self):
        self.assertEqual(h.FOREIGN_STATES, {
            ('1.1.85', '1.1.86'): 'unknown_ownership',
            ('1.1.85', '1.1.87'): 'unknown_ownership',
            ('1.1.86', '1.1.85'): 'conflict',
            ('1.1.86', '1.1.87'): 'conflict',
            ('1.1.87', '1.1.85'): 'conflict',
            ('1.1.87', '1.1.86'): 'unknown_ownership',
        })
        for (selected, foreign), expected in h.FOREIGN_STATES.items():
            own = h.complete.pinned.releases.contract(selected)
            other = h.complete.pinned.releases.contract(foreign)
            self.assertEqual(expected, 'conflict' if other[0] > own[0] or other[2] > own[2] else 'unknown_ownership')

    def test_187_foreign_186_requires_exact_unknown_not_any_rejection_state(self):
        h.complete.select_release('1.1.87')
        valid = {'version': '1.1.87', 'state': 'unknown_ownership', 'installed': False, 'deletion_authorized': False}
        with patch.object(h.complete, 'run_cli', return_value=valid), patch.object(h.deployment, 'check_trace'):
            h.inspect_state(Path('/cli'), Path('/cache'), {}, Path('/trace'), 'unknown_ownership')
        for state in ['conflict', 'incomplete', 'unpacked_payload_verified', 'staged_archive_verified']:
            with patch.object(h.complete, 'run_cli', return_value={**valid, 'state': state}), patch.object(h.deployment, 'check_trace'), self.assertRaises(h.deployment.Failure):
                h.inspect_state(Path('/cli'), Path('/cache'), {}, Path('/trace'), 'unknown_ownership')

    def test_unsupported_or_single_release_fails_before_guard_or_writes(self):
        for releases in [('1.1.85',), ('1.1.85', 'latest')]:
            with patch.object(h.deployment, 'guard') as guard, self.assertRaises(h.deployment.Failure):
                h.run(Path('/cli'), {v: Path('/archive') for v in releases}, Path('/not-created'))
            guard.assert_not_called()

    def test_rejection_that_writes_target_fails_closed(self):
        ok,report,calls=self.exercise(corrupt=True)
        self.assertFalse(ok)
        self.assertEqual(report['failure'],'mixed_release_rejection_changed_state')
        self.assertEqual(len(calls),1)
        self.assertEqual(report['checks'],[])

    def test_fresh_preview_exit_success_is_failure(self):
        process=Mock(); process.wait.return_value=0; process.poll.return_value=0
        with patch.object(h.subprocess,'Popen',return_value=process), self.assertRaises(h.deployment.Failure):
            h.reject_fresh_preview(Path('/cli'),Path('/cache'),Path('/dest'),{},Path('/trace'))

    def test_fresh_preview_requires_trace(self):
        process=Mock(); process.wait.return_value=1; process.poll.return_value=1
        with patch.object(h.subprocess,'Popen',return_value=process), patch.object(h.deployment,'check_trace',side_effect=h.deployment.Failure('trace')), self.assertRaises(h.deployment.Failure):
            h.reject_fresh_preview(Path('/cli'),Path('/cache'),Path('/dest'),{},Path('/trace'))

    def test_conflict_inspection_rejects_verified_or_wrong_version(self):
        valid={'version':'1.1.85','state':'conflict','installed':False,'deletion_authorized':False}
        for invalid in ({**valid,'state':'unpacked_payload_verified'}, {**valid,'version':'1.1.86'}, {**valid,'installed':0}):
            with patch.object(h.complete,'run_cli',return_value=invalid), patch.object(h.deployment,'check_trace'), self.assertRaises(h.deployment.Failure):
                h.inspect_state(Path('/cli'),Path('/cache'),{},Path('/trace'),'conflict')

if __name__ == '__main__': unittest.main()
