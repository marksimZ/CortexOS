import { useEffect,useRef,useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download,RotateCcw,X } from "lucide-react";
import { AppDialog,openDialog } from "../components/AppDialog";
import { SelectMenu } from "../components/SelectMenu";
import { notify } from "../components/Toast";
import { errorText,stateName,type Batch } from "./personalized";
import type { ReportScope } from "./ResultDrawer";
interface Plan {sessionId:string;planHash:string;engineVersion:string;implementationHash:string;eligible:number;skippedAttempts:number;plannedItems:number;pythonCases:number}
interface Session {session_id:string;created_at:string;engine_version:string;result_count:number;passed:number;failed:number}
interface Page<T> {total:number;page:number;pageSize:number;sessions?:T[];results?:T[]}
interface Grade {id:string;evaluator_version:string;details_json:{state:string;regrade:{ordinal:number;modelIndex:number;caseId:string;repetition:number}};[key:string]:unknown}
const initial={total:0,page:0,pageSize:20};
export function ReportActions({workspaceId,batch,scope}:{workspaceId:string;batch:Batch;scope:ReportScope}){
 const exportDialog=useRef<HTMLDialogElement>(null),regradeDialog=useRef<HTMLDialogElement>(null),active=useRef(true);
 const pendingSession=useRef<string|null>(null);
 const [format,setFormat]=useState("json"),[saving,setSaving]=useState(false),[saveError,setSaveError]=useState("");
 const [plan,setPlan]=useState<Plan|null>(null),[loading,setLoading]=useState(false),[running,setRunning]=useState(false),[stopping,setStopping]=useState(false),[approved,setApproved]=useState(false),[error,setError]=useState("");
 const [history,setHistory]=useState<Page<Session>>(initial),[selected,setSelected]=useState<string|null>(null),[grades,setGrades]=useState<Page<Grade>>(initial);
 const args={workspaceId,batchId:batch.id,scope},blocked=["running","stopping"].includes(batch.status);
 useEffect(()=>{active.current=true;return()=>{active.current=false;if(pendingSession.current)void invoke("cancel_benchmark_regrade",{workspaceId,sessionId:pendingSession.current}).catch(()=>{});};},[]);
 const readHistory=async(page=0)=>{const data=await invoke<Page<Session>>("get_benchmark_regrades",{...args,sessionId:null,page});if(active.current)setHistory(data);};
 const open=async()=>{
  setError("");setPlan(null);setSelected(null);setGrades(initial);setApproved(false);setLoading(true);openDialog(regradeDialog.current);
  try{const [p,h]=await Promise.all([invoke<Plan>("preview_benchmark_regrade",args),invoke<Page<Session>>("get_benchmark_regrades",{...args,sessionId:null,page:0})]);if(active.current){setPlan(p);setHistory(h);}}
  catch(e){if(active.current)setError(errorText(e));}finally{if(active.current)setLoading(false);}
 };
 const cancel=async()=>{if(!plan||stopping)return;setStopping(true);try{await invoke("cancel_benchmark_regrade",{workspaceId,sessionId:plan.sessionId});}catch(e){if(active.current){setError(errorText(e));setStopping(false);}}};
 const close=()=>{if(running)void cancel();else regradeDialog.current?.close();};
 const run=async()=>{
  if(!plan||!approved)return;pendingSession.current=plan.sessionId;setRunning(true);setError("");
  try{await invoke("run_benchmark_regrade",{...args,sessionId:plan.sessionId,approvedPlanHash:plan.planHash});if(active.current){notify("本轮评分已单独保存");setApproved(false);setSelected(null);await readHistory();setPlan(await invoke<Plan>("preview_benchmark_regrade",args));}}
  catch(e){if(active.current)setError(errorText(e));}finally{pendingSession.current=null;if(active.current){setRunning(false);setStopping(false);}}
 };
 const readGrades=async(session:string,page=0)=>{setLoading(true);setError("");try{const data=await invoke<Page<Grade>>("get_benchmark_regrades",{...args,sessionId:session,page});if(active.current){setSelected(session);setGrades(data);}}catch(e){if(active.current)setError(errorText(e));}finally{if(active.current)setLoading(false);}};
 const paging=(data:Page<unknown>,change:(page:number)=>void,label:string)=><nav className="benchmark-pagination" aria-label={label}><button className="button text-button" disabled={loading||running||data.page===0} onClick={()=>change(data.page-1)}>上一页</button><span>第 {data.page+1} / {Math.max(1,Math.ceil(data.total/20))} 页 · {data.total} 项</span><button className="button text-button" disabled={loading||running||(data.page+1)*20>=data.total} onClick={()=>change(data.page+1)}>下一页</button></nav>;
 return <>
  <button className="button secondary" disabled={blocked} title={blocked?"请先完成或停止评测":"导出完整报告"} onClick={()=>{setSaveError("");openDialog(exportDialog.current);}}><Download size={16} aria-hidden="true"/>导出报告</button>
  <button className="button secondary" disabled={blocked} title={blocked?"请先完成或停止评测":"按已保存回答重新评分，不调用模型"} onClick={()=>void open()}><RotateCcw size={16} aria-hidden="true"/>重新评分</button>
  <AppDialog ref={exportDialog} className="form-dialog panel-dialog" aria-labelledby="benchmark-export-title" onCancel={e=>{if(saving)e.preventDefault();}}>
   <form className="panel-form" onSubmit={e=>{e.preventDefault();setSaving(true);setSaveError("");void invoke<boolean>("export_benchmark_report",{...args,format}).then(saved=>{if(!active.current)return;if(saved){notify("报告已保存");exportDialog.current?.close();}else notify("已取消保存");}).catch(e=>{if(active.current)setSaveError(errorText(e));}).finally(()=>{if(active.current)setSaving(false);});}}>
    <header className="panel-header"><h2 id="benchmark-export-title">导出评测报告</h2><button className="icon-button" type="button" disabled={saving} aria-label="关闭导出报告" onClick={()=>exportDialog.current?.close()}><X size={18} aria-hidden="true"/></button></header>
    <div className="panel-body"><label className="profile-field">文件格式<SelectMenu aria-label="报告文件格式" value={format} disabled={saving} onChange={e=>setFormat(e.target.value)}><option value="json">JSON · 完整报告</option><option value="csv">CSV · 表格明细</option></SelectMenu></label><p>包含全计划、全部尝试及各轮评分；保留题目、回答、资料和请求快照，请选择合适的保存位置。</p><p>JSON 保留完整数据与校验标识。CSV 便于查看，已保护公式开头；不会覆盖已有文件。</p>{saveError&&<p role="alert" className="cloud-error">{saveError}</p>}{saving&&<p role="status">正在准备报告，请在系统窗口选择保存位置…</p>}</div>
    <footer className="panel-footer dialog-actions"><button className="button secondary" type="button" disabled={saving} onClick={()=>exportDialog.current?.close()}>取消</button><button className="button primary" disabled={saving}>选择保存位置</button></footer>
   </form>
  </AppDialog>
  <AppDialog ref={regradeDialog} className="form-dialog panel-dialog benchmark-regrade-dialog" aria-labelledby="benchmark-regrade-title" onCancel={e=>{if(running){e.preventDefault();void cancel();}}}>
   <header className="panel-header"><h2 id="benchmark-regrade-title">重新评分与历史</h2><button className="icon-button" aria-label={running?"取消本轮重新评分":"关闭重新评分"} disabled={stopping} onClick={close}><X size={18} aria-hidden="true"/></button></header>
   <div className="panel-body">{error&&<div role="alert" className="cloud-error"><p>{error}</p>{!running&&<button className="button secondary" onClick={()=>void open()}>重新预览</button>}</div>}{loading&&<p role="status">正在读取评分记录…</p>}{running&&<p role="status">{stopping?"正在取消，本轮未提交的记录会撤销…":"正在对已保存回答评分，不会请求模型…"}</p>}
    {plan&&<section><h3>本轮范围</h3><p>可重新评分 {plan.eligible} 次回答；跳过 {plan.skippedAttempts} 条不符合条件的尝试。全计划 {plan.plannedItems} 项。</p><p>原成绩保持不变，新结果作为独立记录保存。</p>{plan.pythonCases>0&&<p>{plan.pythonCases} 次编程回答将使用现有 Docker 隔离评分；环境不可用会单独记录。</p>}<details className="personal-details"><summary>评分器与输入标识</summary><pre>{JSON.stringify({engineVersion:plan.engineVersion,implementationHash:plan.implementationHash,inputHash:plan.planHash},null,2)}</pre></details><label className="personal-consent"><input type="checkbox" checked={approved} disabled={running||!plan.eligible} onChange={e=>setApproved(e.target.checked)}/>我确认按上述范围重新评分，保留原成绩。</label></section>}
    <section><h3>已保存的评分轮次</h3>{!loading&&history.total===0&&<p>尚未保存独立评分。</p>}<ul className="personal-list">{history.sessions?.map(s=><li key={s.session_id}><span>{new Date(s.created_at).toLocaleString()} · {s.result_count} 条记录 · 已评分 {s.passed+s.failed} 条 · 通过 {s.passed} 条 · {s.engine_version}</span><button className="button text-button" disabled={running||loading} onClick={()=>void readGrades(s.session_id)}>查看这轮评分</button></li>)}</ul>{history.total>20&&paging(history,p=>{setLoading(true);void readHistory(p).catch(e=>setError(errorText(e))).finally(()=>setLoading(false));},"评分轮次分页")}</section>
    {selected&&<section aria-label="独立评分明细"><h3>本轮评分明细</h3><ul className="benchmark-grade-list">{grades.results?.map(g=><li key={g.id}><strong>{g.details_json.regrade.caseId} · 模型 {g.details_json.regrade.modelIndex+1} · 第 {g.details_json.regrade.repetition} 次 · {stateName[g.details_json.state]??g.details_json.state}</strong><details><summary>评分依据与来源标识</summary><pre>{JSON.stringify(g,null,2)}</pre></details></li>)}</ul>{paging(grades,p=>void readGrades(selected,p),"本轮评分明细分页")}</section>}
   </div>
   <footer className="panel-footer dialog-actions"><button className="button secondary" disabled={stopping} onClick={close}>{running?"取消本轮重新评分":"关闭"}</button><button className="button primary" disabled={!approved||!plan?.eligible||loading||running} onClick={()=>void run()}>确认并重新评分</button></footer>
  </AppDialog>
 </>;
}
