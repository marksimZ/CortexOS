import type { Evidence } from './types';
export interface ProgrammingOrigin {assessmentVersion:number;repositoryId:string;path:string;start:number;end:number}
export interface ProgrammingExample {label:string;role:'target'|'boundary'|'regression';args:unknown[];output:unknown}
export interface ProjectProgrammingSpec {
 kind:'multi_file'|'regression'|'add_tests'|'database_compatibility';
 files:string[];
 reference:Record<string,string>;
 negative:Record<string,string>;
}
export interface ProgrammingSpec {basis:'confirmed_requirement'|'reported_bug';rationale:string;prompt:string;function:string;parameters:{name:string;kind:string}[];returnType:string;examples:ProgrammingExample[];referenceCode:string;wrongCode:string;project?:ProjectProgrammingSpec}
export interface ProgrammingBody {origin:ProgrammingOrigin;source:Evidence;assessmentHash:string;repositoryHash:string;spec:ProgrammingSpec;case:unknown;policy:unknown;validation:{staticValid:boolean;publishable:false;taskExecution:'Not Run';containerValidation:'Not Run';checks:{name:string;expected:string;actual:'Not Run'}[];warnings:string[]}}
export interface ProgrammingRevision {revision:number;body:ProgrammingBody;contentHash:string;createdAt:string}
export interface ProgrammingJob {id:string;revisions:ProgrammingRevision[]}
export interface ProgrammingPayload {history:{id:string;createdAt:string}[];selected:ProgrammingJob|null}
export interface ControlScore {state:string;metrics:{assertionsRun?:number;assertionsPlanned?:number;assertionsPassed?:number};details:{reason:string;buildStatus?:string;assertions?:{index:number;passed:boolean;reason?:string;observation?:unknown}[];ownedContainers?:{name:string;removed:boolean}[]}}
export interface ProgrammingValidation {id:string;status:string;plan:{draftId:string;revision:number;draftHash:string;imageId:string;codeHashes:Record<string,string>;evaluatorVersion:string};planHash:string;contentHash:string;error:string|null;createdAt:string;finishedAt:string|null;records:Record<string,{score?:ControlScore;startedAt?:string;finishedAt?:string;[key:string]:unknown}>;examples:ProgrammingExample[];comparison:{passed:boolean;rulesVersion:string;checks:Record<string,boolean>};pendingContainers:string[];publishable:boolean;evidenceBound:boolean}
export interface ValidationPayload {history:{id:string;status:string;createdAt:string;revision:number}[];selected:ProgrammingValidation|null;published:{id:string;version:number;contentHash:string;createdAt:string}[]}
export interface ProgrammingPublished {name:string;version:number;content_hash:string;execution:string;payload:{task:ProgrammingRevision;case:unknown;validation:ProgrammingValidation;review:{reviewer:string;createdAt:string};taskExecution:string}}
