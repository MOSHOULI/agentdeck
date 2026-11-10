// Synthetic compatibility check using Qwen's pinned, unmodified readConfigFile.
// No CLI wrapper, environment loading, account, hooks, MCP or network is invoked.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { readConfigFile } from '../tests/fixtures/qwen-0.27.0/read-config-file.ts';
const source = readFileSync(new URL('../tests/fixtures/qwen-0.27.0/read-config-file.ts', import.meta.url));
assert.equal(createHash('sha256').update(source).digest('hex'), '64c62594e00b4f5c5acfddfdf8c92ff9f6e057c32c36f3e3a8f30742fbbb279c');
const binary = resolve(process.argv[2] ?? `target/debug/aisync${process.platform === 'win32' ? '.exe' : ''}`);
const directory = mkdtempSync(join(tmpdir(), 'qwen-owned-contract-'));
const settings = join(directory, 'settings.json');
const manifest = join(directory, 'package.json');
const args = ['--settings', settings, '--runtime-manifest', manifest];
const run = (action, extra = [], success = true) => {
  const result = spawnSync(binary, ['qwen', action, ...args, ...extra], {encoding:'utf8', timeout:10000});
  assert.equal(result.error, undefined);
  if (success) {assert.equal(result.status, 0, result.stderr);return JSON.parse(result.stdout);}
  assert.notEqual(result.status, 0);return result;
};
try {
 for (const version of ['0.27.0', '0.27.2']) {
  writeFileSync(manifest, JSON.stringify({name:'@qwen-code/qwen-code',version}));
  const original = {$version:4,general:{language:'en'},security:{example:'SYNTHETIC-ONLY'}};
  // Preserve JavaScript numeric semantics across the entire rewritten document.
  // Default serde_json float parsing (without float_roundtrip) changed these.
  const numbers = ['1000000000000000.123456789e-0', '-0', '-0.0', '5e-324',
    '1.7976931348623157e308', '9007199254740993', '18446744073709551616',
    ...Array.from({length:1000}, (_,i)=>`${1000000000000000+i}.123456789e-0`)];
  const raw = JSON.stringify(original).slice(0,-1)+',"numeric":['+numbers.join(',')+']}';
  const expectedNumbers = JSON.parse(raw).numeric;
  const unchangedNumbers = value => {
    assert.equal(value.numeric.length, expectedNumbers.length);
    value.numeric.forEach((n,i)=>assert.ok(Object.is(n,expectedNumbers[i]), `numeric sample ${i} changed`));
  };
  writeFileSync(settings, raw);
  const plan = run('vim-preview',['--enabled','true']);
  assert.equal(existsSync(join(directory,'.agentdeck-qwen-vim.json')), false);
  run('vim-apply',['--enabled','true','--preview-token',plan.token],false);
  run('vim-apply',['--enabled','true','--preview-token',plan.token,'--yes']);
  // Qwen settings.ts uses JSON.parse on the read string after JSONC/BOM handling.
  // Our output is strict JSON without BOM, so those normalizations are identity.
  const applied = JSON.parse(readConfigFile(settings));
  assert.equal(applied.general.vimMode,true);
  unchangedNumbers(applied);
  assert.equal(applied.general.language,'en');
  assert.equal(applied.security.example,'SYNTHETIC-ONLY');
  // A new process reads the same persisted value. This is not a running Qwen UI test.
  assert.equal(run('vim-preview',['--enabled','true']).changes,false);
  // Updating ownership keeps the first backup, including original absence.
  for (const enabled of [false, true, false]) {
    const update = run('vim-update-preview', ['--enabled', String(enabled)]);
    const beforeUpdate = readFileSync(settings);
    run('vim-update', ['--enabled', String(enabled), '--preview-token', update.token], false);
    assert.deepEqual(readFileSync(settings), beforeUpdate);
    run('vim-update', ['--enabled', String(enabled), '--preview-token', update.token, '--yes']);
    const updated = JSON.parse(readConfigFile(settings));
    assert.equal(updated.general.vimMode, enabled);
    unchangedNumbers(updated);
    assert.equal(updated.general.language, 'en');
  }
  // Keep negative zero intact when simulating an unrelated external edit.
  writeFileSync(settings,readFileSync(settings,'utf8').replace('"language": "en"','"language": "en", "newUserField": "preserve"'));
  run('vim-restore',['--yes']);
  const restored = JSON.parse(readConfigFile(settings));
  assert.equal('vimMode' in restored.general,false);
  unchangedNumbers(restored);
  assert.equal(restored.general.newUserField,'preserve');
  assert.equal(restored.security.example,'SYNTHETIC-ONLY');
  assert.equal(existsSync(join(directory,'.agentdeck-qwen-vim.json')),false);
  const p2=run('vim-preview',['--enabled','true']);
  run('vim-apply',['--enabled','true','--preview-token',p2.token,'--yes']);
  restored.general.vimMode=false;writeFileSync(settings,JSON.stringify(restored));
  run('vim-restore',['--yes'],false);
  assert.equal(JSON.parse(readConfigFile(settings)).general.vimMode,false);
  // Clean this intentionally conflicting synthetic receipt only between fixtures.
  rmSync(join(directory,'.agentdeck-qwen-vim.json'));
 }
  console.log('Pinned Qwen 0.27.0 / 0.27.2 readConfigFile / strict JSON, apply, repeated managed updates, new-process read, original field restore and drift checks passed. No real Agent/authentication/effective layers tested.');
} finally {rmSync(directory,{recursive:true,force:true});}
