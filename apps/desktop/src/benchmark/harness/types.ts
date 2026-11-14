export interface Evidence {sourceId:string;path:string;contentHash:string;startLine:number;endLine:number;excerpt:string}
export interface Requirement {id:string;category:string;originalValue:string;value:string;criterion:string|null;status:'candidate'|'unknown'|'confirmed'|'edited'|'rejected';confidence:'literal_match_unverified'|'unknown';hard:boolean;evidence:Evidence|null}
export interface Fact {value:boolean|null;basis:string;note:string;source:string|null}
export interface Harness {id:string;name:string;website:string;version:string|null;channel:string;checkedAt:string;runtimeVerified:boolean;operatingSystems:string;modelAccess:string;authenticationAndCost:string;contextMechanism:string;toolMechanism:string;facts:Record<string,Fact>}
export interface Catalog {version:string;checkedAt:string;sources:Record<string,{title:string;url:string;checkedAt:string}>;entries:Harness[]}
export interface FitResult {harnessId:string;eligibility:string;lowerPercent:number|null;upperPercent:number|null;evidenceCoverage:number|null;weights:{supported:number;unknown:number;incompatible:number;total:number};verifiedCount:number;rows:{requirementId:string;weight:number;state:string;fact:Fact|null;provisional:boolean}[]}
export interface Body {snapshotIds:string[];documents:{sourceId:string;path:string;contentHash:string}[];requirements:Requirement[];weights:Record<string,number>;note:string;catalog:Catalog;catalogHash:string;report:{kind:string;engineVersion:string;results:FitResult[]}}
export interface Assessment {version:number;body:Body;contentHash:string;createdAt:string}
export interface Snapshot {id:string;name:string;contentHash:string;profile?:{kind?:string}}
export interface Payload {selected:Assessment|null;history:{version:number;contentHash:string;createdAt:string}[];snapshots:Snapshot[];catalog:Catalog;categories:{id:string;label:string}[];criteria:Record<string,{label:string;group:string}>}
export const statuses={candidate:'待确认线索',unknown:'未找到明确要求',confirmed:'用户确认',edited:'用户修改',rejected:'忽略此项'};
export const groups={compatibility:'运行与模型接入',automation:'自动化与事件',governance:'权限与审批',context:'项目指令与上下文'};
