import metadata from '../../../docs/upstream/dsh/root-0.2.2-rc.1.json';
export const dshRootPlan={
 package:metadata.package,version:metadata.version,release_channel:'prerelease',archive_name:metadata.archive_name,url:metadata.url,sha256:metadata.sha256,integrity:metadata.integrity,bytes:metadata.bytes,max_read_bytes:65536,node_requirement:metadata.node,direct_dependency_count:90,direct_dependency_range_count:8,dependencies_resolved:false,installed:false,runtime_verified:false,signature_verified:false,platform_compatibility:'unverified',
} as const;
export type DshRootPlan=typeof dshRootPlan;
export interface DshRootVerification {state:'root_archive_verified';bytes:25197;plan:DshRootPlan}
function object(v:unknown):Record<string,unknown>{if(!v||typeof v!=='object'||Array.isArray(v))throw new Error('DSH_DISTRIBUTION_INVALID_RESPONSE');return v as Record<string,unknown>;}
export function validateDshRootPlan(v:unknown):DshRootPlan{const r=object(v);for(const [k,value]of Object.entries(dshRootPlan))if(r[k]!==value)throw new Error('DSH_DISTRIBUTION_INVALID_RESPONSE');return{...dshRootPlan};}
export function validateDshRootVerification(v:unknown):DshRootVerification{
 const r=object(v);if(r.state!=='root_archive_verified'||r.bytes!==25197||r.dependencies_resolved!==false||r.installed!==false||r.runtime_verified!==false||r.signature_verified!==false||r.platform_compatibility!=='unverified')throw new Error('DSH_DISTRIBUTION_INVALID_RESPONSE');
 return{state:'root_archive_verified',bytes:25197,plan:validateDshRootPlan(r.plan)};
}
