import { draftStore } from "../../draft-store";
import { useEffect,useRef,useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { AppDialog,openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { notify } from '../../components/Toast';
import { errorText,sourceLines } from '../personalized';
import { FitStatus } from './FitStatus';
import { ProgrammingValidation } from './ProgrammingValidation';
import type { Evidence,Snapshot } from './types';
import type { Project } from './taskTypes';
import type { ProgrammingJob,ProgrammingOrigin,ProgrammingPayload,ProgrammingRevision,ProgrammingSpec,ProjectProgrammingSpec } from './programmingTypes';

interface Row {label:string;role:'target'|'boundary'|'regression';args:string;output:string}
interface Editor {jobId:string|null;revision:number;origin:ProgrammingOrigin;spec:Omit<ProgrammingSpec,'examples'>;rows:Row[];dirty:boolean}
const cached=draftStore<Editor>("programming", value => !!(value as Editor | null)?.origin && Array.isArray((value as Editor).rows));
const types:Record<string,string>={integer:'整数',string:'文本',boolean:'布尔值',integer_list:'整数列表',string_list:'文本列表',boolean_list:'布尔值列表'};
const projectKinds={multi_file:'跨文件修改',regression:'仓库回归验证',add_tests:'补写测试',database_compatibility:'数据库兼容性'};
const blank=(version:number):Editor=>({jobId:null,revision:0,origin:{assessmentVersion:version,repositoryId:'',path:'',start:1,end:1},spec:{basis:'confirmed_requirement',rationale:'',prompt:'',function:'',parameters:[{name:'',kind:'integer'}],returnType:'integer',referenceCode:'',wrongCode:''},rows:[],dirty:false});
const from=(job:ProgrammingJob,r:ProgrammingRevision):Editor=>({jobId:job.id,revision:r.revision,origin:r.body.origin,spec:r.body.spec,rows:r.body.spec.examples.map(e=>({...e,args:JSON.stringify(e.args),output:JSON.stringify(e.output)})),dirty:false});
export function ProgrammingDrafts({workspaceId,assessmentVersion,snapshots,disabled,onSource}:{workspaceId:string;assessmentVersion:number;snapshots:Snapshot[];disabled:boolean;onSource:(e:Evidence)=>void}){
 const draftKey=`${workspaceId}:${assessmentVersion}`;
 const [s,setS]=useState<Editor>(()=>cached.get(draftKey)??blank(assessmentVersion)),[data,setData]=useState<ProgrammingPayload|null>(null),[project,setProject]=useState<Project|null>(null),[projects,setProjects]=useState(snapshots);
 const [busy,setBusy]=useState(false),[error,setError]=useState(''),[confirmText,setConfirmText]=useState(''),[validationRunning,setValidationRunning]=useState(false);
 const active=useRef(true),epoch=useRef(0),errorRef=useRef<HTMLDivElement>(null),confirmDialog=useRef<HTMLDialogElement>(null),pending=useRef<()=>void>(()=>{});
 const change=(patch:Partial<Editor>)=>setS(old=>{const next={...old,...patch,dirty:true};cached.set(draftKey,next);return next;});
 const spec=(patch:Partial<Editor['spec']>)=>change({spec:{...s.spec,...patch}});
 const act=async(fn:()=>Promise<void>)=>{setBusy(true);setError('');try{await fn();}catch(e){if(active.current){setError(errorText(e));setTimeout(()=>errorRef.current?.focus(),0);}}finally{if(active.current)setBusy(false);}};
 const getProject=async(id:string)=>invoke<Project>('get_harness_task_project',{workspaceId,snapshotId:id});
 const read=async(id:string|null=null,restore=false)=>{
  const ticket=++epoch.current,next=await invoke<ProgrammingPayload>('get_harness_programming_drafts',{workspaceId,jobId:restore?(cached.get(draftKey)?.jobId??id):id});
  const editor=restore&&cached.has(draftKey)?cached.get(draftKey)!:next.selected?from(next.selected,next.selected.revisions[0]):blank(assessmentVersion);
  const p=editor.origin.repositoryId?await getProject(editor.origin.repositoryId):null;
  if(active.current&&ticket===epoch.current){if(!restore)cached.delete(draftKey);setData(next);setS(editor);setProject(p);}
 };
 useEffect(()=>{active.current=true;void act(()=>read(null,true));return()=>{active.current=false;epoch.current++;};},[draftKey]);
 const confirm=(action:()=>void)=>{if(!s.dirty){action();return;}pending.current=action;setConfirmText('放弃尚未保存的编程草案修改？已保存的修订会保留。');openDialog(confirmDialog.current);};
 const selectSnapshot=(p:Project)=>{
  const path=p.files.find(f=>f.path.endsWith('.py'))?.path??'';
  setProject(p);
  change({origin:{...s.origin,repositoryId:p.id,path,start:1,end:1},spec:{...s.spec,project:s.spec.project&&{...s.spec.project,files:path?[path]:[],reference:{},negative:{}}}});
 };
 const selectProject=async(id:string)=>{const p=await getProject(id);if(active.current)selectSnapshot(p);};
 const importProject=async(kind:string)=>{const p=await invoke<Project|null>('import_harness_repository',{workspaceId,kind});if(!active.current)return;if(!p){notify('已取消选择');return;}setProjects(old=>[{id:p.id,name:p.name,contentHash:p.contentHash},...old]);selectSnapshot(p);};
 const row=(index:number,patch:Partial<Row>)=>change({rows:s.rows.map((r,i)=>i===index?{...r,...patch}:r)});
 const save=()=>act(async()=>{
  const examples=s.rows.map((r,i)=>{try{const args:unknown=JSON.parse(r.args),output:unknown=JSON.parse(r.output);if(!Array.isArray(args))throw '';return {...r,args,output};}catch{throw `第 ${i+1} 组用例需要合法JSON：参数填数组，输出填对应类型的值。`;}});
  const result=await invoke<ProgrammingJob>('save_harness_programming_draft',{workspaceId,input:{jobId:s.jobId,expectedRevision:s.revision,origin:s.origin,spec:{...s.spec,examples}}});
  if(!active.current)return;cached.delete(draftKey);setS(from(result,result.revisions[0]));await read(result.id);notify('编程草案已保存，等待容器验证');
 });
 const latest=data?.selected?.id===s.jobId?data.selected.revisions[0].revision:0,historical=s.revision>0&&s.revision!==latest;
 const savedRevision=data?.selected?.id===s.jobId?data.selected.revisions.find(r=>r.revision===s.revision):undefined;
 const file=project?.files.find(f=>f.path===s.origin.path),lines=file?sourceLines(file.content):[],locked=disabled||busy||historical||validationRunning;
 const projectSpec=s.spec.project;
 const selectTarget=(path:string)=>change({origin:{...s.origin,path,start:1,end:1},spec:{...s.spec,project:projectSpec&&{...projectSpec,files:[...new Set([path,...projectSpec.files].filter(Boolean))]}}});
 const editProjectFile=(path:string,action:'include'|'remove'|'allow'|'lock')=>{
  if(!projectSpec)return;
  const next={...projectSpec,reference:{...projectSpec.reference},negative:{...projectSpec.negative}};
  if(action==='include')next.files=[...next.files,path];
  if(action==='remove')next.files=next.files.filter(p=>p!==path);
  if(action==='remove'||action==='lock'){delete next.reference[path];delete next.negative[path];}
  if(action==='allow'){const original=project?.files.find(f=>f.path===path)?.content??'';next.reference[path]=original;next.negative[path]=original;}
  spec({project:next});
 };
 return <details className="programming-drafts task-setup"><summary>编程任务草案</summary>
  <div className="fit-meta-line">{!savedRevision&&<FitStatus tone="attention">尚未保存</FitStatus>}<span>分析版本 {s.origin.assessmentVersion}</span>{s.revision>0&&<span>修订 {s.revision}</span>}</div>
  {error&&<div className="cloud-error" role="alert" tabIndex={-1} ref={errorRef}><p>{error}</p><button className="button secondary" disabled={disabled||busy} onClick={()=>void act(()=>read(s.jobId,true))}>重新读取（保留草稿）</button></div>}
  <div className="task-history"><label>编程草案<SelectMenu aria-label="编程草案历史" value={s.jobId??''} disabled={disabled||busy} onChange={e=>{const id=e.target.value;if(id)confirm(()=>void act(()=>read(id)));}}><option value="">新草案</option>{data?.history.map(h=><option key={h.id} value={h.id}>{new Date(h.createdAt).toLocaleString()} · {h.id.slice(0,8)}</option>)}</SelectMenu></label><button className="button secondary" disabled={disabled||busy} onClick={()=>confirm(()=>{cached.delete(draftKey);setS(blank(assessmentVersion));setProject(null);})}>另建草案</button>
  {s.jobId&&data?.selected?.id===s.jobId&&<label>修订记录<SelectMenu aria-label="编程草案修订记录" value={s.revision} disabled={disabled||busy} onChange={e=>{const r=data.selected!.revisions.find(r=>r.revision===Number(e.target.value));if(r)confirm(()=>{cached.delete(draftKey);setS(from(data.selected!,r));});}}>{data.selected.revisions.map(r=><option key={r.revision} value={r.revision}>修订 {r.revision} · {new Date(r.createdAt).toLocaleString()}</option>)}</SelectMenu></label>}</div>
  {historical&&<p>正在查看历史修订。选择最新修订后可继续编辑。</p>}
  <fieldset disabled={locked} className="programming-form">
   <legend>固定项目与评分入口</legend>
   <label className="profile-field">任务类型<SelectMenu aria-label="编程任务类型" disabled={!!s.jobId} value={projectSpec?.kind??'function'} onChange={e=>spec({project:e.target.value==='function'?undefined:{kind:e.target.value as ProjectProgrammingSpec['kind'],files:s.origin.path?[s.origin.path]:[],reference:{},negative:{}}})}><option value="function">单函数</option>{Object.entries(projectKinds).map(([value,label])=><option key={value} value={value}>{label}</option>)}</SelectMenu></label>
   {!s.jobId&&<><div className="cloud-actions"><button type="button" className="button secondary" onClick={()=>void act(()=>importProject('folder'))}>选择项目文件夹</button><button type="button" className="button secondary" onClick={()=>void act(()=>importProject('zip'))}>选择项目ZIP</button></div><label className="profile-field">项目快照<SelectMenu aria-label="编程任务项目快照" value={s.origin.repositoryId} onChange={e=>{if(e.target.value)void act(()=>selectProject(e.target.value));}}><option value="">选择已有项目</option>{projects.filter(p=>p.profile?.kind!=='harness_document').map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</SelectMenu></label></>}
   <label className="profile-field">{projectSpec?'评分入口文件':'Python 文件'}<SelectMenu aria-label="编程任务目标文件" disabled={!!s.jobId} value={s.origin.path} onChange={e=>selectTarget(e.target.value)}><option value="">选择 .py 文件</option>{project?.files.filter(f=>f.path.endsWith('.py')).map(f=><option key={f.path} value={f.path}>{f.path}</option>)}</SelectMenu></label>
   {projectSpec&&<details className="personal-details" open><summary>副本文件与修改权限</summary><p>选择 2 至 8 个 Python 或 SQL 文件。标为只读的文件可用于评分或提供上下文。</p>
    <label className="profile-field">加入快照文件<SelectMenu aria-label="加入快照文件" disabled={projectSpec.files.length>=8} value="" onChange={e=>{if(e.target.value)editProjectFile(e.target.value,'include');}}><option value="">选择文件</option>{project?.files.filter(f=>/\.(py|sql)$/.test(f.path)&&!projectSpec.files.includes(f.path)).map(f=><option key={f.path} value={f.path}>{f.path}</option>)}</SelectMenu></label>
    <ul className="task-requirement-list">{projectSpec.files.map(path=><li key={path}><label><input type="checkbox" checked={Object.prototype.hasOwnProperty.call(projectSpec.reference,path)} disabled={!path.endsWith('.py')} onChange={e=>editProjectFile(path,e.target.checked?'allow':'lock')}/>允许修改 {path}</label>{path!==s.origin.path&&<button type="button" className="button text-button" onClick={()=>editProjectFile(path,'remove')}>移除</button>}</li>)}</ul>
   </details>}
   <div className="task-range"><label>开始行<input aria-label="原函数开始行" disabled={!!s.jobId} type="number" min={1} max={lines.length} value={s.origin.start} onChange={e=>change({origin:{...s.origin,start:Number(e.target.value)}})}/></label><label>结束行<input aria-label="原函数结束行" disabled={!!s.jobId} type="number" min={s.origin.start} max={Math.min(lines.length,s.origin.start+119)} value={s.origin.end} onChange={e=>change({origin:{...s.origin,end:Number(e.target.value)}})}/></label></div>
   <details><summary>查看固定原代码</summary><pre className="personal-code">{lines.slice(Math.max(0,s.origin.start-1),s.origin.end).map((l,i)=>`${s.origin.start+i}  ${l}`).join('\n')}</pre></details>
   <label className="profile-field">任务依据<SelectMenu aria-label="编程任务依据类型" value={s.spec.basis} onChange={e=>spec({basis:e.target.value as ProgrammingSpec['basis']})}><option value="confirmed_requirement">我确认的功能需求</option><option value="reported_bug">我报告的故障</option></SelectMenu></label>
   <label className="profile-field">需求或故障依据<textarea aria-label="编程任务依据" rows={2} maxLength={2000} value={s.spec.rationale} onChange={e=>spec({rationale:e.target.value})}/></label>
   <label className="profile-field">预期行为<textarea aria-label="编程任务预期行为" rows={3} maxLength={16000} value={s.spec.prompt} onChange={e=>spec({prompt:e.target.value})}/></label>
   <label className="profile-field">函数名<input aria-label="编程任务函数名" maxLength={64} value={s.spec.function} onChange={e=>spec({function:e.target.value})}/></label>
   <div className="programming-parameters">{s.spec.parameters.map((p,i)=><div className="cloud-actions" key={i}><label>参数 {i+1}<input aria-label={`参数 ${i+1} 名称`} value={p.name} maxLength={64} onChange={e=>spec({parameters:s.spec.parameters.map((p,n)=>n===i?{...p,name:e.target.value}:p)})}/></label><label>类型<SelectMenu aria-label={`参数 ${i+1} 类型`} value={p.kind} onChange={e=>spec({parameters:s.spec.parameters.map((p,n)=>n===i?{...p,kind:e.target.value}:p)})}>{Object.entries(types).map(([k,v])=><option key={k} value={k}>{v}</option>)}</SelectMenu></label><button className="button text-button" disabled={s.spec.parameters.length===1} onClick={()=>spec({parameters:s.spec.parameters.filter((_,n)=>n!==i)})}>移除参数 {i+1}</button></div>)}<button className="button secondary" disabled={s.spec.parameters.length>=5} onClick={()=>spec({parameters:[...s.spec.parameters,{name:'',kind:'integer'}]})}>添加参数</button></div>
   <label className="profile-field">返回类型<SelectMenu aria-label="编程任务返回类型" value={s.spec.returnType} onChange={e=>spec({returnType:e.target.value})}>{Object.entries(types).map(([k,v])=><option key={k} value={k}>{v}</option>)}</SelectMenu></label>
   <details className="personal-details"><summary>用例格式与限制</summary><p>7–32组不同输入，至少各有一组目标行为、边界情况和非目标回归。参数使用JSON数组（如 [1, 2]）；输出使用JSON值（如 3、"文本"、true 或 [1, 2]）。整数与布尔值严格区分，不允许修改输入。列表最多128项，文本最多4096字符。</p><p>原代码范围应包含独立纯函数及必要标准库导入。这里只保存文本，Python语法、真实故障和用例充分性仍待容器验证。</p></details>
   <h4>输入与预期输出（{s.rows.length}/32）</h4>
   <div className="programming-examples">{s.rows.map((r,i)=><fieldset key={i} className="programming-example"><legend>用例 {i+1}</legend><label>用途<SelectMenu aria-label={`用例 ${i+1} 用途`} value={r.role} onChange={e=>row(i,{role:e.target.value as Row['role']})}><option value="target">目标行为</option><option value="boundary">边界情况</option><option value="regression">非目标回归</option></SelectMenu></label><label>检查内容<input aria-label={`用例 ${i+1} 说明`} value={r.label} maxLength={100} onChange={e=>row(i,{label:e.target.value})}/></label><label>参数（JSON数组）<textarea aria-label={`用例 ${i+1} 参数`} rows={2} value={r.args} onChange={e=>row(i,{args:e.target.value})}/></label><label>预期输出（JSON值）<textarea aria-label={`用例 ${i+1} 输出`} rows={2} value={r.output} onChange={e=>row(i,{output:e.target.value})}/></label><button className="button text-button" onClick={()=>change({rows:s.rows.filter((_,n)=>n!==i)})}>移除用例 {i+1}</button></fieldset>)}</div>
   <button className="button secondary" disabled={s.rows.length>=32} onClick={()=>change({rows:[...s.rows,{label:'',role:s.rows.length===1?'boundary':s.rows.length===2?'regression':'target',args:'',output:''}]})}>添加用例</button>
   <details className="personal-details" open><summary>参考实现与错误对照</summary>{projectSpec?Object.keys(projectSpec.reference).map(path=><fieldset key={path}><legend>{path}</legend>{(['reference','negative'] as const).map(variant=><label key={variant} className="profile-field">{variant==='reference'?'参考文件内容':'错误对照文件内容'}<textarea className="programming-code" rows={5} value={projectSpec[variant][path]} onChange={e=>spec({project:{...projectSpec,[variant]:{...projectSpec[variant],[path]:e.target.value}}})}/></label>)}</fieldset>):<><label className="profile-field">参考实现<textarea aria-label="参考实现代码" className="programming-code" rows={6} value={s.spec.referenceCode} onChange={e=>spec({referenceCode:e.target.value})}/></label><label className="profile-field">错误实现<textarea aria-label="错误对照代码" className="programming-code" rows={5} value={s.spec.wrongCode} onChange={e=>spec({wrongCode:e.target.value})}/></label></>}</details>
   <button className="button primary" disabled={!s.dirty||!s.origin.path} onClick={()=>void save()}>{busy?'正在保存…':'保存编程草案'}</button>
  </fieldset>
  {savedRevision&&<details className="personal-details"><summary>保存依据与待验证项目</summary><FitStatus tone="success">保存时结构检查通过</FitStatus>{s.dirty&&<p>以上状态仅对应已保存修订；当前修改尚未检查。</p>}<button className="button text-button" disabled={busy||disabled} onClick={()=>onSource(savedRevision.body.source)}>查看保存时的来源</button><p>这是保存草案时的静态记录。实际容器结果在下方“验证与发布”中单独保存。</p><pre>修订指纹：{savedRevision.contentHash}</pre></details>}
  {savedRevision&&<ProgrammingValidation key={`${s.jobId}:${s.revision}`} workspaceId={workspaceId} draftId={s.jobId!} revision={savedRevision} dirty={s.dirty} disabled={disabled||busy||historical} onRunning={setValidationRunning} onSource={onSource}/>}
  <AppDialog ref={confirmDialog} className="panel-dialog confirm-dialog" aria-labelledby="programming-discard-title"><header className="panel-header"><h2 id="programming-discard-title">放弃未保存的修改</h2><button className="icon-button" aria-label="关闭放弃编程草案确认" onClick={()=>confirmDialog.current?.close()}><X size={18} aria-hidden="true"/></button></header><div className="panel-body"><p>{confirmText}</p></div><div className="dialog-actions"><button className="button secondary" onClick={()=>confirmDialog.current?.close()}>继续编辑</button><button className="button primary" onClick={()=>{confirmDialog.current?.close();pending.current();}}>放弃修改</button></div></AppDialog>
 </details>;
}
