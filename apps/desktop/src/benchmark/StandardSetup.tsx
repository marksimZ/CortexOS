import { draftStore } from "../draft-store";
import { capacityError, NumberField, validModelCapacity } from "../components/NumberField";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorText, ruleName, type Batch, type Dataset, type ModelRequest } from "./personalized";

interface CaseSummary { id:string; prompt:string; evaluator:string; difficulty:string; requirements:string[] }
interface Draft { chosen:string[]; cases:string[]; ctx:number|null; max:number|null; timeout:number|null; repetitions:number|null; image:string; release:boolean }
const drafts=draftStore<Draft>("standard-benchmark", value => !!value && typeof value === "object" && Array.isArray((value as Draft).chosen) && Array.isArray((value as Draft).cases));
const quick=["zh-006","math-001"];
export function StandardSetup({workspaceId,onPlan}:{workspaceId:string;onPlan:(batch:Batch)=>void}){
  const [draft,setDraft]=useState<Draft>(()=>drafts.get(workspaceId)??{chosen:[],cases:quick,ctx:4096,max:1024,timeout:180,repetitions:1,image:"",release:false});
  const [datasets,setDatasets]=useState<Dataset[]>([]),[models,setModels]=useState<{name:string;digest:string|null}[]>([]),[cases,setCases]=useState<CaseSummary[]>([]);
  const [capacityInvalid,setCapacityInvalid]=useState(false);
  const [search,setSearch]=useState(""),[busy,setBusy]=useState(false),[loading,setLoading]=useState(true),[error,setError]=useState("");
  const update=(patch:Partial<Draft>)=>setDraft(previous=>{const next={...previous,...patch};drafts.set(workspaceId,next);return next;});
  useEffect(()=>{
    let current=true;
    const load=async()=>{
      const results=await Promise.allSettled([
        invoke<Dataset[]>("list_benchmark_datasets",{workspaceId}),
        invoke<{models:{name:string;digest:string|null}[]}>("list_benchmark_models",{workspaceId}),
        invoke<CaseSummary[]>("list_standard_benchmark_cases",{workspaceId}),
      ]);
      if(!current)return;
      const [data,catalog,questions]=results;
      if(data.status==="fulfilled")setDatasets(data.value.filter(d=>d.id==="cortex-core-60"));
      if(catalog.status==="fulfilled")setModels(catalog.value.models);
      if(questions.status==="fulfilled")setCases(questions.value);
      const failures=results.filter((r):r is PromiseRejectedResult=>r.status==="rejected");
      if(failures.length)setError(failures.map(r=>errorText(r.reason)).join("；"));
      setLoading(false);
    };
    void load();return()=>{current=false;};
  },[workspaceId]);
  const dataset=datasets.find(d=>d.version===2);
  const plan=async(form: HTMLFormElement)=>{
    const {ctx,max,timeout,repetitions}=draft;
    if (ctx===null || max===null || timeout===null || repetitions===null) return;
    if (!validModelCapacity(ctx,max)) {
      setCapacityInvalid(true);
      const advanced=form.querySelector<HTMLDetailsElement>("details:has(.standard-context)");
      if(advanced){advanced.open=true;advanced.querySelector<HTMLInputElement>(".standard-context input")?.focus();}
      return;
    }
    setBusy(true);setError("");
    try{
      if(!dataset)throw "固定题库尚未就绪，请重新读取。";
      const requests:ModelRequest[]=draft.chosen.map(name=>({model:name,expectedDigest:models.find(m=>m.name===name)?.digest??null,parameters:{numCtx:ctx,numPredict:max,temperature:0,timeoutSeconds:timeout},requirements:["completion"]}));
      onPlan(await invoke<Batch>("plan_standard_benchmark",{workspaceId,input:{datasetId:dataset.id,datasetVersion:2,datasetHash:dataset.contentHash,models:requests.map(local=>({kind:"local",local})),caseIds:draft.cases,repetitions,maxOutputTokens:max,price:null,pythonImage:draft.image.trim()||null,releaseModels:draft.release?draft.chosen:[]}}));
    }catch(e){setError(errorText(e));}finally{setBusy(false);}
  };
  const toggle=(list:string[],id:string)=>list.includes(id)?list.filter(v=>v!==id):[...list,id];
  const visible=cases.filter(c=>`${c.id} ${c.prompt} ${ruleName[c.evaluator]}`.toLocaleLowerCase().includes(search.toLocaleLowerCase()));
  return <section className="personal-step" aria-labelledby="standard-setup-title">
    <h2 id="standard-setup-title">本地标准评测</h2>
    <p>使用本机模型，无需云端密钥。</p>
    {error&&<p role="alert" className="cloud-error">{error}</p>}
    {loading&&<p role="status">正在读取本地模型和固定题库…</p>}
    <form onSubmit={e=>{e.preventDefault();void plan(e.currentTarget);}}><fieldset className="personal-form" disabled={busy||loading}>
      <h3>{dataset?.manifest.name??"Core 60"} · v2</h3>
      <details className="personal-details"><summary>基础检查集的用途与历史版本</summary><p>Core 60 验证基础功能和评分器，不作为正式模型能力排行榜。v2 的 60 题可选题运行；v1 和既有单题记录保持原样。</p><ul>{datasets.map(d=><li key={d.version}>{d.manifest.name} · v{d.version} · {d.manifest.case_count} 题</li>)}</ul></details>
      <fieldset className="personal-choices"><legend>选择本地模型（最多 6 个）</legend>{models.map(model=><label key={model.name}><input type="checkbox" disabled={!model.digest} checked={draft.chosen.includes(model.name)} onChange={()=>update({chosen:toggle(draft.chosen,model.name)})}/>{model.name}{!model.digest&&"（指纹不可用）"}</label>)}{!loading&&!models.length&&<p>没有可用模型，请在设置页检查 Ollama，然后返回重新读取。</p>}</fieldset>
      <div className="profile-header"><h3>选择题目 · {draft.cases.length} / {cases.length}</h3><div className="cloud-actions"><button className="button secondary" type="button" onClick={()=>update({cases:quick.filter(id=>cases.some(c=>c.id===id))})}>先试 2 题</button><button className="button secondary" type="button" onClick={()=>update({cases:cases.map(c=>c.id)})}>选择全部</button><button className="button text-button" type="button" onClick={()=>update({cases:[]})}>清空选择</button></div></div>
      <label className="profile-field">查找题目<input type="search" value={search} onChange={e=>setSearch(e.target.value)}/></label>
      <div className="standard-case-list" role="group" aria-label="标准题目选择">{visible.map(item=><label key={item.id}><input type="checkbox" checked={draft.cases.includes(item.id)} onChange={()=>update({cases:toggle(draft.cases,item.id)})}/><span><strong>{item.id} · {ruleName[item.evaluator]}</strong><span>{item.prompt}</span>{item.requirements.includes("python_sandbox")&&<span>需要 Docker 隔离环境</span>}</span></label>)}</div>
      <div className="personal-row"><NumberField label="每题重复次数" min={1} max={5} value={draft.repetitions} onChange={repetitions=>update({repetitions})}/><NumberField label="回答 Token 上限" min={64} max={2048} value={draft.max} onChange={max=>{setCapacityInvalid(false);update({max});}}/></div>
      <details className="personal-details"><summary>模型参数与运行环境</summary><div className="personal-row"><div className="standard-context"><NumberField label="上下文容量" min={1024} max={16384} value={draft.ctx} error={capacityInvalid?capacityError:undefined} onChange={ctx=>{setCapacityInvalid(false);update({ctx});}}/></div><NumberField label="单题超时（秒）" min={30} max={600} value={draft.timeout} onChange={timeout=>update({timeout})}/></div><label className="profile-field">Python 3.13 Docker 镜像内容 ID<input value={draft.image} placeholder="sha256:…" onChange={e=>update({image:e.target.value})}/></label><p>缺少 Docker 时，Python 题会记录为运行环境出错，不在宿主机执行代码，也不计入答题正确率。</p><label className="personal-consent"><input type="checkbox" checked={draft.release} onChange={e=>update({release:e.target.checked})}/>切换模型时，允许释放上一个已选评测模型</label><p>只释放计划中明确授权的模型，发送前核对实际指纹；不会释放其他模型。默认保留现有加载状态。</p></details>
      <p>计划上限：{draft.chosen.length} 个模型 × {draft.cases.length} 题 × {draft.repetitions ?? "—"} 次 = {draft.repetitions===null?"—":draft.chosen.length*draft.cases.length*draft.repetitions} 次答题请求。</p>
      <button className="button primary" disabled={!dataset||!draft.cases.length||!draft.chosen.length||draft.chosen.length>6||busy}>预览标准评测计划</button>
    </fieldset></form>
  </section>;
}
