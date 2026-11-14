import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { AppDialog,openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { errorText } from '../personalized';
import { FitStatus } from './FitStatus';
import { taskNames,type PublishedTasks } from './taskTypes';

type Model={name:string;digest:string|null};
type Run={id:string;status:string;error:string|null;plan:{caseId:string;model:{model:string};evidence:string};result:{score:{state:string;details:unknown};actual:unknown;modelRequested:boolean}|null};
const labels:Record<string,string>={running:'运行中',interrupted:'已中断',cancelled:'已停止',failed:'保存失败',Passed:'通过',Failed:'未通过',Unsupported:'不支持','Needs Review':'待复核','Execution Error':'执行出错','Scoring Error':'评分出错'};

export function ReadonlyRuns({workspaceId,published}:{workspaceId:string;published:PublishedTasks}) {
  const tasks=published.tasks.filter(item=>['call_chain','tool_call','permission_review'].includes(item.task.kind));
  const datasetId=`harness-${published.provenance.generationJobId}`;
  const [taskId,setTaskId]=useState(tasks[0]?.id??''),[models,setModels]=useState<Model[]>([]),[model,setModel]=useState('');
  const [runs,setRuns]=useState<Run[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState('');
  const dialog=useRef<HTMLDialogElement>(null);
  const running=runs.some(run=>run.status==='running');
  const picked=models.find(item=>item.name===model&&item.digest);
  const read=()=>invoke<Run[]>('get_harness_readonly_runs',{workspaceId,datasetId,version:published.version});
  useEffect(()=>{
    if(!running)return;
    let live=true;
    const interval=setInterval(()=>void read().then(rows=>{if(live)setRuns(rows);}).catch(reason=>{if(live)setError(errorText(reason));}),750);
    return()=>{live=false;clearInterval(interval);};
  },[running,workspaceId,datasetId,published.version]);
  async function act(action:()=>Promise<void>) {
    setBusy(true);setError('');
    try {await action();}catch(reason){setError(errorText(reason));}finally{setBusy(false);}
  }
  const load=()=>act(async()=>{
    const [catalog,rows]=await Promise.all([invoke<{models:Model[]}>('list_benchmark_models',{workspaceId}),read()]);
    const available=catalog.models.filter(item=>item.digest);
    setModels(available);setModel(old=>available.some(item=>item.name===old)?old:available[0]?.name??'');setRuns(rows);
  });
  if(tasks.length===0)return null;
  return <details className="task-setup" onToggle={event=>{if(event.currentTarget.open&&!models.length&&!busy)void load();}}>
    <summary>验证已发布的只读任务</summary>
    <div className="cloud-actions">
      <label>任务<SelectMenu aria-label="只读评测任务" disabled={busy||running} value={taskId} onChange={event=>setTaskId(event.target.value)}>{tasks.map((item,index)=><option key={item.id} value={item.id}>{index+1}. {taskNames[item.task.kind]}</option>)}</SelectMenu></label>
      <label>本地模型<SelectMenu aria-label="只读评测模型" disabled={busy||running} value={model} onChange={event=>setModel(event.target.value)}><option value="">选择模型</option>{models.map(item=><option key={item.name} value={item.name}>{item.name}</option>)}</SelectMenu></label>
      <button className="button secondary" disabled={busy} onClick={()=>void load()}>刷新模型与记录</button>
      <button className="button primary" disabled={!picked||!taskId||busy||running} onClick={()=>openDialog(dialog.current)}>运行一题…</button>
    </div>
    {runs.map(run=><details className="personal-details" key={run.id}><summary>{run.plan.model.model} <FitStatus tone={run.result?.score.state==='Passed'?'success':'neutral'}>{labels[run.result?.score.state??run.status]??run.status}</FitStatus></summary>{run.error&&<p>{run.error}</p>}{run.result&&<pre>{JSON.stringify(run.result,null,2)}</pre>}</details>)}
    {error&&<p className="cloud-error" role="alert">{error}</p>}
    <AppDialog ref={dialog} className="panel-dialog form-dialog" aria-labelledby="readonly-run-title">
      <header className="panel-header"><h2 id="readonly-run-title">确认只读任务模型调用</h2><button className="icon-button" aria-label="关闭只读任务确认" onClick={()=>dialog.current?.close()}><X size={18}/></button></header>
      <div className="panel-body"><p>{picked?.name} 将处理一题。最多 1 次请求，4096 上下文，输出最多 512 Token，限时 120 秒。原生工具只记录请求对象。</p><code>{picked?.digest}</code></div>
      <footer className="dialog-actions"><button className="button secondary" onClick={()=>dialog.current?.close()}>返回</button><button className="button primary" disabled={!picked?.digest||busy||running} onClick={()=>void act(async()=>{dialog.current?.close();if(!picked?.digest)return;await invoke('run_harness_readonly_task',{workspaceId,input:{datasetId,version:published.version,contentHash:published.content_hash,caseId:taskId,model:picked.name,digest:picked.digest,confirmed:true}});setRuns(await read());})}>确认调用</button></footer>
    </AppDialog>
  </details>;
}
