import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { AppDialog, openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { errorText } from '../personalized';
import { JobRequest, type PendingJob } from './JobRequest';
import { FitStatus } from './FitStatus';

interface Event {kind:string;data:Record<string,any>;createdAt:string}
interface Session {id:string;status:string;clean:boolean;pendingGraders:string[];modelCalls:number;error:string|null;plan:{model:{model:string;expectedDigest:string};datasetVersion:number};events:Event[]}
interface Payload {request?:PendingJob;history:{id:string;status:string;createdAt:string}[];selected:Session|null;active:{id:string;checkId:string;assessmentVersion:number;status:string}|null}
interface Model {name:string;digest:string|null}
const states:Record<string,string>={running:'正在执行',grading:'正在测试',completed:'执行完成',failed:'执行出错',unsupported:'不支持本次协议',cancelled:'已停止',interrupted:'执行中断',cleanup_required:'待清理',closed:'已清理'};
const scores:Record<string,string>={Passed:'测试通过',Failed:'测试未通过',Unsupported:'测试能力不支持','Not Run':'测试未运行','Execution Error':'测试环境出错','Scoring Error':'评分出错',Cancelled:'测试已停止','Needs Review':'需要复核'};
const reasons:Record<string,string>={aider_edit_outside_format:'回复未按允许修改的文件和对应 Python 代码块的格式返回，已停止。',aider_signature_changed:'回复改动了函数签名，已停止。',aider_edit_outside_function:'回复包含函数以外的代码，已停止。',aider_invalid_python:'回复中的 Python 代码无法解析，已停止。',aider_no_file_change:'Aider 未改动目标文件。',aider_task_failed:'Aider 执行出错。请展开记录查看。',model_request_timeout:'模型请求已超时。',cancelled:'本次执行已停止。',wall_timeout:'已达到 180 秒上限，停止本次执行。',request_budget_exceeded:'已达到 4 次请求上限，停止本次执行。',model_output_limit:'本次模型输出达到 512 Token 上限，未使用截断内容继续执行。',model_identity_changed:'所选模型版本发生变化，已停止转发。',codex_task_failed:'Codex 未能完成任务。请展开记录查看实际错误；没有自动重试。',tool_approval_required:'模型请求了额外权限，本次隔离任务不允许提权。',claude_task_failed:'Claude 未能完成任务，请查看实际错误；没有自动重试。',model_usage_missing:'模型响应缺少用量数据，本次通道已停止。',unsupported_tool:'编程工具提供了当前通道不支持的工具，未发送模型请求。',unknown_tool_call:'模型返回了未提供的工具名称，未交给编程工具执行。',invalid_tool_arguments:'模型返回的工具参数不符合结构要求，未执行。'};

interface SessionProps {workspaceId:string;assessmentVersion:number;checkId:string;runtimeId:string;runtimeHash:string;ready:boolean;disabled:boolean}
export function CodexSession(props:SessionProps){return <ToolSession {...props} tool="codex"/>;}
export function ClaudeSession(props:Omit<SessionProps,'checkId'>){return <ToolSession {...props} checkId="" tool="claude"/>;}
export function AiderSession(props:Omit<SessionProps,'checkId'>){return <ToolSession {...props} checkId="" tool="aider"/>;}
function ToolSession({workspaceId,assessmentVersion,checkId,runtimeId,runtimeHash,ready,disabled,tool}:SessionProps&{tool:'codex'|'claude'|'aider'}){
 const label=tool==='codex'?'Codex':tool==='claude'?'Claude':'Aider';
 const checkScope=tool==='codex'?{checkId}:{};
 const [data,setData]=useState<Payload|null>(null),[models,setModels]=useState<Model[]>([]),[model,setModel]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState(''),[stopping,setStopping]=useState(false);
 const [submitting,setSubmitting]=useState(false);
 const dialog=useRef<HTMLDialogElement>(null),alive=useRef(true),epoch=useRef(0),selected=useRef<string|null>(null);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;epoch.current++;};},[]);
 const read=async(id:string|null=selected.current)=>{const ticket=++epoch.current;const next=await invoke<Payload>(`get_harness_${tool}_session`,{workspaceId,assessmentVersion,...checkScope,jobId:id});if(alive.current&&ticket===epoch.current){selected.current=next.selected?.id??null;setData(next);if(!['running','grading'].includes(next.selected?.status??''))setStopping(false);}};
 const act=async(action:()=>Promise<void>)=>{setBusy(true);setError('');try{await action();}catch(e){if(alive.current&&errorText(e)!=='已取消等待。')setError(reasons[String(e)]??errorText(e));}finally{if(alive.current)setBusy(false);}};
 const load=async()=>{const results=await Promise.allSettled([read(),invoke<{models:Model[]}>('list_benchmark_models',{workspaceId})]);if(!alive.current)return;if(results[1].status==='fulfilled'){const list=results[1].value.models.filter(m=>m.digest);setModels(list);setModel(old=>list.some(m=>m.name===old)?old:list[0]?.name??'');}const fail=results.find(r=>r.status==='rejected');if(fail?.status==='rejected')throw fail.reason;};
 const request=data?.request, waiting=!!request||submitting;
 const record=data?.selected,working=!!record&&['running','grading'].includes(record.status);
 useEffect(()=>{if(!working&&!waiting)return;let disposed=false;let timer:ReturnType<typeof setTimeout>;const poll=async()=>{try{await read(waiting?null:selected.current);}catch(e){if(!disposed&&alive.current)setError(errorText(e));}if(!disposed)timer=setTimeout(()=>void poll(),700);};timer=setTimeout(()=>void poll(),700);return()=>{disposed=true;clearTimeout(timer);};},[working,waiting,workspaceId,assessmentVersion,checkId]);
 const chosen=models.find(m=>m.name===model);
 const start=()=>act(async()=>{dialog.current?.close();setSubmitting(true);try{if(!ready||!chosen?.digest)throw new Error('请先核对已通过的隔离检查和实际本地模型。');const next=await invoke<{id:string}>(`start_harness_${tool}_session`,{workspaceId,waitForResources:true,input:{assessmentVersion,...checkScope,runtimeId,runtimeHash,model:{model:chosen.name,expectedDigest:chosen.digest,parameters:{numCtx:4096,numPredict:512,temperature:0,timeoutSeconds:120},requirements:tool==='aider'?['completion']:['completion','tool_calls']},confirmed:true}});if(alive.current)await read(next.id);}finally{if(alive.current)setSubmitting(false);}});
 const clean=(id:string)=>act(async()=>{const result=await invoke<{stopping?:boolean}>(`cleanup_harness_${tool}_session`,{workspaceId,jobId:id});if(alive.current){setStopping(!!result.stopping);await read();}});
 const score=record?.events.find(e=>e.kind==='score')?.data.score;
 const candidate=record?.events.find(e=>e.kind==='candidate')?.data.patch;
 const changedFiles:{path:string;before:string;after:string}[]=candidate?.files??(candidate?[candidate]:[]);
 return <details className={`task-setup ${tool}-session`} onToggle={e=>{if(e.currentTarget.open&&!data&&!busy)void act(load);}}>
  <summary>{label} 单题执行</summary>
  <section aria-label={`${label}单题执行`} aria-busy={!request&&(busy||working||waiting)}>
   <div className="cloud-actions"><label>本地模型<SelectMenu aria-label={`${label}任务模型`} value={model} disabled={disabled||busy||working||waiting} onChange={e=>setModel(e.target.value)}><option value="">选择实际本地模型</option>{models.map(m=><option key={m.name} value={m.name}>{m.name}</option>)}</SelectMenu></label><button className="button secondary" disabled={busy||working||waiting} onClick={()=>void act(load)}>更新模型与执行记录</button><button className="button primary" disabled={!ready||disabled||busy||working||waiting||!chosen?.digest||!!data?.active} onClick={()=>openDialog(dialog.current)}>运行一个任务…</button>{working?<button className="button secondary" disabled={busy||stopping} onClick={()=>void clean(record.id)}>停止本次任务</button>:record&&(!record.clean||record.pendingGraders.length>0)&&<button className="button secondary" disabled={busy} onClick={()=>void clean(record.id)}>清理本次执行副本</button>}</div>
   {request&&<JobRequest key={request.id} job={request} onError={setError} cancel={()=>invoke(`cleanup_harness_${tool}_session`,{workspaceId,jobId:request.id,onlyWaiting:true})}/>}
   {!ready&&!working&&<p>选择已通过且已清理的 {label} 隔离检查后，才能运行新任务；已有执行记录仍可查看。</p>}
   {error&&<p className="cloud-error" role="alert">{error}</p>}
   {data?.active&&data.active.id!==record?.id&&<div className="fit-meta-line"><span>还有未结束的编程任务。</span><button className="button secondary" disabled={busy} onClick={()=>data.active?.checkId===checkId&&data.active.assessmentVersion===assessmentVersion?void act(()=>read(data.active!.id)):void clean(data.active!.id)}>{data.active.checkId===checkId&&data.active.assessmentVersion===assessmentVersion?'查看未结束任务':'停止或清理未结束任务'}</button></div>}
   {!!data?.history.length&&<label className="profile-field">任务执行历史<SelectMenu aria-label={`${label}任务执行历史`} value={record?.id??''} disabled={busy||working||waiting} onChange={e=>void act(()=>read(e.target.value))}>{data.history.map(r=><option key={r.id} value={r.id}>{new Date(r.createdAt).toLocaleString()} / {states[r.status]??r.status}</option>)}</SelectMenu></label>}
   {!request&&<p role="status">{stopping?'正在停止并清理…':busy?'正在读取本地条件…':working?'正在执行，实际事件会持续保留…':record?states[record.status]??record.status:'确认后才会调用模型，不会自动开始。'}</p>}
   {record&&<><div className="fit-meta-line"><strong>{record.plan.model.model}</strong><FitStatus tone="neutral">模型请求 {record.modelCalls} / 4</FitStatus><FitStatus tone={record.clean&&record.pendingGraders.length===0?'success':'attention'}>{record.clean&&record.pendingGraders.length===0?'副本已清理':'副本待清理'}</FitStatus><FitStatus tone={score?.state==='Passed'?'success':score?'attention':'neutral'}>{score?scores[score.state]??score.state:'测试未运行'}</FitStatus></div>
    {record.error&&<p className="cloud-error">{reasons[record.error]??record.error}</p>}
    {changedFiles.length>0&&<details className="personal-details"><summary>查看收集到的文件</summary>{changedFiles.map(file=><section key={file.path}><h4>{file.path}</h4><div className="workspace-diff"><div><strong>固定快照原文</strong><pre>{file.before}</pre></div><div><strong>副本内容</strong><pre>{file.after}</pre></div></div></section>)}</details>}
    {score&&<details className="personal-details"><summary>独立测试的逐条结果</summary><pre>{JSON.stringify(score,null,2)}</pre></details>}
    <details className="personal-details"><summary>实际执行记录与调用预算</summary><pre>{JSON.stringify(record,null,2)}</pre></details>
   </>}
  </section>
  <AppDialog ref={dialog} className="panel-dialog form-dialog" aria-labelledby={`${tool}-session-title`}><header className="panel-header"><h2 id={`${tool}-session-title`}>确认本次本地模型调用</h2><button className="icon-button" aria-label="关闭任务调用确认" onClick={()=>dialog.current?.close()}><X size={18}/></button></header><div className="panel-body"><p>使用 {chosen?.name} 执行当前已审核的一道编程题。本次可能加载模型并占用显存，请先关闭其他占用显存的任务。</p><dl className="fit-catalog-details"><dt>模型版本</dt><dd><code>{chosen?.digest}</code></dd><dt>调用上限</dt><dd>最多 4 次，4096 上下文，每次最多 512 Token</dd><dt>时间上限</dt><dd>任务 180 秒，单次请求 120 秒</dd><dt>任务范围</dt><dd>只允许任务版本批准的路径，评分在独立容器执行</dd></dl><p>保留无网络、无本机目录挂载、非管理员与原资源限额。{tool==='aider'?'使用 Docker 默认隔离，仅允许编辑任务副本中的目标文件。':'沿用已检查的兼容保护策略。'}达到预算、需要额外权限或协议不支持时停止，不自动重试。</p></div><footer className="panel-footer dialog-actions"><button className="button secondary" onClick={()=>dialog.current?.close()}>返回</button><button className="button primary" disabled={!ready||!chosen?.digest||busy||disabled||!!data?.active} onClick={()=>void start()}>确认调用并运行一题</button></footer></AppDialog>
 </details>;
}
