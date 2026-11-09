import { scopeIds, type ScopePaths } from '../qoderScopeContract';
export const selectedScopes:ScopePaths={user_settings:'/user/settings.json',project_settings:'',local_settings:'',project_mcp:'/project/.mcp.json'};
export function rawScopes(paths:ScopePaths=selectedScopes){return{
 mode:'explicit_scope_summary',contract:'qoder_cli_documentation_2026_11_08',selection_issue:null,effective_configuration:'unknown',runtime_version:'unverified',can_apply:false,can_restore:false,
 limits:{max_files:4,max_manifest_bytes:65536,max_path_bytes:4096,max_path_components:64,max_json_depth:16,max_json_entries:4096,max_json_string_bytes:8192},
 scopes:scopeIds.map(scope=>({scope,status:paths[scope]?'read':'not_selected',issue:null,summary:paths[scope]?{mcp_server_count:2,mcp_section_present:true,permissions_present:false,hooks_present:false,model_present:true,security_present:false,other_top_level_fields_present:false}:null})),
 repeated_mcp_identifier_count:scopeIds.filter(id=>paths[id]).length>1?1:0, limitations:['SECRET_VALUE'],secret:'SECRET_NAME'
};}
