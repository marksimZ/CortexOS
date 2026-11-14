import { useEffect,useRef,useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { SelectMenu } from '../../components/SelectMenu';
import { FitStatus } from './FitStatus';
import { WorkspaceHandoff } from './WorkspaceHandoff';
import { CodexRuntime } from './CodexRuntime';
import { errorText } from '../personalized';

interface TaskVersion {id:string;version:number;contentHash:string;createdAt:string}
interface CheckRecord {id:string;contentHash:string;body:{datasetId:string;datasetVersion:number;datasetHash:string;finishedAt:string;canRun:false;taskExecution:'Not Run';cli:{state:string;version?:string;reason?:string};docker:{state:string;reason?:string};checks:{id:string;name:string;state:'passed'|'blocked'|'not_checked';detail:string}[]}}
interface Payload {datasets:TaskVersion[];history:CheckRecord[];selected:CheckRecord|null;canRun:false}
const label={passed:'已核对',blocked:'未就绪',not_checked:'未检查'};
// Keep the stored observation intact; use plain wording in the reading view.
const plain=(text:string)=>text.replace(/宿主/g,'本机').replace(/Agent/g,'编程工具').replace(/CLI/g,'程序').replace(/工作副本/g,'项目副本');
const reason:Record<string,string>={not_in_path:'没有在本机PATH中找到原生程序。',wrapper_not_executed:'找到了启动脚本，当前检查不执行脚本，版本仍未知。',native_windows_probe_only:'当前只支持Windows原生程序的版本检查。',cli_version_timeout:'版本检查超过5秒，已结束探针。',cli_version_unrecognized:'程序输出无法确认为Codex版本，未保存原始输出。',cli_too_large:'程序超过512 MiB，未执行版本探针。',cli_start_failed:'无法启动程序的版本检查。',cli_version_failed:'程序未能完成版本检查。',cli_changed_during_probe:'检查期间程序内容发生变化，请重试。',probe_cleanup_failed:'版本检查临时目录清理失败。',docker_unavailable:'本地Docker不可用。',docker_environment_error:'无法连接本地Docker引擎，请确认Docker正在运行。',python_image_unavailable:'发布时固定的Python镜像不可用。'};
export function ExecutionPreflight({workspaceId,assessmentVersion,disabled}:{workspaceId:string;assessmentVersion:number;disabled:boolean}){
 const [data,setData]=useState<Payload|null>(null),[selection,setSelection]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const alive=useRef(true),epoch=useRef(0);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;epoch.current++;};},[]);
 const read=async(checkId:string|null=null)=>{
  const ticket=++epoch.current;setBusy(true);setError('');
  try{const next=await invoke<Payload>('get_harness_execution_preflight',{workspaceId,assessmentVersion,checkId});if(!alive.current||ticket!==epoch.current)return;const recordTask=next.selected?`${next.selected.body.datasetId}:${next.selected.body.datasetVersion}`:'';const chosen=checkId?recordTask:next.datasets.some(d=>`${d.id}:${d.version}`===selection)?selection:next.datasets[0]?`${next.datasets[0].id}:${next.datasets[0].version}`:'';setSelection(chosen);setData({...next,selected:recordTask===chosen?next.selected:null});}
  catch(e){if(alive.current&&ticket===epoch.current)setError(errorText(e));}finally{if(alive.current&&ticket===epoch.current)setBusy(false);}
 };
 const task=data?.datasets.find(d=>`${d.id}:${d.version}`===selection);
 const check=async()=>{
  if(!task)return;const ticket=++epoch.current;setBusy(true);setError('');
  try{await invoke('check_harness_execution_preflight',{workspaceId,input:{assessmentVersion,datasetId:task.id,version:task.version,datasetHash:task.contentHash}});if(alive.current&&ticket===epoch.current)await read();}
  catch(e){if(alive.current&&ticket===epoch.current)setError(errorText(e));}finally{if(alive.current&&ticket===epoch.current)setBusy(false);}
 };
 const selected=data?.selected;
 return <details className="task-setup execution-preflight" onToggle={e=>{if(e.currentTarget.open&&!data&&!busy)void read();}}>
  <summary>Codex 执行前检查</summary>
  <section aria-label="Codex执行前检查" aria-busy={busy}>
   <div className="cloud-actions"><label>已发布编程任务<SelectMenu aria-label="执行检查任务版本" value={selection} disabled={busy||disabled} onChange={e=>{setSelection(e.target.value);setData(old=>old?{...old,selected:null}:old);}}><option value="">选择已发布任务</option>{data?.datasets.map(d=><option key={`${d.id}:${d.version}`} value={`${d.id}:${d.version}`}>任务 {d.id.replace('harness-python-','').slice(0,8)} / 版本 {d.version}</option>)}</SelectMenu></label><button className="button secondary" disabled={busy||disabled} onClick={()=>void read()}>更新任务列表</button><button className="button primary" disabled={!task||busy||disabled} onClick={()=>void check()}>检查执行条件</button></div>
   {error&&<div className="cloud-error" role="alert"><p>{error}</p><button className="button secondary" disabled={busy||disabled} onClick={()=>void read()}>重新读取检查记录</button></div>}
   {busy&&<p role="status">正在检查或读取本机条件，没有运行编程任务…</p>}
   {data?.datasets.length===0&&<p>当前分析版本还没有已发布的编程任务。先完成草案验证与人工发布，再更新列表。</p>}
   {data&&data.history.length>0&&<label className="profile-field">检查历史<SelectMenu aria-label="执行检查历史" value={selected?.id??''} disabled={busy||disabled} onChange={e=>void read(e.target.value)}>{data.history.map(r=><option key={r.id} value={r.id}>{new Date(r.body.finishedAt).toLocaleString()} / 任务 {r.body.datasetId.replace('harness-python-','').slice(0,8)} / 版本 {r.body.datasetVersion}</option>)}</SelectMenu></label>}
   {selected?<>
    <div className="fit-meta-line"><strong>任务 {selected.body.datasetId.replace('harness-python-','').slice(0,8)} / 版本 {selected.body.datasetVersion}</strong><FitStatus tone="neutral">原始检查快照</FitStatus><FitStatus tone="neutral">任务未运行</FitStatus></div>
    {!busy&&<p role="status">这是此前保存的检查；当前隔离检查与执行结果见下方。</p>}
    <ul className="preflight-checks">{selected.body.checks.map(c=><li key={c.id}><div><strong>{plain(c.name)}</strong><FitStatus tone={c.state==='passed'?'success':c.state==='blocked'?'attention':'neutral'}>{label[c.state]}</FitStatus></div><p>{plain(c.detail)}</p></li>)}</ul>
    <details className="personal-details"><summary>版本、环境与检查依据</summary><dl className="fit-catalog-details"><dt>Codex版本</dt><dd>{selected.body.cli.version??'未确认'}</dd><dt>程序检查原因</dt><dd>{selected.body.cli.reason?(reason[selected.body.cli.reason]??selected.body.cli.reason):'宿主版本已读取；没有检测容器内CLI或登录状态。'}</dd><dt>Docker检查原因</dt><dd>{selected.body.docker.reason?(reason[selected.body.docker.reason]??selected.body.docker.reason):'发布时固定的镜像及本地引擎可读取。'}</dd><dt>检查时间</dt><dd>{new Date(selected.body.finishedAt).toLocaleString()}</dd><dt>记录指纹</dt><dd><code>{selected.contentHash}</code></dd></dl><p>这是检查时的环境快照。环境、任务或模型配置变化后需要重新检查；实际任务使用下方独立确认与调用预算。</p><pre>{JSON.stringify(selected.body,null,2)}</pre></details>
    <CodexRuntime key={`runtime-${selected.id}`} workspaceId={workspaceId} assessmentVersion={assessmentVersion} checkId={selected.id} checkHash={selected.contentHash} disabled={busy||disabled}/>
    <WorkspaceHandoff key={selected.id} workspaceId={workspaceId} assessmentVersion={assessmentVersion} checkId={selected.id} checkHash={selected.contentHash} disabled={busy||disabled}/>
   </>:data&&<p>选择一个已发布版本后检查。检查只读取程序版本和Docker信息；实际任务需另行确认。</p>}
  </section>
 </details>;
}
