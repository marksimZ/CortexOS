import { useEffect,useRef,useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { AppDialog,openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { JobRequest, type PendingJob } from './JobRequest';
import { FitStatus } from './FitStatus';
import { errorText } from '../personalized';
import type { ControlScore } from './programmingTypes';

interface Patch {path:string;startLine:number;endLine:number;before:string;after:string;originalFileHash:string;proposedFileHash:string;unchangedOutsideRange:boolean;writtenToOriginal:false}
interface Event {kind:string;data:{code?:string;patch?:Patch;score?:ControlScore;reason?:string;[key:string]:unknown};createdAt:string}
interface Copy {id:string;planHash:string;status:string;error:string|null;pendingContainers:string[];events:Event[];plan:{checkId:string;datasetVersion:number;expiresAt:string;imageId:string;bundle:{originalCode:string;target:string;scope:string;taskJson:string}}}
interface Payload {request?:PendingJob;history:{id:string;status:string;createdAt:string}[];selected:Copy|null;active:{id:string;checkId:string;status:string}|null}
const names:Record<string,string>={opening:'正在准备副本',ready:'副本已准备',grading:'正在收集与独立测试',completed:'本次测试已完成',closed:'副本已关闭',cancelled:'已停止',failed:'本次操作未完成',interrupted:'上次操作中断',cleanup_required:'仍需清理'};
const grades:Record<string,string>={Passed:'通过',Failed:'未通过',Cancelled:'已停止','Execution Error':'执行出错','Scoring Error':'评分出错','Not Run':'未运行'};
const errors:Record<string,string>={workspace_expired:'临时副本已停止或过期，请清理后新建。',docker_unavailable:'Docker不可用，请启动Docker Desktop后重试。',docker_environment_error:'无法连接本地Docker引擎。',workspace_unexpected_file:'副本中出现了不允许的文件，已拒绝交接。',workspace_unexpected_directory:'副本中出现了不允许的目录，已拒绝交接。',workspace_task_changed:'副本题目发生变化，已拒绝交接。',container_cleanup_failed:'容器清理未完成，请重试清理。',cancelled:'操作已停止。'};
const problem=(text:string)=>errors[text]??(/(?:wall_timeout|output_limit)$/.test(text)?'准备或文件传输超出限制，未继续测试；请确认清理状态后重试。':/^[a-z_]+$/.test(text)?'本次副本操作未完成，请查看下方完整记录中的具体原因。':text);
const drafts=new Map<string,string>();
export function WorkspaceHandoff({workspaceId,assessmentVersion,checkId,checkHash,disabled}:{workspaceId:string;assessmentVersion:number;checkId:string;checkHash:string;disabled:boolean}){
 const [submitting,setSubmitting]=useState(false);
 const [data,setData]=useState<Payload|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState(''),[code,setCode]=useState(''),[action,setAction]=useState<'prepare'|'submit'|'close'|null>(null),[expired,setExpired]=useState(false),[stopping,setStopping]=useState(false);
 const [closeTarget,setCloseTarget]=useState<string|null>(null);
 const alive=useRef(true),epoch=useRef(0),selected=useRef<string|null>(null),dialog=useRef<HTMLDialogElement>(null),diff=useRef<HTMLDialogElement>(null);
 const record=data?.selected,working=record?.status==='opening'||record?.status==='grading',key=record?`${workspaceId}:${record.id}`:'';
 const request=data?.request,waiting=!!request||submitting;
 const read=async(id:string|null=selected.current)=>{const ticket=++epoch.current;const next=await invoke<Payload>('get_harness_workspace',{workspaceId,assessmentVersion,checkId,jobId:id});if(alive.current&&ticket===epoch.current){selected.current=next.selected?.id??null;setData(next);}};
 const act=async(fn:()=>Promise<void>)=>{setBusy(true);setError('');try{await fn();}catch(e){if(alive.current&&errorText(e)!=='已取消等待。')setError(problem(errorText(e)));}finally{if(alive.current)setBusy(false);}};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;epoch.current++;};},[]);
 useEffect(()=>{if(!record)return;const saved=record.events.find(e=>e.kind==='submission')?.data.code;setCode(drafts.get(`${workspaceId}:${record.id}`)??saved??record.plan.bundle.originalCode);},[record?.id]);
 useEffect(()=>{if(!working&&!waiting){setStopping(false);return;}let cancelled=false,timer:number;const tick=async()=>{try{await read(waiting?null:selected.current);}catch(e){if(alive.current)setError(errorText(e));}if(!cancelled)timer=window.setTimeout(()=>void tick(),1000);};timer=window.setTimeout(()=>void tick(),750);return()=>{cancelled=true;window.clearTimeout(timer);};},[working,waiting,record?.id]);
 useEffect(()=>{setExpired(false);if(record?.status!=='ready')return;const timer=window.setTimeout(()=>setExpired(true),Math.max(0,Date.parse(record.plan.expiresAt)-Date.now()));return()=>window.clearTimeout(timer);},[record?.id,record?.status]);
 useEffect(()=>{const warn=(e:BeforeUnloadEvent)=>{if(drafts.size){e.preventDefault();e.returnValue='';}};window.addEventListener('beforeunload',warn);return()=>window.removeEventListener('beforeunload',warn);},[]);
 const ask=(next:typeof action,target:string|null=record?.id??null)=>{setAction(next);setCloseTarget(target);openDialog(dialog.current);};
 const confirm=()=>act(async()=>{dialog.current?.close();setSubmitting(action!=='close');try{if(action==='prepare'){const next=await invoke<{id:string}>('prepare_harness_workspace',{workspaceId,waitForResources:true,input:{assessmentVersion,checkId,checkHash}});if(alive.current){selected.current=next.id;await read(next.id);}}else if(action==='submit'&&record){await invoke('submit_harness_workspace',{workspaceId,waitForResources:true,input:{jobId:record.id,planHash:record.planHash,code,confirmed:true}});drafts.delete(key);if(alive.current)await read();}else if(action==='close'&&closeTarget){await invoke('close_harness_workspace',{workspaceId,jobId:closeTarget});drafts.delete(`${workspaceId}:${closeTarget}`);if(alive.current)await read();}}finally{if(alive.current)setSubmitting(false);}});
 const patch=record?.events.find(e=>e.kind==='candidate')?.data.patch,score=record?.events.find(e=>e.kind==='score')?.data.score;
 const editor=<label className="profile-field">{record?.status==='ready'?'本次函数修改':'已提交的函数修改'}<textarea className="programming-code" aria-label="副本函数修改" readOnly={busy||working||waiting||record?.status!=='ready'||expired} maxLength={24000} rows={10} value={code} onChange={e=>{setCode(e.target.value);drafts.set(key,e.target.value);}}/></label>;
 const canPrepare=!busy&&!waiting&&!disabled&&!working&&!data?.active&&record?.status!=='ready'&&!record?.pendingContainers.length;
 return <details className="task-setup workspace-handoff" onToggle={e=>{if(e.currentTarget.open&&!data&&!busy)void act(()=>read(null));}}>
  <summary>任务副本与独立测试</summary>
  <section aria-label="任务副本与独立测试">
   <div className="cloud-actions"><button className="button secondary" disabled={!canPrepare} onClick={()=>ask('prepare')}>新建任务副本</button><button className="button text-button" disabled={busy||disabled} onClick={()=>void act(()=>read())}>重新读取副本记录</button>{working?<button className="button secondary" disabled={busy||stopping} onClick={()=>void act(async()=>{await invoke('close_harness_workspace',{workspaceId,jobId:record!.id});if(alive.current){setStopping(true);await read();}})}>停止并清理</button>:record&&(['ready','interrupted','cleanup_required'].includes(record.status)||record.pendingContainers.length>0)&&<button className="button secondary" disabled={busy||disabled||waiting} onClick={()=>ask('close')}>关闭并清理副本</button>}</div>
   {request&&<JobRequest key={request.id} job={request} onError={setError} cancel={()=>invoke('close_harness_workspace',{workspaceId,jobId:request.id,onlyWaiting:true})}/>}
   {data?.active&&data.active.id!==record?.id&&<div className="fit-meta-line"><span>此工作区还有一个未关闭副本。</span>{data.active.checkId===checkId?<button className="button secondary" onClick={()=>void act(()=>read(data.active!.id))}>查看未关闭副本</button>:<button className="button secondary" onClick={()=>ask('close',data.active!.id)}>关闭未完成副本</button>}</div>}
   {error&&<div className="cloud-error" role="alert"><p>{error}</p></div>}
   {data&&data.history.length>0&&<label className="profile-field">副本历史<SelectMenu aria-label="任务副本历史" disabled={busy||working||waiting} value={record?.id??''} onChange={e=>void act(()=>read(e.target.value))}>{data.history.map(h=><option key={h.id} value={h.id}>{new Date(h.createdAt).toLocaleString()} / {names[h.status]??h.status}</option>)}</SelectMenu></label>}
   {!request&&<p role="status">{busy?'正在处理本次副本…':stopping?'正在停止并清理容器…':expired?'副本已过期，请关闭并新建。':record?names[record.status]??record.status:'从已发布任务准备临时副本，手动验证修改与测试的交接。'}</p>}
   {record&&<>
    <div className="fit-meta-line"><FitStatus tone="neutral">人工交接验证</FitStatus><FitStatus tone="neutral">编程工具未运行</FitStatus>{record.pendingContainers.length>0&&<FitStatus tone="attention">待清理 {record.pendingContainers.length} 个容器</FitStatus>}</div>
    {record.error&&<p className="cloud-error">{problem(record.error)}</p>}
    <details className="personal-details"><summary>副本范围与题目</summary><p>仅复制已选函数，原文件 {record.plan.bundle.target.replace('project/','')} 的其他内容保持在原快照中。副本不含参考实现、测试断言或预期答案。</p><pre>{record.plan.bundle.taskJson}</pre><p>最多保留10分钟；停止后不自动恢复。未收集修改只保留在当前会话草稿，提交后保留独立记录。</p></details>
    {(record.status==='ready'||record.events.some(e=>e.kind==='submission'))&&<>{record.status==='ready'?editor:<details className="personal-details"><summary>查看已提交代码</summary>{editor}</details>}{record.status==='ready'&&<button className="button primary" disabled={busy||disabled||waiting||expired||!code.trim()||code===record.plan.bundle.originalCode} onClick={()=>ask('submit')}>收集修改并独立测试</button>}</>}
    {patch&&<div className="fit-meta-line"><span>{patch.path} / 原第 {patch.startLine}–{patch.endLine} 行</span><button className="button secondary" onClick={()=>openDialog(diff.current)}>查看本次修改</button><FitStatus tone="neutral">未写入原项目</FitStatus></div>}
    {score&&<><div className="fit-meta-line"><strong>独立测试结果</strong><FitStatus tone={score.state==='Passed'?'success':'attention'}>{grades[score.state]??score.state}</FitStatus><span>通过 {score.metrics.assertionsPassed??0} / 已运行 {score.metrics.assertionsRun??0}</span></div><details className="personal-details"><summary>逐条测试记录</summary><div className="task-checks"><table><thead><tr><th>测试</th><th>结果</th><th>实际观察</th></tr></thead><tbody>{score.details.assertions?.map(r=><tr key={r.index}><th scope="row">{r.index}</th><td>{r.passed?'通过':'未通过'}</td><td><details><summary>查看观察</summary><pre>{JSON.stringify(r,null,2)}</pre></details></td></tr>)}</tbody></table></div></details></>}
    {record.pendingContainers.length===0&&!working&&<p>本次已记录的容器均已清理。</p>}
    <details className="personal-details"><summary>来源、容器与完整记录</summary><pre>{JSON.stringify(record,null,2)}</pre></details>
   </>}
  </section>
  <AppDialog ref={dialog} className="panel-dialog form-dialog" aria-labelledby="workspace-confirm-title"><header className="panel-header"><h2 id="workspace-confirm-title">{action==='prepare'?'准备任务副本':action==='submit'?'收集修改并独立测试':'关闭任务副本'}</h2><button className="icon-button" aria-label="关闭副本确认" onClick={()=>dialog.current?.close()}><X size={18}/></button></header><div className="panel-body"><p>{action==='prepare'?'在本机Docker中创建临时函数副本，最多保留10分钟。本轮不会启动编程工具、调用模型或修改原项目。':action==='submit'?'本次修改将保存为独立记录；副本清理后，在新的受限容器中运行已审核的测试。不会把修改写回原项目，也不记录为编程工具的成绩。':'将清理本次副本并放弃未提交编辑；已保存的修改和测试记录会保留。'}</p></div><footer className="panel-footer dialog-actions"><button className="button secondary" onClick={()=>dialog.current?.close()}>返回</button><button className="button primary" disabled={busy} onClick={()=>void confirm()}>{action==='prepare'?'准备副本':action==='submit'?'确认并独立测试':'关闭并放弃未提交编辑'}</button></footer></AppDialog>
  <AppDialog ref={diff} className="panel-dialog fit-source-dialog" aria-labelledby="workspace-diff-title"><header className="panel-header"><h2 id="workspace-diff-title">本次函数修改</h2><button className="icon-button" aria-label="关闭本次修改" onClick={()=>diff.current?.close()}><X size={18}/></button></header><div className="panel-body">{patch&&<><p>{patch.path} / 未写入原项目</p><div className="workspace-diff"><section><h3>固定快照原文</h3><pre>{patch.before}</pre></section><section><h3>本次修改</h3><pre>{patch.after}</pre></section></div><details><summary>来源与修改指纹</summary><pre>{JSON.stringify({originalFileHash:patch.originalFileHash,proposedFileHash:patch.proposedFileHash,unchangedOutsideRange:patch.unchangedOutsideRange},null,2)}</pre></details></>}</div></AppDialog>
 </details>;
}
