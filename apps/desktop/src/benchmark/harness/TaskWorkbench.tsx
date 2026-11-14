import { useEffect,useRef,useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FolderOpen,FileArchive,X } from 'lucide-react';
import { AppDialog,openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { FileName } from '../../components/FileName';
import { notify } from '../../components/Toast';
import { errorText,sourceLines } from '../personalized';
import type { Evidence,Requirement,Snapshot } from './types';
import { TaskReview } from './TaskReview';
import { ReadonlyRuns } from './ReadonlyRuns';
import { FitStatus } from './FitStatus';
import { ProgrammingDrafts } from './ProgrammingDrafts';
import { ExecutionPreflight } from './ExecutionPreflight';
import { ClaudeRuntime, AiderRuntime } from './ClaudeRuntime';
import { taskNames,type Project,type PublishedTasks,type TaskEdit,type TaskJob,type TaskPayload } from './taskTypes';
interface Location {path:string;start:number;end:number}
interface Scenario extends Location {kind:'call_chain'|'tool_call'|'permission_review';symbols:string[]}
export function TaskWorkbench({workspaceId,assessmentVersion,requirements,snapshots}:{workspaceId:string;assessmentVersion:number;requirements:Requirement[];snapshots:Snapshot[]}){
 const [data,setData]=useState<TaskPayload|null>(null),[job,setJob]=useState<TaskJob|null>(null),[sets,setSets]=useState<PublishedTasks|null>(null),[projects,setProjects]=useState(snapshots);
 const [project,setProject]=useState<Project|null>(null),[path,setPath]=useState(''),[start,setStart]=useState(1),[end,setEnd]=useState(1),[locations,setLocations]=useState<Location[]>([]),[ids,setIds]=useState<string[]>([]);
 const [taskKind,setTaskKind]=useState<'locate_excerpt'|Scenario['kind']>('locate_excerpt');
 const [chain,setChain]=useState(''),[scenarios,setScenarios]=useState<Scenario[]>([]);
 const [selection,setSelection]=useState<Record<string,number>>({}),[reviewer,setReviewer]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState(''),[editing,setEditing]=useState(false),[source,setSource]=useState<{path:string;content:string;start:number;end:number}|null>(null);
 const active=useRef(true),errorRef=useRef<HTMLDivElement>(null),dialog=useRef<HTMLDialogElement>(null);
 const act=async(fn:()=>Promise<void>)=>{setBusy(true);setError('');try{await fn();return true;}catch(e){if(active.current){setError(errorText(e));setTimeout(()=>errorRef.current?.focus(),0);}return false;}finally{if(active.current)setBusy(false);}};
 const load=async(jobId:string|null=null)=>{const next=await invoke<TaskPayload>('get_harness_tasks',{workspaceId,jobId});if(active.current){setData(next);setJob(next.selected);setSets(null);setSelection({});}};
 useEffect(()=>{active.current=true;void act(()=>load());return()=>{active.current=false;};},[workspaceId]);
 const selectSnapshot=(p:Project)=>{setProject(p);setLocations([]);setScenarios([]);setPath(p.files.find(f=>f.language!=='文档')?.path??'');setStart(1);setEnd(1);};
 const openProject=async(id:string)=>{const next=await invoke<Project>('get_harness_task_project',{workspaceId,snapshotId:id});if(active.current)selectSnapshot(next);};
 const pick=async(kind:string)=>{const p=await invoke<Project|null>('import_harness_repository',{workspaceId,kind});if(!active.current)return;if(!p){notify('已取消选择');return;}selectSnapshot(p);setProjects(old=>[{id:p.id,name:p.name,contentHash:p.contentHash},...old]);notify('项目快照已保存在本机');};
 const confirmed=requirements.filter(r=>['confirmed','edited'].includes(r.status)&&r.evidence);
 const file=project?.files.find(f=>f.path===path),lines=file?sourceLines(file.content):[];
 const count=ids.length+locations.length+scenarios.length;
 const symbols=chain.split(/[,，\n]/).map(s=>s.trim()).filter(Boolean);
 const addLocation=()=>{const loc={path,start,end};if(taskKind==='locate_excerpt')setLocations(old=>[...old,loc]);else setScenarios(old=>[...old,{...loc,kind:taskKind,symbols:taskKind==='call_chain'?symbols:[]}]);};
 const sourceOf=async(e:Evidence)=>{const p=await invoke<Project>('get_harness_task_project',{workspaceId,snapshotId:e.sourceId});if(!active.current)return;const f=p.files.find(f=>f.path===e.path);if(!f||f.contentHash!==e.contentHash)throw '原文校验失败，请保留本地数据。';setSource({path:e.path,content:f.content,start:e.startLine,end:e.endLine});openDialog(dialog.current);};
 const revise=async(id:string,revision:number,edit:TaskEdit)=>act(async()=>{const next=await invoke<TaskJob>('revise_harness_task',{workspaceId,jobId:job!.id,taskId:id,expectedRevision:revision,edit});if(active.current)setJob(next);notify('新修订已保存，请重新审核');});
 const version=job?Math.max(0,...(data?.tasksets.filter(s=>s.id===`harness-${job.id}`).map(s=>s.version)??[])):0;
 return <section className="harness-tasks" aria-label="项目任务生成与审核">
  <header className="fit-section-heading"><h3>测试任务</h3><div className="fit-meta-line"><span>本地生成</span><span>分析版本 {assessmentVersion}</span></div></header>
  {error&&<div className="cloud-error" role="alert" tabIndex={-1} ref={errorRef}><p>{error}</p><button className="button secondary" disabled={busy||editing} onClick={()=>void act(()=>load(job?.id??null))}>重新读取</button></div>}
  <details className="personal-details"><summary>本轮任务范围</summary><p>只读题基于已确认的规范和固定项目快照生成。编程任务在发布前执行独立的容器对照。</p></details>
  <details className="task-setup" open={!job&&!sets}><summary>生成候选任务</summary>
   <fieldset disabled={busy||editing}><legend>选择已确认的项目规范</legend>{confirmed.length===0?<p>先在“需求确认”中确认有原文的要求，并保存分析版本。</p>:<ul className="task-requirement-list">{confirmed.map(r=><li key={r.id}><label><input type="checkbox" checked={ids.includes(r.id)} disabled={!ids.includes(r.id)&&count>=12} onChange={e=>setIds(old=>e.target.checked?[...old,r.id]:old.filter(id=>id!==r.id))}/><span>{r.value}</span></label></li>)}</ul>}</fieldset>
   <details className="personal-details"><summary>添加代码文件定位任务</summary><div className="cloud-actions"><button className="button secondary" disabled={busy||editing} onClick={()=>void act(()=>pick('folder'))}><FolderOpen size={16} aria-hidden="true"/>选择项目文件夹</button><button className="button secondary" disabled={busy||editing} onClick={()=>void act(()=>pick('zip'))}><FileArchive size={16} aria-hidden="true"/>选择项目ZIP</button></div>
    <label className="profile-field">项目快照<SelectMenu aria-label="任务项目快照" disabled={busy||editing} value={project?.id??''} onChange={e=>{if(e.target.value)void act(()=>openProject(e.target.value));}}><option value="">选择已有项目</option>{projects.filter(s=>s.profile?.kind!=='harness_document').map(s=><option key={s.id} value={s.id}>{s.name}</option>)}</SelectMenu></label>
    {project&&<><label className="profile-field">代码或配置文件<SelectMenu aria-label="任务来源文件" disabled={busy||editing} value={path} onChange={e=>{setPath(e.target.value);setStart(1);setEnd(1);}}><option value="">选择文件</option>{project.files.filter(f=>f.language!=='文档').map(f=><option key={f.path} value={f.path}>{f.path}</option>)}</SelectMenu></label>{file&&<><label className="profile-field">题目类型<SelectMenu aria-label="只读任务类型" value={taskKind} onChange={e=>setTaskKind(e.target.value as typeof taskKind)}>{(['locate_excerpt','call_chain','tool_call','permission_review'] as const).map(kind=><option key={kind} value={kind}>{taskNames[kind]}</option>)}</SelectMenu></label>{taskKind==='call_chain'&&<label className="profile-field">依次调用的函数（每行一个模块限定名称）<textarea aria-label="调用链函数" value={chain} rows={3} onChange={e=>setChain(e.target.value)}/></label>}<div className="task-range"><label>开始行<input aria-label="任务开始行" type="number" min={1} max={lines.length} value={start} onChange={e=>setStart(Number(e.target.value))}/></label><label>结束行<input aria-label="任务结束行" type="number" min={start} max={Math.min(lines.length,start+29)} value={end} onChange={e=>setEnd(Number(e.target.value))}/></label><button className="button secondary" disabled={busy||editing||start<1||end<start||end>lines.length||end-start>=30||count>=12||(taskKind==='call_chain'&&(symbols.length<2||symbols.length>5))} onClick={addLocation}>加入任务来源</button></div><pre className="personal-code">{lines.slice(Math.max(0,start-1),Math.min(lines.length,end)).map((s,i)=>`${i+start}  ${s}`).join('\n')}</pre></>}</>}
    <ul className="personal-list">{locations.map((l,i)=><li key={i}><FileName name={l.path}/><span>行 {l.start}–{l.end}</span><button className="button text-button" disabled={busy||editing} onClick={()=>setLocations(old=>old.filter((_,index)=>i!==index))}>移除</button></li>)}{scenarios.map((s,i)=><li key={`scenario-${i}`}><FileName name={s.path}/><span>{taskNames[s.kind]}，第 {s.start}–{s.end} 行</span><button className="button text-button" disabled={busy||editing} onClick={()=>setScenarios(old=>old.filter((_,index)=>i!==index))}>移除</button></li>)}</ul><details><summary>导入范围</summary><p>复用现有只读项目扫描：最多2000条目、单文件512 KiB、文本合计20 MiB、ZIP最多50 MiB。排除链接、依赖、产物和已识别的敏感内容；不运行脚本、不上传资料。</p></details>
   </details>
   <button className="button primary" disabled={busy||editing||count===0} onClick={()=>void act(async()=>{const next=await invoke<TaskJob>('create_harness_tasks',{workspaceId,input:{assessmentVersion,repositoryId:project?.id??null,requirementIds:ids,locations,scenarios}});if(!active.current)return;await load(next.id);setIds([]);setLocations([]);setScenarios([]);notify('候选已生成，请逐题审核');})}>生成 {count} 项候选任务</button>
  </details>
  <ProgrammingDrafts workspaceId={workspaceId} assessmentVersion={assessmentVersion} snapshots={projects} disabled={busy||editing} onSource={e=>void act(()=>sourceOf(e))}/>
  <ExecutionPreflight workspaceId={workspaceId} assessmentVersion={assessmentVersion} disabled={busy||editing}/>
  <ClaudeRuntime key={`claude-${workspaceId}-${assessmentVersion}`} workspaceId={workspaceId} assessmentVersion={assessmentVersion} disabled={busy||editing}/>
  <AiderRuntime key={`aider-${workspaceId}-${assessmentVersion}`} workspaceId={workspaceId} assessmentVersion={assessmentVersion} disabled={busy||editing}/>
  {busy&&<p role="status">正在处理本地任务…</p>}
  {data&&<div className="task-history"><label>任务草案<SelectMenu aria-label="任务草案历史" value={sets?'':job?.id??''} disabled={busy||editing} onChange={e=>{if(e.target.value)void act(()=>load(e.target.value));}}><option value="">选择草案</option>{data.history.map(h=><option value={h.id} key={h.id}>{new Date(h.createdAt).toLocaleString()} · {h.id.slice(0,8)}</option>)}</SelectMenu></label><label>已发布版本<SelectMenu aria-label="已发布任务版本" value={sets?`${sets.provenance.generationJobId}:${sets.version}`:''} disabled={busy||editing} onChange={e=>{if(!e.target.value)return;const s=data.tasksets.find(s=>`${s.id.replace(/^harness-/,'')}:${s.version}`===e.target.value);if(s)void act(async()=>{const result=await invoke<PublishedTasks>('get_harness_taskset',{workspaceId,datasetId:s.id,version:s.version});if(active.current){setSets(result);setSelection({});}});}}><option value="">选择已发布版本</option>{data.tasksets.map(s=><option key={`${s.id}:${s.version}`} value={`${s.id.replace(/^harness-/,'')}:${s.version}`}>版本 {s.version} · {s.count} 项 · {new Date(s.createdAt).toLocaleString()}</option>)}</SelectMenu></label></div>}
  {(sets||job)&&<><div className="task-review-toolbar"><strong>{sets?`已发布版本 ${sets.version}`:'逐题审核'}（{(sets?.tasks??job!.candidates).length} 项）</strong>{!sets&&<><label>审核人<input aria-label="任务审核人" maxLength={100} value={reviewer} onChange={e=>setReviewer(e.target.value)}/></label><button className="button primary" disabled={busy||editing||!reviewer.trim()||!Object.keys(selection).length} onClick={()=>void act(async()=>{await invoke('publish_harness_tasks',{workspaceId,jobId:job!.id,selection:Object.entries(selection).map(([id,revision])=>({id,revision,approved:true})),reviewer,expectedVersion:version});if(!active.current)return;await load(job!.id);notify('任务集新版本已发布');})}>发布已批准的 {Object.keys(selection).length} 项</button></>}</div>
   <div className="fit-meta-line"><span>固定分析版本 {(sets?.plan??job!.plan).assessmentVersion}</span><FitStatus tone="attention">任务尚未运行</FitStatus>{!sets&&job!.plan.assessmentVersion!==assessmentVersion&&<span>此草案使用较早的分析版本</span>}</div>
   <TaskReview key={`${job?.id}:${sets?.version??'draft'}:${version}`} draftKey={`${workspaceId}:${job?.id??'published'}`} items={sets?.tasks??job!.candidates} busy={busy} published={sets!==null} selection={selection} onRevise={revise} onSelection={setSelection} onEditing={setEditing} onSource={e=>void act(()=>sourceOf(e))}/>
   {sets&&<ReadonlyRuns key={`${sets.provenance.generationJobId}:${sets.version}`} workspaceId={workspaceId} published={sets}/>}
   <details className="personal-details"><summary>版本与指纹</summary><pre>{JSON.stringify(sets?{version:sets.version,contentHash:sets.content_hash,plan:sets.plan}:job!.plan,null,2)}</pre></details>
  </>}
  <AppDialog ref={dialog} className="panel-dialog fit-source-dialog" aria-labelledby="task-source-title"><header className="panel-header"><h2 id="task-source-title">{source?.path} · 行 {source?.start}–{source?.end}</h2><button className="icon-button" aria-label="关闭任务原文" onClick={()=>dialog.current?.close()}><X size={18}/></button></header><div className="panel-body"><pre>{source?.content.split('\n').map((line,i)=><span className="fit-source-line" key={i} data-active={i+1>=source.start&&i+1<=source.end} ref={el=>{if(el&&i+1===source.start)el.scrollIntoView({block:'center'});}}>{i+1}  {line}{'\n'}</span>)}</pre></div></AppDialog>
 </section>;
}
