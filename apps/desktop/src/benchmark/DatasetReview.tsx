import { draftStore } from "../draft-store";
import { useEffect,useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorText,stateName,ruleName,type Job,type Dataset,type Candidate,type History } from "./personalized";
const reviewDrafts=draftStore<{caseId:string;revision:number;text:string}>("dataset-review", value => typeof (value as {text?:unknown} | null)?.text === "string");
export function DatasetReview({workspaceId,job,onJob,history,onOpen,onPublished}:{workspaceId:string;job:Job|null;onJob:(j:Job)=>void;history:History[];onOpen:(id:string)=>void;onPublished:(d:Dataset)=>void}){
 const [jsonError,setJsonError]=useState("");
 const [selection,setSelection]=useState<Record<string,number>>({}),[reviewer,setReviewer]=useState(""),[editing,setEditing]=useState<string|null>(null),[draft,setDraft]=useState(""),[error,setError]=useState(""),[busy,setBusy]=useState(false);
 const draftKey=`${workspaceId}:${job?.id??"none"}`;
 useEffect(()=>{const cached=reviewDrafts.get(draftKey);if(cached && job?.candidates?.some(item => item.id===cached.caseId && item.revision===cached.revision)){setEditing(cached.caseId);setDraft(cached.text);}else{setEditing(null);setDraft("");}},[draftKey,job?.candidates]);
 const act=async(fn:()=>Promise<void>)=>{setBusy(true);setError("");try{await fn();}catch(e){setError(errorText(e));}finally{setBusy(false);}};
 return <section className="personal-step"><h2>审核与发布</h2>{error&&<p role="alert" className="cloud-error">{error}</p>}
 {history.length>0&&<details className="personal-details"><summary>生成历史</summary><ul className="personal-list">{history.map(j=><li key={j.id}><span>{new Date(j.createdAt).toLocaleString()}</span><span>{stateName[j.status]??j.status}</span><button className="button text-button" onClick={()=>{setSelection({});setEditing(null);onOpen(j.id);}}>打开</button></li>)}</ul></details>}
 {!job?<p>先生成候选题，或从历史中打开。</p>:<><p role="status">{stateName[job.status]??job.status} · {job.candidates.length} 道候选题</p>{job.error&&<p role="alert" className="cloud-error">{job.error}</p>}
 <p>逐题核对题意、标准答案、来源语义与边界测试。自动检查仅验证结构和规则一致性，不证明模型给出的答案正确。</p>
 {job.candidates.map(item=><article className="personal-candidate" key={item.id}><header><h3>{ruleName[String((item.candidate.case.scoring as {id?:string})?.id)]??"暂不支持的类别"} · {{easy:"基础",medium:"中等",hard:"困难"}[String(item.candidate.case.difficulty)]??"待检查"}</h3><span>{!item.validation.supported?"暂不支持自动评分":item.validation.valid?"结构检查通过 · 等待人工审核":"需要修订"}</span></header><p className="personal-question">{String((item.candidate.case.input as {prompt?:string})?.prompt??"题目结构异常，请查看原始响应。")}</p><p>{item.candidate.rationale}</p>
 {item.validation.errors.map((e,i)=><p className="cloud-error" key={i}>{e}</p>)}<details className="personal-details"><summary>标准答案、评分规则与来源</summary><pre>{JSON.stringify(item.candidate,null,2)}</pre>{item.validation.warnings.map((w,i)=><p key={i}>{w}</p>)}</details>
 {editing===item.id?<><label className="profile-field">修改候选 JSON<textarea aria-label="修改候选 JSON" aria-invalid={!!jsonError || undefined} aria-describedby={jsonError ? "candidate-json-error" : undefined} rows={18} value={draft} onChange={e=>{setJsonError("");setDraft(e.target.value);reviewDrafts.set(draftKey,{caseId:item.id,revision:item.revision,text:e.target.value});}}/></label>{jsonError&&<p id="candidate-json-error" className="form-error" role="alert">{jsonError}</p>}<div className="cloud-actions"><button className="button primary" disabled={busy} onClick={()=>void act(async()=>{let candidate: Candidate;
      try {
        const row: unknown = JSON.parse(draft);
        if (!row || typeof row !== "object" || Array.isArray(row)) throw new Error();
        candidate = row as Candidate;
        if (!candidate.case || typeof candidate.case !== "object" || Array.isArray(candidate.case) ||
          typeof candidate.rationale !== "string" || !candidate.rationale.trim() || !Array.isArray(candidate.sources)) throw new Error();
      } catch {
        setJsonError("请输入包含 case 对象、rationale 文字和 sources 数组的有效 JSON。");
        return;
      }onJob(await invoke<Job>("revise_benchmark_candidate",{workspaceId,jobId:job.id,caseId:item.id,expectedRevision:item.revision,candidate}));setEditing(null);setJsonError("");reviewDrafts.delete(draftKey);setSelection(s=>{const next={...s};delete next[item.id];return next;});})}>保存新修订</button><button className="button secondary" onClick={()=>{setEditing(null);setJsonError("");reviewDrafts.delete(draftKey);}}>取消编辑</button></div></>:<button className="button text-button" disabled={busy||job.status!=="review"||editing!==null} onClick={()=>{setEditing(item.id);setDraft(JSON.stringify(item.candidate,null,2));reviewDrafts.set(draftKey,{caseId:item.id,revision:item.revision,text:JSON.stringify(item.candidate,null,2)});}}>修订题目</button>}
 <label className="personal-consent"><input type="checkbox" disabled={busy||!item.validation.valid||!item.validation.supported||editing===item.id} checked={selection[item.id]===item.revision} onChange={e=>setSelection(s=>{const next={...s};if(e.target.checked)next[item.id]=item.revision;else delete next[item.id];return next;})}/>我已核对本题，批准第 {item.revision} 次修订</label>
 </article>)}
 <details className="personal-details"><summary>原始响应与实际用量</summary><pre>{job.response?.text??"暂无响应"}</pre><pre>{JSON.stringify(job.usage,null,2)}</pre></details>
 <div className="personal-publish"><label>审核人<input aria-label="审核人" maxLength={100} value={reviewer} onChange={e=>setReviewer(e.target.value)}/></label><button className="button primary" disabled={busy||editing!==null||!reviewer.trim()||!Object.keys(selection).length||job.status!=="review"} onClick={()=>void act(async()=>{const dataset=await invoke<Dataset>("publish_benchmark_dataset",{workspaceId,jobId:job.id,reviewer,selection:Object.entries(selection).map(([id,revision])=>({id,revision,approved:true}))});setSelection({});onPublished(dataset);})}>发布已批准的 {Object.keys(selection).length} 题</button></div>
 </>}
 </section>;
}
