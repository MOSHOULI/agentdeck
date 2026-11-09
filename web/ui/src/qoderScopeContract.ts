export const scopeIds = ['user_settings','project_settings','local_settings','project_mcp'] as const;
export type ScopeId = typeof scopeIds[number];
export type ScopePaths = Record<ScopeId,string>;
export const scopeLabels: Record<ScopeId,string> = { user_settings:'用户设置 settings.json', project_settings:'项目设置 settings.json', local_settings:'本地设置 settings.local.json', project_mcp:'项目 MCP .mcp.json' };
export const flags = ['mcp_section_present','permissions_present','hooks_present','model_present','security_present','other_top_level_fields_present'] as const;
export const flagLabels: Record<typeof flags[number],string> = {mcp_section_present:'MCP 分区',permissions_present:'权限分区',hooks_present:'钩子分区',model_present:'模型分区',security_present:'安全分区',other_top_level_fields_present:'其他顶层字段'};
type Summary = Record<typeof flags[number],boolean> & {mcp_server_count:number};
export type Observation = {scope:ScopeId;status:'not_selected'|'read'|'missing'|'unsupported'|'unreadable';summary:Summary|null};
export type ScopeReport = {scopes:Observation[];overlap:number|null};
const invalid=()=>new Error('QODER_SCOPE_INVALID_RESPONSE');
function object(v:unknown):Record<string,unknown>{if (!v || typeof v!=='object'||Array.isArray(v))throw invalid();return v as Record<string,unknown>;}
const count=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0&&v<=4096;
const issues=['path_must_be_absolute','parent_traversal','unsupported_path','other_selection_invalid','path_limit','unexpected_file_name','missing','permission_denied','io_failure','link_or_reparse_point','not_directory','not_regular_file','multiple_hard_links','file_changed','file_too_large','unsupported_json','duplicate_json_key','json_limit','unsupported_shape','unsupported_schema','unrecognized_manifest','manifest_required','platform_unsupported'];
export function validateScopeReport(value:unknown,paths:ScopePaths):ScopeReport {
  const r=object(value);
  if(r.mode!=='explicit_scope_summary'||r.contract!=='qoder_cli_documentation_2026_11_08'||r.selection_issue!==null||r.effective_configuration!=='unknown'||r.runtime_version!=='unverified'||r.can_apply!==false||r.can_restore!==false)throw invalid();
  const limits=object(r.limits);const expected={max_files:4,max_manifest_bytes:65536,max_path_bytes:4096,max_path_components:64,max_json_depth:16,max_json_entries:4096,max_json_string_bytes:8192};
  for(const [k,v] of Object.entries(expected))if(limits[k]!==v)throw invalid();
  if(!Array.isArray(r.scopes)||r.scopes.length!==4)throw invalid();
  const scopes=r.scopes.map((raw,i):Observation=>{
    const o=object(raw),scope=scopeIds[i];if(o.scope!==scope)throw invalid();
    if(!paths[scope]){if(o.status!=='not_selected'||o.issue!==null||o.summary!==null)throw invalid();return{scope,status:'not_selected',summary:null};}
    if(o.status==='read'){
      if(o.issue!==null)throw invalid();const summary=object(o.summary);if(!count(summary.mcp_server_count))throw invalid();
      const fixed={mcp_server_count:summary.mcp_server_count} as Summary;
      for(const f of flags){if(typeof summary[f]!=='boolean')throw invalid();fixed[f]=summary[f];}
      if(!fixed.mcp_section_present&&fixed.mcp_server_count!==0)throw invalid();return{scope,status:'read',summary:fixed};
    }
    if(o.summary!==null||typeof o.issue!=='string'||!issues.includes(o.issue))throw invalid();
    const status=o.issue==='missing'?'missing':['permission_denied','io_failure','file_changed'].includes(o.issue)?'unreadable':'unsupported';
    if(o.status!==status)throw invalid();return{scope,status,summary:null};
  });
  const complete=scopes.every(s=>s.status==='read'||s.status==='not_selected');
  const overlap=r.repeated_mcp_identifier_count; const read=scopes.filter(s=>s.status==='read');
  const maxOverlap=read.length<2?0:Math.floor(read.reduce((sum,s)=>sum+(s.summary?.mcp_server_count??0),0)/2);
  if(!scopeIds.some(id=>!!paths[id])||(complete?typeof overlap!=='number'||!Number.isSafeInteger(overlap)||overlap<0||overlap>maxOverlap:overlap!==null))throw invalid();
  return{scopes,overlap:r.repeated_mcp_identifier_count as number|null};
}
