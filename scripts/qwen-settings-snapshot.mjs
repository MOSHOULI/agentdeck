// Invoked only by test-qwen-settings-runtime.py inside its mandatory network namespace.
// Import the unmodified settings implementation from the hash-pinned official package.
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
const [packagePath, workspace, expected, trusted] = process.argv.slice(2);
assert.ok(expected === 'true' || expected === 'false');
assert.ok(trusted === 'true' || trusted === 'false');
const { readSettingsSnapshot } = await import(pathToFileURL(join(packagePath, 'chunks/chunk-E6R76DS6.js')));
const settings = readSettingsSnapshot(workspace, { environment: process.env, workspaceTrusted: trusted === 'true' });
assert.equal(settings.merged.general.vimMode, expected === 'true');
console.log(JSON.stringify({ vimMode: settings.merged.general.vimMode, workspaceTrusted: trusted === 'true' }));
