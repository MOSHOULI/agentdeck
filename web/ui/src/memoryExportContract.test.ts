import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { readMemoryExport, previewMemoryExport, replaceMemoryExport } from './bridge';
import { validateExportSnapshot, validateExportPreview, validateExportReceipt } from './memoryExportContract';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const row = {id:'1',agentId:'codex',content:'text',contentHash:'b'.repeat(64),collectedAt:1,autoInject:false,tags:[]};
const snapshot = {agentId:'codex',databaseExists:true,stateToken:'a'.repeat(64),manualEntries:[row],automaticEntries:[]};
const preview = {agentId:'codex',stateToken:snapshot.stateToken,previewToken:'c'.repeat(64),manualEntries:[row],automaticEntries:[],outputEntries:[row]};
beforeEach(() => { vi.resetAllMocks(); Object.assign(window, {__TAURI_INTERNALS__: {}}); });
afterEach(() => { delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; window.history.replaceState({}, '', '/'); });
it('dispatches exact camelCase payloads and validates all three responses', async () => {
  vi.mocked(invoke).mockResolvedValueOnce(snapshot).mockResolvedValueOnce(preview).mockResolvedValueOnce({agentId:'codex',manualIds:['1'],exportedCount:1});
  await expect(readMemoryExport('codex')).resolves.toEqual(snapshot);
  await expect(previewMemoryExport('codex',snapshot.stateToken,['1'])).resolves.toEqual(preview);
  await expect(replaceMemoryExport('codex',preview.previewToken,['1'])).resolves.toEqual({agentId:'codex',manualIds:['1'],exportedCount:1});
  expect(invoke).toHaveBeenNthCalledWith(1,'read_memory_export',{agentId:'codex'});
  expect(invoke).toHaveBeenNthCalledWith(2,'preview_memory_export',{agentId:'codex',expectedStateToken:snapshot.stateToken,memoryIds:['1']});
  expect(invoke).toHaveBeenNthCalledWith(3,'replace_memory_export',{agentId:'codex',previewToken:preview.previewToken,memoryIds:['1']});
});
it.each(['/', '/?demo=1'])('never dispatches or fabricates success outside native %s', async url => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  window.history.replaceState({}, '', url);
  await expect(readMemoryExport('codex')).rejects.toThrow();
  await expect(previewMemoryExport('codex',snapshot.stateToken,['1'])).rejects.toThrow();
  await expect(replaceMemoryExport('codex',preview.previewToken,['1'])).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
it.each([['01'],['0'],['-1'],['9223372036854775808'],['1','1'],Array.from({length:201},(_,i)=>String(i+1))].map(ids => ({ids})))('rejects invalid ID proposals before IPC: $ids', async ({ids}) => {
  await expect(previewMemoryExport('codex',snapshot.stateToken,ids)).rejects.toThrow();
  await expect(replaceMemoryExport('codex',preview.previewToken,ids)).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
it('rejects target aliases and malformed tokens before IPC', async () => {
  await expect(readMemoryExport('claude')).rejects.toThrow();
  await expect(previewMemoryExport('codex','bad',['1'])).rejects.toThrow();
  await expect(replaceMemoryExport('codex','bad',['1'])).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
it.each([
  {...snapshot,agentId:'opencode'}, {...snapshot,unexpected:true}, {...snapshot,stateToken:'bad'},
  {...snapshot,databaseExists:false}, {...snapshot,manualEntries:[{...row,sourceSessionId:'private'}]},
  {...snapshot,manualEntries:[{...row,collectedAt:1.5}]}, {...snapshot,manualEntries:[{...row,content:'中'.repeat(22000)}]},
  {...snapshot,automaticEntries:[row]}, {...snapshot,automaticEntries:[{...row,id:'2',agentId:'opencode',autoInject:true}]},
  {...snapshot,manualEntries:[{...row,agentId:'C:/private/path'}]},
  {...snapshot,manualEntries:[row,row]}, {...snapshot,manualEntries:[{...row,tags:['x'.repeat(2097152)]}]},
])('rejects malformed, oversized, foreign or unexpected snapshot fields', value => {
  expect(()=>validateExportSnapshot(value,'codex')).toThrow();
});
it('rejects snapshot union >500 even when each separate array is within limit', () => {
  const automatic = Array.from({length:500},(_,i)=>({...row,id:String(i+2),autoInject:true}));
  expect(()=>validateExportSnapshot({...snapshot,automaticEntries:automatic},'codex')).toThrow();
});
it('validates preview union, target, prior token and matching row detail', () => {
  for (const value of [
    {...preview,stateToken:'d'.repeat(64)}, {...preview,agentId:'opencode'}, {...preview,manualEntries:[]},
    {...preview,outputEntries:[]}, {...preview,outputEntries:[{...row,content:'changed'}]},
    {...preview,automaticEntries:[{...row,id:'2',autoInject:true}]}, {...preview,unexpected:true},
  ]) expect(()=>validateExportPreview(value,'codex',snapshot.stateToken,['1'])).toThrow();
});
it('allows overlap and mixed-source manual rows with exact output union', () => {
  const auto = {...row,autoInject:true};
  const mixed = {...row,id:'2',agentId:'claude-code'};
  expect(validateExportPreview({...preview,manualEntries:[auto,mixed],automaticEntries:[auto],outputEntries:[auto,mixed]},'codex',snapshot.stateToken,['2','1']).outputEntries).toHaveLength(2);
});
it('rejects malformed receipts, wrong target or changed proposal', () => {
  for (const value of [
    {agentId:'opencode',manualIds:['1'],exportedCount:1}, {agentId:'codex',manualIds:[],exportedCount:1},
    {agentId:'codex',manualIds:['1'],exportedCount:0}, {agentId:'codex',manualIds:['1'],exportedCount:501},
    {agentId:'codex',manualIds:['1'],exportedCount:1,unexpected:true},
  ]) expect(()=>validateExportReceipt(value,'codex',['1'])).toThrow();
});
