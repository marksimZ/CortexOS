import { useBatchOverview } from "./useBatchOverview";
import { navigate, useNavigation } from "../navigation";
import { useEffect,useState } from "react";
import { benchmark } from "../api/benchmark";
import { usePersonalizedHistory } from "./usePersonalizedHistory";
import { ProfileEditor } from "./ProfileEditor";
import { RepositoryImport } from "./RepositoryImport";
import { GenerationSetup } from "./GenerationSetup";
import { DatasetReview } from "./DatasetReview";
import { BatchSetup } from "./BatchSetup";
import { PersonalizedReport } from "./PersonalizedReport";
import { errorText,type Snapshot,type Location,type Job } from "./personalized";
export function PersonalizedWorkbench({workspaceId,onConnection}:{workspaceId:string|null;onConnection:()=>void}){if(!workspaceId)return <p>请先选择工作区。</p>;return <Workbench key={workspaceId} workspaceId={workspaceId} onConnection={onConnection}/>;}
function Workbench({workspaceId,onConnection}:{workspaceId:string;onConnection:()=>void}){
 const [step,setStep]=useState(0),[snapshot,setSnapshot]=useState<Snapshot|null>(null),[snippets,setSnippets]=useState<Location[]>([]),[job,setJob]=useState<Job|null>(null),[dataset,setDataset]=useState<string|null>(null),[error,setError]=useState("");
 const {profile,setProfile,loading:profileLoading,jobs,batches,refresh}=usePersonalizedHistory(workspaceId,setError);
 const requestedJob=useNavigation().params.get("job");
 const profileReady=!!profile&&profile.action!=="reset";
 const overview=useBatchOverview(workspaceId,"personalized"), batch=overview.batch;
 useEffect(()=>{if(overview.id)setStep(5);else setStep(current=>current===5?4:current);},[overview.id]);
 const openJob=async(id:string)=>{try{setJob(await benchmark("get_benchmark_generation",{workspaceId,jobId:id}));}catch(e){setError(errorText(e));}};
 useEffect(()=>{
  if(!requestedJob||!profileReady)return;
  const request=new AbortController();
  void benchmark("get_benchmark_generation",{workspaceId,jobId:requestedJob},request.signal)
   .then(result=>{setJob(result);setStep(3);})
   .catch(e=>{if(!request.signal.aborted)setError(errorText(e));});
  return()=>request.abort();
 },[workspaceId,requestedJob,profileReady]);
 const openBatch=overview.open;
 useEffect(()=>{if(job?.status!=="sending")return;let active=true,pending=false;const timer=window.setInterval(async()=>{if(pending)return;pending=true;try{const next=await benchmark("get_benchmark_generation",{workspaceId,jobId:job.id});if(active)setJob(next);}catch(e){if(active)setError(errorText(e));}finally{pending=false;}},1000);return()=>{active=false;window.clearInterval(timer);};},[workspaceId,job?.id,job?.status]);
 const move=(next:number)=>{setStep(next);setError("");if(next===5&&batch)overview.open(batch.id);else if(overview.id||requestedJob)navigate("benchmark",{workspace:workspaceId,view:"personalized"});void refresh();};
 return <div className={`personal-workbench${step===5?" is-report":""}`}><header className="personal-workbench-header"><nav className="profile-steps" aria-label="个性化评测步骤">{["使用偏好","项目资料","生成设置","审核题库","选择模型","结果报告"].map((label,i)=><button className="button secondary" key={label} aria-current={step===i?"step":undefined} disabled={i>0&&(!profile||profile.action==="reset")} onClick={()=>move(i)}>{i+1}. {label}</button>)}</nav>{step!==0&&<button className="button text-button" onClick={onConnection}>连接设置</button>}</header>
 {(error||overview.error)&&<div role="alert" className="cloud-error"><p>{error||overview.error}</p><button className="button secondary" onClick={()=>{void refresh();overview.refresh();}}>重新读取</button></div>}
 {step===0?<><ProfileEditor workspaceId={workspaceId} onConnection={onConnection}/>{profileLoading?<p role="status">正在读取…</p>:<button className="button primary" onClick={()=>void benchmark("get_benchmark_profile",{workspaceId}).then(p=>{setProfile(p);if(p&&p.action!=="reset")move(1);else setError("请先保存或暂时跳过使用偏好。");}).catch(e=>setError(errorText(e)))}>继续选择项目</button>}</>:step===1?<RepositoryImport workspaceId={workspaceId} snapshot={snapshot} onSnapshot={setSnapshot} snippets={snippets} onSnippets={setSnippets} onNext={()=>move(2)}/>:step===2?<GenerationSetup workspaceId={workspaceId} profileVersion={profile?.version??0} snapshot={snapshot} snippets={snippets} job={job} onJob={setJob} onReview={()=>move(3)}/>:step===3?<DatasetReview workspaceId={workspaceId} job={job} onJob={setJob} history={jobs} onOpen={id=>void openJob(id)} onPublished={d=>{setDataset(d.id);move(4);}}/>:step===4?<BatchSetup workspaceId={workspaceId} datasetId={dataset} onPlan={b=>{overview.open(b.id);setStep(5);}}/>:overview.loading?<p role="status">正在读取评测报告…</p>:<PersonalizedReport key={overview.id??"history"} workspaceId={workspaceId} batch={batch} onBatch={overview.update} history={batches} onOpen={id=>void openBatch(id)} onNew={()=>move(4)}/>}
 </div>;
}
