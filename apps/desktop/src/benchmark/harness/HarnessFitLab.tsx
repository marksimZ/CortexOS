import { useEffect,useRef,useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FilePlus2,FileText,X } from 'lucide-react';
import { AppDialog,openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { FileName } from '../../components/FileName';
import { notify } from '../../components/Toast';
import { errorText } from '../personalized';
import { RequirementEditor } from './RequirementEditor';
import { FitReport } from './FitReport';
import { FitStatus } from './FitStatus';
import { HarnessExperiments } from './HarnessExperiments';
import { TaskWorkbench } from './TaskWorkbench';
import { groups,type Assessment,type Body,type Payload,type Requirement,type Snapshot } from './types';
import './harness.css';
interface Draft {ids:string[];body:Body|null;weights:Record<string,number>;note:string;version:number;step:number;stale:boolean;reportStale:boolean;dirty:boolean}
const drafts=new Map<string,Draft>();
const empty=():Draft=>({ids:[],body:null,weights:{compatibility:3,automation:3,governance:3,context:3},note:'',version:0,step:0,stale:false,reportStale:false,dirty:false});
export function HarnessFitLab({workspaceId,desktop}:{workspaceId:string|null;desktop:boolean}){
 if(!workspaceId||!desktop)return <section><p>请在桌面应用中选择工作区，导入项目规范后开始分析。</p></section>;
 return <WorkspaceLab key={workspaceId} workspaceId={workspaceId}/>;
}
function WorkspaceLab({workspaceId}:{workspaceId:string}){
 const [data,setData]=useState<Payload|null>(null),[s,setS]=useState<Draft>(empty),[busy,setBusy]=useState(false),[error,setError]=useState(''),[historical,setHistorical]=useState(false);
 const [source,setSource]=useState<{path:string;content:string;line:number}|null>(null);
 const sourceDialog=useRef<HTMLDialogElement>(null),confirmDialog=useRef<HTMLDialogElement>(null),active=useRef(true),pending=useRef<()=>void>(()=>{});
 const [confirmation,setConfirmation]=useState('');
 const setDraft=(patch:Partial<Draft>)=>setS(prev=>{const next={...prev,...patch,dirty:true};drafts.set(workspaceId,next);return next;});
 const from=(a:Assessment|null):Draft=>a?{...empty(),ids:a.body.snapshotIds,body:a.body,weights:a.body.weights,note:a.body.note,version:a.version}:empty();
 const read=async(version:number|null=null,keepDraft=true)=>{
  setBusy(true);setError('');try{
   const next=await invoke<Payload>('get_harness_fit',{workspaceId,version});if(!active.current)return;
   setData(next);setHistorical(version!==null&&version!==next.history[0]?.version);
   setS(keepDraft&&version===null&&drafts.has(workspaceId)?drafts.get(workspaceId)!:from(next.selected));
  }catch(e){if(active.current)setError(errorText(e));}finally{if(active.current)setBusy(false);}
 };
 useEffect(()=>{active.current=true;void read();return()=>{active.current=false;};},[workspaceId]);
 useEffect(()=>{const warn=(e:BeforeUnloadEvent)=>{if(drafts.size){e.preventDefault();e.returnValue='';}};window.addEventListener('beforeunload',warn);return()=>window.removeEventListener('beforeunload',warn);},[]);
 const confirm=(text:string,action:()=>void)=>{pending.current=action;setConfirmation(text);openDialog(confirmDialog.current);};
 const importFile=async()=>{
  setBusy(true);setError('');try{
   const snapshot=await invoke<Snapshot|null>('import_harness_document',{workspaceId});if(!active.current)return;
   if(!snapshot){notify('已取消选择');return;}
   const next=await invoke<Payload>('get_harness_fit',{workspaceId,version:null});if(!active.current)return;
   setData(next);setDraft({ids:[...s.ids,snapshot.id],stale:true});notify('文档已保存在本机');
  }catch(e){if(active.current)setError(errorText(e));}finally{if(active.current)setBusy(false);}
 };
 const calculate=async(reset:boolean,save=false)=>{
  if(!reset&&!save&&s.body&&!s.reportStale&&!s.stale){setS(prev=>({...prev,step:2}));return;}
  setBusy(true);setError('');try{
   const input={snapshotIds:s.ids,requirements:reset?[]:s.body?.requirements??[],weights:s.weights,expectedVersion:s.version,note:s.note};
   if(save){
    const saved=await invoke<Assessment>('save_harness_fit',{workspaceId,input});if(!active.current)return;
    drafts.delete(workspaceId);setS({...from(saved),step:2});setHistorical(false);
    const next=await invoke<Payload>('get_harness_fit',{workspaceId,version:null});if(active.current)setData(next);notify('分析版本已保存');
   }else{
    const body=await invoke<Body>('analyze_harness_fit',{workspaceId,input});if(!active.current)return;
    setDraft({body,stale:false,reportStale:false,step:reset?1:2});
   }
  }catch(e){if(active.current)setError(errorText(e));}finally{if(active.current)setBusy(false);}
 };
 const viewSource=async(r:Requirement)=>{
  if(!r.evidence)return;setError('');try{
   const docs=await invoke<{path:string;content:string}[]>('get_harness_document',{workspaceId,snapshotId:r.evidence.sourceId});if(!active.current)return;
   const doc=docs.find(d=>d.path===r.evidence!.path);if(!doc)throw '来源文档已缺失，请检查本地数据。';
   setSource({...doc,line:r.evidence.startLine});openDialog(sourceDialog.current);
  }catch(e){if(active.current)setError(errorText(e));}
 };
 const changeRequirement=(r:Requirement)=>{if(s.body)setDraft({body:{...s.body,requirements:s.body.requirements.map(old=>old.id===r.id?r:old)},reportStale:true});};
 const latest=data?.history[0]?.version??0;
 return <section className="harness-fit-lab" aria-label="项目与Harness适配分析">
  <div className="fit-toolbar">
   <nav className="fit-steps" aria-label="项目适配步骤">{['项目资料','需求确认','适配结果','测试任务','组合实验'].map((label,i)=><button key={label} className="button secondary" aria-current={s.step===i?'step':undefined} disabled={busy||(i>0&&(!s.body||s.stale))||(i>=3&&(!s.version||s.dirty))} onClick={()=>i===2&&!historical?void calculate(false):setS(prev=>({...prev,step:i}))}>{i+1}. {label}</button>)}</nav>
   <div className="fit-analysis-actions cloud-actions" role="group" aria-label="分析版本操作">
   {data&&<label>已保存版本<SelectMenu aria-label="分析历史版本" disabled={busy} value={historical?s.version:''} onChange={e=>{const version=e.target.value?Number(e.target.value):null;const action=()=>void read(version,false);if(s.dirty)confirm('切换版本将放弃本次未保存的修改。旧的已保存版本会保留。',()=>{drafts.delete(workspaceId);action();});else action();}}><option value="">最新版本{latest?` · ${latest}`:''}</option>{data.history.map(v=><option key={v.version} value={v.version}>版本 {v.version} · {new Date(v.createdAt).toLocaleString()}</option>)}</SelectMenu></label>}
   <button className={`button primary${s.step>=3?' fit-save-inactive':''}`} disabled={busy||historical||!s.body||s.stale||s.step>=3} onClick={()=>void calculate(false,true)}>保存新版本</button>
  </div></div>
  <div className="fit-scroll" role="region" aria-label="项目适配内容" tabIndex={0}>
  {error&&<div className="cloud-error" role="alert"><p>{error}</p><button className="button secondary" disabled={busy} onClick={()=>void read(null,true)}>重新读取（保留草稿）</button></div>}
  {!data?<p role="status">{busy?'正在读取项目适配…':'尚未读取到本地资料。'}</p>:<>
   {historical&&<p className="fit-notice">正在查看历史版本，内容与目录保持保存时的状态。选择“最新版本”后继续修改。</p>}
   <div className="fit-workspace">
    <aside className="fit-documents" aria-label="项目资料">
     <h3>项目资料</h3><button className="button secondary" disabled={busy||historical||s.step>=3||s.ids.length>=12} onClick={()=>void importFile()}><FilePlus2 size={16} aria-hidden="true"/>导入规范文档</button>
     <details><summary>从已有项目快照选择</summary><ul className="fit-snapshot-options">{data.snapshots.map(snapshot=><li key={snapshot.id}><label><input type="checkbox" disabled={busy||historical||s.step>=3||(!s.ids.includes(snapshot.id)&&s.ids.length>=12)} checked={s.ids.includes(snapshot.id)} onChange={e=>setDraft({ids:e.target.checked?[...s.ids,snapshot.id]:s.ids.filter(id=>id!==snapshot.id),stale:true})}/><FileName name={snapshot.name}/></label></li>)}</ul>{data.snapshots.length===0&&<p>当前工作区还没有项目快照。</p>}</details>
     <ul className="fit-selected-docs">{s.ids.map(id=><li key={id}><FileText size={15} aria-hidden="true"/><FileName name={data.snapshots.find(x=>x.id===id)?.name??'已保存来源快照'}/><button className="icon-button" disabled={busy||historical||s.step>=3} aria-label={`移除资料 ${data.snapshots.find(x=>x.id===id)?.name??id}`} onClick={()=>setDraft({ids:s.ids.filter(v=>v!==id),stale:true})}><X size={14}/></button></li>)}</ul>
     <button className="button primary" disabled={busy||historical||s.step>=3||!s.ids.length} onClick={()=>{if(s.body)confirm('重新分析会重新生成待确认线索，替换本次修改。已保存的历史版本保持不变。',()=>void calculate(true));else void calculate(true);}}>分析所选资料</button>
     <details><summary>读取范围与方法</summary><p>仅导入你选择的UTF-8 .md/.txt文件，单文件最多2 MiB。也可复用已有项目快照的文档与项目摘要。规则匹配只产生待确认线索，不执行文档指令，不联网调用模型。</p></details>
    </aside>
    <div className="fit-main">
     {s.body&&s.dirty&&<p className="fit-notice">先保存分析版本，再生成绑定此版本的测试任务。</p>}
     {busy&&<p role="status">正在处理本地资料…</p>}
     {s.stale&&<p className="fit-notice">资料选择已变化，请先分析所选资料。</p>}
     {s.step===4&&s.body?<HarnessExperiments key={`${workspaceId}:${s.version}`} workspaceId={workspaceId} assessmentVersion={s.version}/>:s.step===3&&s.body?<TaskWorkbench key={`${workspaceId}:${s.version}`} workspaceId={workspaceId} assessmentVersion={s.version} requirements={s.body.requirements} snapshots={data.snapshots}/>:s.step===0||!s.body||s.stale?<div className="fit-start"><h3>先了解项目，再选择编程工具</h3><p>导入development.md、AGENTS.md或项目说明，确认需求后查看候选工具及每项依据。</p><h4>本地能力目录</h4><ul>{data.catalog.entries.map(e=><li className="fit-catalog-item" key={e.id}><span>{e.name}</span>{e.version?<span>{e.version}</span>:<FitStatus tone="attention">安装版本未知</FitStatus>}</li>)}</ul></div>:s.step===1?<>
      <div className="fit-section-heading"><h3>确认项目要求</h3><button className="button secondary" disabled={busy||historical} onClick={()=>void calculate(false)}>查看适配结果</button></div>
      <p>线索保留原文；请检查否定、举例和未来计划。关联条件表示需要支持该能力；不确定的表述先保持未知，确认后再标记“必须满足”。</p>
      <RequirementEditor data={data} requirements={s.body.requirements} disabled={busy||historical} onChange={changeRequirement} onSource={r=>void viewSource(r)}/>
     </>:<>
      <fieldset className="fit-weights" disabled={busy||historical}><legend>比较侧重点</legend>{Object.entries(groups).map(([key,label])=><label key={key}>{label}<input aria-label={`${label}权重`} type="number" min={0} max={5} value={s.weights[key]} onChange={e=>setDraft({weights:{...s.weights,[key]:Number(e.target.value)},reportStale:true})}/></label>)}</fieldset>
      {s.reportStale?<div className="fit-notice"><p>需求或权重已修改，请更新分析后查看比例。</p><button className="button primary" disabled={busy} onClick={()=>void calculate(false)}>更新分析</button></div>:<FitReport body={s.body} criteria={data.criteria} onSource={r=>void viewSource(r)}/>}
     </>}
     {s.body&&!historical&&!s.stale&&s.step<3&&<label className="profile-field fit-version-note">本次确认或修改说明<input maxLength={1000} value={s.note} onChange={e=>setDraft({note:e.target.value})}/></label>}
    </div>
   </div>
  </>}
  </div>
  <AppDialog ref={sourceDialog} className="panel-dialog fit-source-dialog" aria-labelledby="fit-source-title"><header className="panel-header"><h2 id="fit-source-title">{source?.path} · 第{source?.line}行</h2><button className="icon-button" aria-label="关闭项目原文" onClick={()=>sourceDialog.current?.close()}><X size={18}/></button></header><div className="panel-body"><pre>{source?.content.split('\n').map((line,i)=><span className="fit-source-line" data-active={i+1===source.line} key={i} ref={el=>{if(el&&i+1===source.line)el.scrollIntoView({block:'center'});}}>{i+1}  {line}{'\n'}</span>)}</pre></div></AppDialog>
  <AppDialog ref={confirmDialog} className="panel-dialog form-dialog" aria-labelledby="fit-confirm-title"><header className="panel-header"><h2 id="fit-confirm-title">确认切换</h2></header><div className="panel-body"><p>{confirmation}</p></div><footer className="panel-footer dialog-actions"><button className="button secondary" onClick={()=>confirmDialog.current?.close()}>保留当前修改</button><button className="button primary" onClick={()=>{confirmDialog.current?.close();pending.current();}}>继续</button></footer></AppDialog>
 </section>;
}
