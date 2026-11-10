import { expect, it } from 'vitest';
import { validateAnimationsPreview as preview, validateAnimationsRecovery as recovery, validateAnimationsMutation as mutation } from './codexAnimationsContract';
const token = 'a'.repeat(64);
it('projects fixed data and drops unexpected sensitive fields', () => {
  expect(preview({ field:'tui.animations', before:true, after:false, changes:true, token, secret:'secret' }, false)).not.toHaveProperty('secret');
  expect(recovery({ state:'restore_original', token, secret:'secret' })).toEqual({ state:'restore_original', token });
});
it('rejects mismatched field, bool, target, changes and invalid token', () => {
  const good={ field:'tui.animations',before:true,after:false,changes:true,token };
  for (const v of [null, [], {...good,field:'model'}, {...good,before:'true'}, {...good,after:true}, {...good,changes:false}, {...good,token:'bad'}]) expect(() => preview(v,false)).toThrow();
});
it('recovery requires independently valid token except no_receipt', () => {
  expect(recovery({ state:'no_receipt',token:null })).toEqual({state:'no_receipt',token:null});
  for (const v of [null, {state:'new'}, {state:'restore_original',token:null}, {state:'no_receipt',token}, {state:'cleanup_receipt',token:'bad'}]) expect(() => recovery(v)).toThrow();
  expect(() => mutation({success:true})).toThrow(); expect(mutation(false)).toBe(false);
});
import { validateAnimationsCapability as capability, validateAnimationsObservation as observation } from './codexAnimationsContract';
it('accepts only fixed platform capability values',()=>{ for(const value of ['linux_managed','windows_read_only','unsupported']) expect(capability(value)).toBe(value); for(const value of [null,{},'Windows 11','linux',true]) expect(()=>capability(value)).toThrow(); });
it('read-only observation projects only fixed bool/state and rejects write-capable data',()=>{
  const good={schemaVersion:1,readOnly:true,state:'ready',current:false,applySupported:false,restoreSupported:false};
  expect(observation({...good,token:'secret',secret:'secret'})).toEqual(good);
  for(const value of [null,{}, {...good,schemaVersion:2},{...good,readOnly:false},{...good,current:'false'},{...good,state:'missing'},{...good,applySupported:true},{...good,restoreSupported:true},{...good,state:'secret'}]) expect(()=>observation(value)).toThrow();
});
