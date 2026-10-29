import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SelectMenu } from "../components/SelectMenu";
import { notify } from "../components/Toast";

const domains = { software:"软件开发", education:"学习与教育", finance:"金融与经济研究", data:"数据分析", science:"科学研究", writing:"内容创作", product:"产品与商业工作" };
const tasks = { coding:"代码编写", debugging:"Bug 修复", review:"代码审查", math:"数学推理", reading:"文档阅读", analysis:"数据分析", research:"研究报告", qa:"知识问答", automation:"任务自动化" };
const weights = { quality:"回答质量", speed:"运行速度", cost:"API 成本", resources:"本地资源占用", privacy:"隐私保护", coding:"编程能力", context:"长上下文能力" };
const levels:Record<string,string> = {beginner:"入门",intermediate:"中级",advanced:"高级"};
export interface Profile { domains:string[]; otherDomain:string; level:string|null; tasks:string[]; taskNotes:string; tools:string[]; weights:Record<string,number> }
interface Version {workspaceId:string;version:number;action:string;profile:Profile;contentHash:string;createdAt:string}
const defaults = ():Profile => ({domains:[],otherDomain:"",level:null,tasks:[],taskNotes:"",tools:[],weights:Object.fromEntries(Object.keys(weights).map(k=>[k,3]))});
// Unsaved answers survive page/workspace switches within this process; never contain credentials.
const drafts = new Map<string,{profile:Profile;version:number;step:number}>();
export function ProfileEditor({workspaceId,onConnection}:{workspaceId:string|null;onConnection:()=>void}) {
  if (!workspaceId) return <section className="profile-page"><h2>使用偏好</h2><p>请先选择工作区，再填写使用偏好。</p><button className="button secondary" onClick={onConnection}>返回连接设置</button></section>;
  return <WorkspaceProfile key={workspaceId} workspaceId={workspaceId} onConnection={onConnection} />;
}
function WorkspaceProfile({workspaceId,onConnection}:{workspaceId:string;onConnection:()=>void}) {
  const [saved,setSaved]=useState<Version|null>(null);
  const [profile,setProfile]=useState<Profile>(defaults);
  const [version,setVersion]=useState(0);
  const [step,setStep]=useState(0);
  const [editing,setEditing]=useState(true);
  const [loading,setLoading]=useState(true);
  const [busy,setBusy]=useState(false);
  const [dirty,setDirty]=useState(drafts.has(workspaceId));
  const [error,setError]=useState<string|null>(null);
  const [loadFailed,setLoadFailed]=useState(false);
  const [toolText,setToolText]=useState("");
  const active=useRef(true);
  const heading=useRef<HTMLHeadingElement>(null);
  const load=async()=>{
    setLoading(true);setError(null);setLoadFailed(false);
    try {
      const next=await invoke<Version|null>("get_benchmark_profile",{workspaceId});
      if (!active.current) return;
      const draft=drafts.get(workspaceId);
      setSaved(next);setVersion(draft?.version??next?.version??0);setProfile(draft?.profile??next?.profile??defaults());setStep(draft?.step??0);
      setDirty(!!draft);setEditing(!!draft||!next||next.action==="reset");
      setToolText((draft?.profile??next?.profile??defaults()).tools.join(", "));
    } catch (e) {if(active.current){setLoadFailed(true);setError(typeof e==="string"?e:"无法读取使用偏好，请重试。");}}
    finally {if(active.current)setLoading(false);}
  };
  useEffect(()=>{ active.current=true;void load();return()=>{active.current=false;}; },[workspaceId]);
  useEffect(()=>{const warn=(event:BeforeUnloadEvent)=>{if(drafts.size){event.preventDefault();event.returnValue="";}};window.addEventListener("beforeunload",warn);return()=>window.removeEventListener("beforeunload",warn);},[]);
  const change=(next:Profile)=>{setProfile(next);setDirty(true);drafts.set(workspaceId,{profile:next,version,step});};
  const move=(next:number)=>{setStep(next);const draft=drafts.get(workspaceId);if(draft)draft.step=next;heading.current?.focus();};
  const save=async(action:"save"|"skip"|"reset")=>{
    if(action!=="save" && !window.confirm(action==="reset"?"重置本工作区的使用偏好？旧版本会保留，新版本将恢复为未填写状态。":"跳过问卷并使用默认偏好？当前未保存的填写内容将清空，之后可以修改。"))return;
    setBusy(true);setError(null);
    try {
      const next=await invoke<Version>("save_benchmark_profile",{workspaceId,input:{expectedVersion:version,action,profile}});
      if(!active.current)return;
      setSaved(next);setVersion(next.version);setProfile(next.profile);setEditing(action==="reset");setStep(0);setDirty(false);drafts.delete(workspaceId);notify(action==="reset"?"使用偏好已重置":"使用偏好已保存");
      setToolText(next.profile.tools.join(", "));
    }catch(e){if(active.current)setError(typeof e==="string"?e:"保存未完成，填写内容已保留。请重试。");}
    finally{if(active.current)setBusy(false);}
  };
  const selected=(items:string[],key:string)=>items.includes(key)?items.filter(v=>v!==key):[...items,key];
  const choices=(options:Record<string,string>,value:string[],update:(next:string[])=>void)=><div className="profile-options">{Object.entries(options).map(([id,label])=><label key={id}><input type="checkbox" checked={value.includes(id)} onChange={()=>update(selected(value,id))}/>{label}</label>)}</div>;
  const toolOptions=Array.from(new Set([...(profile.domains.includes("software")?["Python","TypeScript","Rust","React","SQL"]:[]),...(profile.domains.some(d=>["data","finance","science"].includes(d))?["Python","SQL","Excel","R"]:[]),"Excel","其他工具"])).filter(v=>v!=="其他工具");
  const total=Object.values(profile.weights).reduce((a,b)=>a+b,0);
  return <section className="profile-page" aria-labelledby="profile-title">
    <header className="profile-header"><div><h2 id="profile-title" ref={heading} tabIndex={-1}>使用偏好</h2>{saved&&<span>版本 {saved.version} · {new Date(saved.createdAt).toLocaleString("zh-CN")}</span>}</div><button className="button text-button" disabled={busy} onClick={onConnection}>连接设置</button></header>
    {loading?<p role="status">正在读取使用偏好…</p>:<>
      {error&&<div role="alert" className="cloud-error"><p>{error}</p><div className="cloud-actions"><button className="button secondary" disabled={busy} onClick={()=>void load()}>重新读取（保留草稿）</button><button className="button secondary" disabled={busy} onClick={onConnection}>检查云端连接</button>{dirty&&<button className="button secondary" disabled={busy} onClick={()=>{if(window.confirm("放弃当前未保存的修改并读取最新版本？")){drafts.delete(workspaceId);void load();}}}>放弃草稿并读取最新版本</button>}</div></div>}
      {editing?<form onSubmit={e=>{e.preventDefault();if(step<2)move(step+1);else void save("save");}}>
        <nav className="profile-steps" aria-label="使用偏好步骤">{["工作与水平","任务与工具","选择模型的偏好"].map((label,i)=><button type="button" key={label} aria-current={step===i?"step":undefined} className="button secondary" disabled={busy} onClick={()=>move(i)}>{i+1}. {label}</button>)}</nav>
        <fieldset disabled={busy||loadFailed}><legend className="sr-only">{["工作与水平","任务与工具","选择模型的偏好"][step]}</legend>
          {step===0&&<><h3>你主要从事什么？</h3>{choices(domains,profile.domains,next=>change({...profile,domains:next}))}<label className="profile-field">其他领域<input maxLength={120} value={profile.otherDomain} onChange={e=>change({...profile,otherDomain:e.target.value})}/></label><label className="profile-field">专业水平<SelectMenu aria-label="专业水平" value={profile.level??""} onChange={e=>change({...profile,level:e.target.value||null})}><option value="">暂不填写</option>{Object.entries(levels).map(([id,label])=><option key={id} value={id}>{label}</option>)}</SelectMenu></label></>}
          {step===1&&<><h3>你最常用 AI 完成哪些任务？</h3>{choices(tasks,profile.tasks,next=>change({...profile,tasks:next}))}<label className="profile-field">补充你的任务<textarea rows={3} maxLength={2000} value={profile.taskNotes} onChange={e=>change({...profile,taskNotes:e.target.value})}/></label><h3>常用工具和环境</h3>{choices(Object.fromEntries(toolOptions.map(v=>[v,v])),profile.tools,next=>{setToolText(next.join(", "));change({...profile,tools:next});})}<label className="profile-field">工具清单（用逗号分隔，最多 20 个）<input value={toolText} onChange={e=>{setToolText(e.target.value);change({...profile,tools:Array.from(new Set(e.target.value.split(/[,，]/).map(v=>v.trim()).filter(Boolean)))});}}/></label></>}
          {step===2&&<><h3>选择模型时，你更看重什么？</h3><p>0 表示不考虑，5 表示非常看重。这些权重记录偏好，不代表模型成绩。</p><div className="profile-weights">{Object.entries(weights).map(([id,label])=><label key={id}><span>{label}</span><input aria-label={label} type="range" min={0} max={5} step={1} value={profile.weights[id]} onChange={e=>change({...profile,weights:{...profile.weights,[id]:Number(e.target.value)}})}/><output>{profile.weights[id]} / 5</output></label>)}</div>{total===0&&<p role="alert" className="cloud-error">至少将一项偏好设为 1 或更高。</p>}</>}
        </fieldset>
        <footer className="profile-footer"><span role="status">{dirty?"有未保存的修改":"所有问题均可选填"}</span><div className="cloud-actions"><button type="button" className="button text-button" disabled={busy||loadFailed} onClick={()=>void save("skip")}>暂时跳过</button>{step>0&&<button type="button" className="button secondary" disabled={busy} onClick={()=>move(step-1)}>上一步</button>}<button className="button primary" type="submit" disabled={busy||loadFailed||(step===2&&total===0)}>{busy?"正在保存…":step<2?"下一步":"保存使用偏好"}</button></div></footer>
      </form>:<><dl className="profile-summary"><dt>领域</dt><dd>{[...profile.domains.map(k=>domains[k as keyof typeof domains]),profile.otherDomain].filter(Boolean).join("、")||"未填写"}</dd><dt>专业水平</dt><dd>{profile.level?levels[profile.level]:"未填写"}</dd><dt>常见任务</dt><dd>{profile.tasks.map(k=>tasks[k as keyof typeof tasks]).join("、")||"未填写"}</dd><dt>任务补充</dt><dd className="profile-notes">{profile.taskNotes||"未填写"}</dd><dt>常用工具</dt><dd>{profile.tools.join("、")||"未填写"}</dd><dt>偏好权重</dt><dd>{Object.entries(weights).map(([id,label])=><div key={id}>{label}：{profile.weights[id]} / 5</div>)}</dd></dl>{saved?.action==="skip"&&<p>已跳过问卷，当前使用默认偏好。</p>}<div className="cloud-actions"><button className="button primary" disabled={busy} onClick={()=>{setEditing(true);setStep(0);}}>修改使用偏好</button><button className="button secondary" disabled={busy} onClick={()=>void save("reset")}>重置使用偏好</button></div><p className="profile-next">偏好保存在当前工作区。可以继续选择项目资料或直接生成候选题。</p></>}
    </>}
  </section>;
}
