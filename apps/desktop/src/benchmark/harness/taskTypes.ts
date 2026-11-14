import type { Evidence } from './types';
export interface ProjectFile {path:string;language:string;content:string;contentHash:string}
export interface Project {id:string;name:string;contentHash:string;files:ProjectFile[]}
export interface TaskEdit {prompt:string;rationale:string}
export interface ProjectTask {kind:keyof typeof taskNames;case:{case_id:string;input:{prompt:string};expected:{answers?:string[];name?:string;arguments?:unknown};scoring:unknown};rationale:string;sources:Evidence[];policy:{allowedPaths:string[];timeoutSeconds:number;maxOutputBytes:number;tools?:string}}
export interface TaskItem {id:string;revision:number;task:ProjectTask;validation:{valid:boolean;checks:{name:string;input:string;expected:string;actual:string;passed:boolean}[];evaluatorVersion:string;taskExecution:string;warnings:string[]}}
export interface TaskJob {id:string;plan:{assessmentVersion:number;assessmentHash:string;generatorVersion:string;repositoryId:string|null;repositoryHash:string|null};planHash:string;createdAt:string;candidates:TaskItem[]}
export interface Taskset {id:string;version:number;contentHash:string;createdAt:string;count:number}
export interface TaskPayload {selected:TaskJob|null;history:{id:string;createdAt:string}[];tasksets:Taskset[]}
export interface PublishedTasks {version:number;content_hash:string;execution:string;plan:TaskJob['plan'];tasks:TaskItem[];provenance:{generationJobId:string}}
export const taskNames={requirement_quote:'规范原文复述',locate_excerpt:'代码文件定位',call_chain:'Python 调用链定位',tool_call:'原生工具调用',permission_review:'权限审批'};
export const checkNames:Record<string,string>={reference_answer:'参考答案',empty_answer:'空答案',wrong_answer:'错误答案',unexpected_tool_call:'多余工具调用'};
