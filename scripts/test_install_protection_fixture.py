import json
import os
from pathlib import Path
import tempfile
import unittest
import uuid
import install_protection_fixture as h

class Tests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.home=Path(self.tmp.name)/'home';self.home.mkdir(mode=0o700)
        self.directory=self.home/'.agentdeck/install-protection/codex'
        for p in (self.home/'.agentdeck',self.directory.parent,self.directory):p.mkdir(mode=0o700)
        self.record={'schema':1,'platform':'windows' if os.name=='nt' else 'linux','phase':'clear','attempt':None,'prefix':None,'prefixIdentity':None}
        self.write('gate.lock',b'');self.state(self.record)
    def write(self,name,data):
        p=self.directory/name;p.write_bytes(data);p.chmod(0o600)
    def state(self,record):self.write('state.json',json.dumps(record).encode())
    def failed(self):
        self.record.update(phase='failed_returned',attempt=str(uuid.uuid4()),prefix=str(self.home/'prefix'),prefixIdentity='authored-identity');self.state(self.record)
    def test_clear_and_failed_exact_records(self):
        self.assertEqual(h.validate_install_protection(self.home,'clear',True),self.record)
        self.failed();self.assertEqual(h.validate_install_protection(self.home,'failed_returned',True),self.record)
    def test_extra_files_bad_schema_and_wrong_phase_rejected(self):
        self.write('.leftover.tmp',b'');
        with self.assertRaises(h.Failure):h.validate_install_protection(self.home)
        (self.directory/'.leftover.tmp').unlink()
        for patch in ({'schema':True},{'phase':'ignored'},{'attempt':'extra'},{'extra':1}):
            self.state({**self.record,**patch})
            with self.assertRaises(h.Failure):h.validate_install_protection(self.home)
        self.write('state.json',b'{"schema":1,"schema":1}')
        with self.assertRaises(h.Failure):h.validate_install_protection(self.home)
    def test_lock_content_and_hardlinks_rejected(self):
        self.write('gate.lock',b'x')
        with self.assertRaises(h.Failure):h.validate_install_protection(self.home)
        self.write('gate.lock',b'');os.link(self.directory/'state.json',self.home/'linked')
        with self.assertRaises(h.Failure):h.validate_install_protection(self.home)
    def test_synthetic_states_are_separate_unchanged_source_and_not_recovery(self):
        self.failed();before=h.protection_snapshot(self.home)
        for case in ('armed','corrupt'):
            target=Path(self.tmp.name)/case;target.mkdir(mode=0o700)
            snap=h.prepare_synthetic_protection(self.home,target,case)
            self.assertEqual(snap,h.protection_snapshot(target))
            if case=='armed':self.assertEqual(h.validate_install_protection(target)['phase'],'armed')
            else:
                with self.assertRaises(h.Failure):h.validate_install_protection(target)
            self.assertEqual(h.protection_snapshot(self.home),before)
            with self.assertRaises(h.Failure):h.prepare_synthetic_protection(self.home,target,case)
    @unittest.skipIf(os.name=='nt','Unix ancestor link regression')
    def test_linked_product_ancestor_is_rejected(self):
        product=self.home/'.agentdeck';saved=self.home/'saved-product';product.rename(saved);product.symlink_to(saved,target_is_directory=True)
        with self.assertRaises(h.Failure):h.validate_install_protection(self.home)

    @unittest.skipIf(os.name=='nt','Unix permissions regression')
    def test_private_permissions_required(self):
        (self.directory/'state.json').chmod(0o644)
        with self.assertRaises(h.Failure):h.validate_install_protection(self.home)

if __name__=='__main__':unittest.main()
