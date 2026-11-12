import { describe, it, expect } from 'vitest';
import { parseCodexInstallRecovery, recoveryRevision } from './codexInstallRecoveryContract';
export const eligibleRecovery = {
  schemaVersion: 1, agentId:'codex', state:'eligible', prefix:'/synthetic/prefix',
  slots:[{slot:'package_directory',state:'missing'},{slot:'posix_launcher',state:'missing'}],
  revision:'a'.repeat(64), runtimeNotRun:true, cleanupSupported:false, limitations:'Synthetic metadata only.',
};
describe('Codex install recovery projection', () => {
  it('accepts only complete fixed passive evidence', () => {
    expect(parseCodexInstallRecovery(eligibleRecovery)).toEqual(eligibleRecovery);
    expect(parseCodexInstallRecovery({...eligibleRecovery,state:'blocked',revision:null})).toMatchObject({state:'blocked'});
    expect(parseCodexInstallRecovery({...eligibleRecovery,state:'clear',revision:null,prefix:null,slots:[]})).toMatchObject({state:'clear'});
  });
  it.each([
    null, [], {}, {...eligibleRecovery,schemaVersion:2}, {...eligibleRecovery,agentId:'other'},
    {...eligibleRecovery,state:'ready'}, {...eligibleRecovery,runtimeNotRun:false}, {...eligibleRecovery,cleanupSupported:true},
    {...eligibleRecovery,revision:null}, {...eligibleRecovery,revision:'A'.repeat(64)},
    {...eligibleRecovery,state:'blocked'}, {...eligibleRecovery,state:'clear'},
    {...eligibleRecovery,prefix:'relative'}, {...eligibleRecovery,prefix:'/bad\npath'},
    {...eligibleRecovery,slots:[]}, {...eligibleRecovery,slots:[eligibleRecovery.slots[0],eligibleRecovery.slots[0]]},
    {...eligibleRecovery,slots:[{slot:'package_directory',state:'present'},eligibleRecovery.slots[1]]},
    {...eligibleRecovery,slots:[{slot:'unknown',state:'missing'},eligibleRecovery.slots[1]]},
    {...eligibleRecovery,limitations:'x'.repeat(8193)},
  ])('rejects incomplete or authority-expanding response %j', value => {
    expect(() => parseCodexInstallRecovery(value)).toThrow();
  });
  it('rejects malformed confirm tokens', () => {
    for(const v of ['',null,'a'.repeat(63),'g'.repeat(64),'a'.repeat(64)+'\n']) expect(recoveryRevision(v)).toBe(false);
  });
});
