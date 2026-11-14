import { lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { AppDialog, openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { notify } from '../../components/Toast';
import { errorText } from '../personalized';
import { FitStatus } from './FitStatus';
import type { Point } from '../../graph-data';
import type { ViewportApi } from '../../GraphViewport';

const Viewport = lazy(() => import('../../GraphViewport'));
const tools: Record<string,string> = {codex:'Codex','claude-code':'Claude Code',aider:'Aider'};
const states: Record<string,string> = {running:'运行中',grading:'评分中',completed:'已结束',failed:'失败',unsupported:'不支持',queued:'排队中',not_run:'未运行',cancelled:'已停止',interrupted:'已中断',closed:'已清理',cleanup_required:'待清理'};
const failures: Record<string,string> = {not_run:'未运行',interruption:'中断或停止',scope_or_permission:'范围或权限拦截',budget:'预算或超时',unsupported:'不支持',model_or_provider:'模型或服务',environment:'运行环境',assertions:'断言未通过',scoring:'评分错误',execution:'执行失败',not_scored:'无有效成绩'};
const scoreNames: Record<string,string> = {Passed:'通过',Failed:'未通过',Unsupported:'不支持','Not Run':'未运行','Execution Error':'环境错误','Scoring Error':'评分错误',Cancelled:'已停止','Needs Review':'待复核'};
const eventNames: Record<string,string> = {prepared:'准备隔离副本',protocol_request:'收到工具请求',model_requested:'发出模型请求',model_response:'收到模型回复',configuration:'启动参数',cli:'工具实际输出',diagnostic:'运行诊断',exit:'工具退出',candidate:'保存候选修改',cleanup:'清理执行容器',score:'独立评分',grader_container:'启动评分容器',grader_cleanup:'清理评分容器',cleanup_error:'清理失败',execution_error:'执行失败',controller_error:'通道拒绝',collection_error:'收集失败'};
const number = (v:number|null|undefined) => v==null?'—':v.toLocaleString('zh-CN');
const percent = (v:number|null|undefined) => v==null?'—':`${(v*100).toFixed(1)}%`;
const time = (ms:number|null) => ms==null?'—':`${(ms/1000).toFixed(1)} 秒`;

interface Option {id:string;hash:string;harness:string;harnessVersion:string;taskKey:string;datasetId:string;datasetVersion:number;function:string|null}
interface Model {name:string;digest:string|null}
interface Selection {id:number;runtime:string;model:string}
interface Listing {history:{id:string;status:string;createdAt:string}[];options:Option[];active:{id:string;assessmentVersion:number}|null;maxRuns:number}
interface Result {
  id:string;taskKey:string;function:string;datasetVersion:number;weight:number;repetition:number;
  harness:string;harnessVersion:string;model:string;digest:string;status:string;error:string|null;failureCategory:string;
  scoreState:string|null;assertions:{assertionsRun?:number;assertionsPlanned?:number;assertionsPassed?:number}|null;
  candidateChanged:boolean|null;passed:boolean;durationMs:number|null;inputTokens:number|null;outputTokens:number|null;
  requests:number;responses:number;apiCost:number|null;approvalCount:number|null;blockedActions:number;buildStatus:string;recorded:boolean;clean:boolean|null;pendingGraders:string[]|null;
}
interface Summary {
  harness:string;harnessVersion:string;model:string;digest:string;planned:number;finished:number;passed:number;
  weightedSuccess:number|null;successInterval95:[number,number]|null;assertionsPassed:number;assertionsRun:number;
  assertionsPlannedForScoredTasks:number;scoredTasks:number;patches:number;durationMeanMs:number|null;durationSamples:number;
  inputTokens:number|null;outputTokens:number|null;apiCost:number|null;blockedActions:number;nativePermissionDenials:number|null;buildsRequired:number;buildsRun:number;buildsPassed:number;failures:Record<string,number>;
}
interface Report {
  id:string;status:string;error:string|null;planHash:string;experimentApprovals:number;
  plan:{assessmentVersion:number;provider:string;evidence:string;hardware:Record<string,unknown>;items:Record<string,unknown>[];repetitions:number};
  rows:Result[];summary:Summary[];staticFit:{catalogVersion:string;results:{harnessId:string;eligibility:string;lowerPercent:number|null;upperPercent:number|null}[]};
  comparison:{sameTasks:boolean;sameScoringEnvironment:boolean;complete:boolean;comparable:boolean;suggested:Summary|null};
}
interface Trace {id:string;events:{kind:string;createdAt:string;data:Record<string,unknown>}[]}
function eventTitle(event:Trace['events'][number]) {
  if (event.kind==='permission_decision') return '应用拒绝权限请求';
  if (event.kind==='permission_observation') return '工具报告权限拦截';
  if (event.kind==='cli' && event.data.parsed && typeof event.data.parsed==='object') {
    const parsed=event.data.parsed as {type?:string;item?:{type?:string};message?:{content?:{type?:string;name?:string}[]}};
    if (parsed.item?.type==='command_execution') return parsed.type==='item.started'?'启动工具命令':'工具命令结果';
    const call=Array.isArray(parsed.message?.content)?parsed.message.content.find(part=>part.type==='tool_use'):undefined;
    if (call?.name) return `工具调用 ${call.name}`;
    if (Array.isArray(parsed.message?.content)&&parsed.message.content.some(part=>part.type==='tool_result')) return '工具执行结果';
  }
  return eventNames[event.kind]??event.kind;
}

const buildStatus:Record<string,string>={not_applicable:'不适用',not_run:'未执行',passed:'通过',failed:'未通过'};
const money=(v:number|null)=>v===null?'—':`¥${v.toLocaleString('zh-CN')}`;
const chartMetrics={
  success:{label:'加权通过率',value:(s:Summary)=>s.weightedSuccess,display:percent},
  time:{label:'平均耗时',value:(s:Summary)=>s.durationMeanMs,display:time},
  cost:{label:'API 费用',value:(s:Summary)=>s.apiCost,display:money},
};
function ExperimentChart({summary}:{summary:Summary[]}) {
  const [selected,setSelected]=useState<keyof typeof chartMetrics>('success');
  const metric=chartMetrics[selected];
  const maximum=selected==='success'?1:Math.max(0,...summary.map(row=>metric.value(row)??0));
  return <figure className="harness-comparison-chart">
    <figcaption><strong>组合对比</strong><SelectMenu aria-label="组合图表指标" value={selected} onChange={event=>setSelected(event.target.value as typeof selected)}>{Object.entries(chartMetrics).map(([key,value])=><option key={key} value={key}>{value.label}</option>)}</SelectMenu></figcaption>
    <ul aria-label={metric.label}>{summary.map(row=>{const value=metric.value(row);return <li key={`${row.harness}:${row.harnessVersion}:${row.model}:${row.digest}`}>
      <span className="harness-chart-name">{tools[row.harness]}<span>{row.model}</span></span>
      <span className="harness-chart-track" aria-hidden="true"><span style={{width:`${maximum?(value??0)/maximum*100:0}%`}}/></span>
      <strong>{value===null?'暂无数据':metric.display(value)}</strong>
    </li>;})}</ul>
  </figure>;
}

export function HarnessExperiments({workspaceId,assessmentVersion}:{workspaceId:string;assessmentVersion:number}) {
  const scope={workspaceId,assessmentVersion};
  const [listing,setListing]=useState<Listing|null>(null);
  const [models,setModels]=useState<Model[]>([]);
  const [modelError,setModelError]=useState('');
  const [selections,setSelections]=useState<Selection[]>([]);
  const [weights,setWeights]=useState<Record<string,number>>({});
  const [repetitions,setRepetitions]=useState(1);
  const [report,setReport]=useState<Report|null>(null);
  const [selected,setSelected]=useState('');
  const [traceId,setTraceId]=useState('');
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [filter,setFilter]=useState('all');
  const dialog=useRef<HTMLDialogElement>(null);
  const nextId=useRef(0);
  const alive=useRef(true);
  const epoch=useRef(0);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;epoch.current++;};},[]);

  async function load(id?:string, refreshModels=false) {
    const ticket=++epoch.current;
    const data=await invoke<Listing>('list_harness_experiments',scope);
    if (!alive.current || ticket!==epoch.current) return;
    setListing(data);
    const current=id??selected;
    const choice=data.history.some(h=>h.id===current)?current:data.history[0]?.id??'';
    setSelected(choice);
    const result=choice?await invoke<Report>('get_harness_experiment',{workspaceId,experimentId:choice}):null;
    if (!alive.current || ticket!==epoch.current) return;
    setReport(result);
    if (!refreshModels) return;
    try {
      const catalog=await invoke<{models:Model[]}>('list_benchmark_models',{workspaceId});
      if (!alive.current || ticket!==epoch.current) return;
      const list=catalog.models.filter(m=>m.digest);
      setModels(list);setModelError('');
      setSelections(old=>old.length?old:data.options.slice(0,2).map(o=>({id:nextId.current++,runtime:o.id,model:list[0]?.name??''})));
    } catch (value) {
      if (alive.current && ticket===epoch.current) setModelError(errorText(value));
    }
  }
  async function act(operation:()=>Promise<void>) {
    setBusy(true);setError('');
    try {await operation();} catch(value) {if (alive.current) setError(errorText(value));}
    finally {if (alive.current) setBusy(false);}
  }
  useEffect(()=>{void act(()=>load(undefined,true));},[]);
  useEffect(()=>{
    if (!listing?.active || busy) return;
    let current=true;
    let timer:ReturnType<typeof setTimeout>;
    async function poll() {
      try {await load(selected);} catch(value) {if (current && alive.current) setError(errorText(value));}
      if (current) timer=setTimeout(()=>void poll(),900);
    }
    timer=setTimeout(()=>void poll(),900);
    return()=>{current=false;clearTimeout(timer);};
  },[listing?.active?.id,selected,busy]);

  const options=listing?.options??[];
  const resolved=selections.map(s=>({option:options.find(o=>o.id===s.runtime),model:models.find(m=>m.name===s.model)}));
  const runCount=selections.length*repetitions;
  const canStart=!busy&&!listing?.active&&selections.length>=2&&runCount<=(listing?.maxRuns??24)&&resolved.every(({option,model})=>option&&model?.digest);
  function update(id:number, patch:Partial<Selection>) {setSelections(items=>items.map(item=>item.id===id?{...item,...patch}:item));}
  function start() {
    if (!canStart) return;
    dialog.current?.close();
    void act(async()=>{
      const next=await invoke<{id:string}>('start_harness_experiment',{workspaceId,input:{assessmentVersion,repetitions,confirmed:true,selections:resolved.map(({option,model})=>({runtimeId:option!.id,runtimeHash:option!.hash,model:model!.name,digest:model!.digest,weight:weights[option!.taskKey]??1}))}});
      setTraceId('');await load(next.id);
    });
  }
  const switchReport=(id:string)=>{setTraceId('');setFilter('all');void act(()=>load(id));};
  return <section className="harness-experiment" aria-label="组合对照实验">
    <div className="fit-section-heading"><h3>模型与编程工具对照</h3><button className="button secondary" disabled={busy} onClick={()=>void act(()=>load(selected,true))}>刷新检查与记录</button></div>
    {error&&<p className="cloud-error" role="alert">{error}</p>}
    {modelError&&<p className="cloud-error" role="alert">{modelError}</p>}
    <details className="task-setup" open={!listing?.history.length || undefined}>
      <summary>配置实验</summary>
      {!options.length&&<p>请先在“测试任务”完成一个已发布编程任务的运行前检查。</p>}
      <div className="fit-comparison" tabIndex={0} aria-label="实验配置列表"><table>
        <thead><tr><th>工具与任务版本</th><th>本地模型</th><th>任务权重</th><th>操作</th></tr></thead>
        <tbody>{selections.map((item,i)=>{
          const option=resolved[i].option;
          return <tr key={item.id}>
            <td><SelectMenu aria-label={`第 ${i+1} 行工具与任务`} value={item.runtime} disabled={busy||!!listing?.active} onChange={e=>update(item.id,{runtime:e.target.value})}><option value="">选择已通过的检查</option>{options.map(o=><option key={o.id} value={o.id}>{tools[o.harness]} / {o.function??o.datasetId} / v{o.datasetVersion}</option>)}</SelectMenu></td>
            <td><SelectMenu aria-label={`第 ${i+1} 行模型`} value={item.model} disabled={busy||!!listing?.active} onChange={e=>update(item.id,{model:e.target.value})}><option value="">选择本地模型</option>{models.map(m=><option key={m.name} value={m.name}>{m.name}</option>)}</SelectMenu></td>
            <td><SelectMenu aria-label={`第 ${i+1} 行任务权重`} value={String(option?weights[option.taskKey]??1:1)} disabled={!option||busy||!!listing?.active} onChange={e=>option&&setWeights(old=>({...old,[option.taskKey]:Number(e.target.value)}))}>{Array.from({length:10},(_,index)=><option key={index+1} value={index+1}>{index+1}</option>)}</SelectMenu></td>
            <td><button className="button text-button" aria-label={`移除第 ${i+1} 行`} disabled={busy||!!listing?.active} onClick={()=>setSelections(items=>items.filter(s=>s.id!==item.id))}>移除</button></td>
          </tr>;
        })}</tbody>
      </table></div>
      <div className="cloud-actions">
        <button className="button secondary" disabled={busy||!!listing?.active||!options.length||selections.length>=24} onClick={()=>setSelections(items=>[...items,{id:nextId.current++,runtime:options[0].id,model:models[0]?.name??''}])}>添加组合</button>
        <label>重复次数<SelectMenu aria-label="实验重复次数" value={String(repetitions)} disabled={busy||!!listing?.active} onChange={e=>setRepetitions(Number(e.target.value))}>{[1,2,3].map(n=><option key={n} value={n}>{n}</option>)}</SelectMenu></label>
        <button className="button primary" disabled={!canStart} onClick={()=>openDialog(dialog.current)}>确认运行 {runCount} 个任务…</button>
      </div>
      <p>相同任务版本共享权重。固定同一模型可比较工具，固定同一工具可比较模型。</p>
    </details>
    {!!listing?.history.length&&<div className="cloud-actions">
      <label>实验记录<SelectMenu aria-label="组合实验历史" value={selected} disabled={busy} onChange={e=>switchReport(e.target.value)}>{listing.history.map(h=><option key={h.id} value={h.id}>{new Date(h.createdAt).toLocaleString()} / {states[h.status]??h.status}</option>)}</SelectMenu></label>
      {listing.active&&<button className="button secondary" disabled={busy} onClick={()=>void act(async()=>{await invoke('stop_harness_experiment',{workspaceId,experimentId:listing.active!.id});await load(selected);})}>停止运行中的实验</button>}
    </div>}
    {report&&<>
      <div className="fit-meta-line"><FitStatus tone={report.status==='completed'?'success':'neutral'}>{states[report.status]??report.status}</FitStatus><strong>{report.plan.evidence==='test_endpoint'?'测试端点记录':'本地模型实测'}</strong><span>分析版本 {report.plan.assessmentVersion} / 重复 {report.plan.repetitions} 次</span></div>
      {report.error&&<p role="alert" className="cloud-error">{report.error}</p>}
      <p>{report.comparison.suggested?`本批优先考虑 ${tools[report.comparison.suggested.harness]} / ${report.comparison.suggested.model}。`:'当前样本与区间不足以推荐领先组合。'} {!report.comparison.sameTasks?'组合覆盖的任务版本不同，请按任务查看结果。':''} {!report.comparison.sameScoringEnvironment?'评分环境有差异，请按任务查看结果。':''}</p>
      {report.summary.length>0&&<ExperimentChart summary={report.summary}/>}
      <div className="fit-comparison" tabIndex={0} aria-label="组合实验汇总"><table>
        <thead><tr><th>组合</th><th>通过 / 结束 / 计划</th><th>加权通过率</th><th>95% 区间</th><th>断言通过 / 执行</th><th>构建通过 / 执行</th><th>平均耗时</th><th>输入 / 输出 Token</th></tr></thead>
        <tbody>{report.summary.map(s=><tr key={`${s.harness}:${s.harnessVersion}:${s.model}:${s.digest}`}>
          <th scope="row">{tools[s.harness]} {s.harnessVersion}<br/>{s.model}</th><td>{s.passed} / {s.finished} / {s.planned}</td><td>{percent(s.weightedSuccess)}</td><td>{s.successInterval95?.map(percent).join(' – ')??'—'}</td><td>{s.assertionsPassed} / {s.assertionsRun}</td><td>{s.buildsRequired?`${s.buildsPassed} / ${s.buildsRun}`:'不适用'}</td><td>{time(s.durationMeanMs)}</td><td>{number(s.inputTokens)} / {number(s.outputTokens)}</td>
        </tr>)}</tbody>
      </table></div>
      <details className="personal-details"><summary>覆盖、失败、费用和静态兼容依据</summary>
        <p>区间按未加权样本计算。同一任务的重复结果可能相关，小样本仅用于描述本批结果。断言覆盖按已有评分记录的已执行 / 计划计算。</p>
        <ul>{report.summary.map(s=><li key={`${s.harness}:${s.model}:${s.digest}`}>{tools[s.harness]} / {s.model}，断言覆盖 {s.assertionsRun}/{s.assertionsPlannedForScoredTasks}（{s.scoredTasks} 题已评分），正确补丁 {s.passed}/{s.patches}，已记录的范围或权限拦截 {s.blockedActions}，API 费用 {s.apiCost==null?'未知':money(s.apiCost)}。{s.buildsRequired>0&&`构建覆盖 ${s.buildsRun}/${s.buildsRequired}。`}{s.nativePermissionDenials!=null&&`工具报告的权限拒绝 ${s.nativePermissionDenials} 次。`}{Object.entries(s.failures).map(([k,v])=>`${failures[k]??k} ${v}`).join('、')}</li>)}</ul>
        <p>本批人工启动确认 {report.experimentApprovals} 次。工具没有提供的内部审批次数显示为未知。</p>
        <ul aria-label="静态兼容情况">{report.staticFit.results.map(r=><li key={r.harnessId}>{tools[r.harnessId]??r.harnessId} / {r.eligibility==='excluded'?'不符合必须条件':r.eligibility==='needs_verification'?'必须条件待核实':'资料支持待实测'} / {r.lowerPercent==null?'证据不足':`${r.lowerPercent.toFixed(0)}%–${r.upperPercent?.toFixed(0)}%`}</li>)}</ul>
        <p>目录 {report.staticFit.catalogVersion} / Provider {report.plan.provider}</p>
        <pre>{JSON.stringify({hardware:report.plan.hardware,planHash:report.planHash,configuration:report.plan.items},null,2)}</pre>
      </details>
      <div className="cloud-actions">
        <label>结果筛选<SelectMenu aria-label="实验结果筛选" value={filter} onChange={e=>setFilter(e.target.value)}><option value="all">全部任务</option><option value="failed">未通过或中断</option></SelectMenu></label>
        {(['json','csv'] as const).map(format=><button key={format} className="button secondary" disabled={busy||report.status==='running'} onClick={()=>void act(async()=>{if(await invoke<boolean>('export_harness_experiment',{workspaceId,experimentId:report.id,format}))notify('实验报告已保存');})}>导出 {format.toUpperCase()}</button>)}
      </div>
      <div className="fit-comparison" tabIndex={0} aria-label="逐题实验结果"><table>
        <thead><tr><th>任务</th><th>组合</th><th>结果</th><th>耗时</th><th>Token</th><th>执行记录</th></tr></thead>
        <tbody>{report.rows.filter(r=>filter==='all'||(!r.passed&&r.status!=='queued')).map(r=><tr key={r.id}>
          <th scope="row">{r.function} / v{r.datasetVersion} / 第 {r.repetition} 次</th><td>{tools[r.harness]} / {r.model}</td>
          <td>{states[r.status]??r.status}{r.scoreState?` / ${scoreNames[r.scoreState]??r.scoreState}`:''}{failures[r.failureCategory]?` / ${failures[r.failureCategory]}`:''}{r.buildStatus!=='not_applicable'&&<div>构建 {buildStatus[r.buildStatus]??'未知'}</div>}{r.error&&<div className="fit-status-copy">{r.error}</div>}</td>
          <td>{time(r.durationMs)}</td><td>{number(r.inputTokens)} / {number(r.outputTokens)}</td>
          <td><button className="button text-button" disabled={!r.recorded} onClick={()=>setTraceId(r.id)} >查看执行过程</button>{r.recorded&&!['running','grading'].includes(r.status)&&(!r.clean||!!r.pendingGraders?.length)&&<button className="button text-button" disabled={busy||!!listing?.active} onClick={()=>void act(async()=>{const tool=r.harness==='claude-code'?'claude':r.harness;await invoke(`cleanup_harness_${tool}_session`,{workspaceId,jobId:r.id});await load(report.id);})}>清理副本</button>}</td>
        </tr>)}</tbody>
      </table></div>
      {traceId&&<ExperimentTrace key={`${report.id}:${traceId}`} workspaceId={workspaceId} experimentId={report.id} runId={traceId} onClose={()=>setTraceId('')}/>}
    </>}
    {!report&&!busy&&<p>选择组合并运行后，可查看逐题结果、比较汇总和执行图。</p>}
    <AppDialog ref={dialog} className="panel-dialog form-dialog" aria-labelledby="harness-experiment-confirm"><header className="panel-header"><h2 id="harness-experiment-confirm">确认组合实验调用</h2><button className="icon-button" aria-label="关闭组合实验确认" onClick={()=>dialog.current?.close()}><X size={18}/></button></header>
      <div className="panel-body"><p>顺序运行 {runCount} 个任务，每个最多 4 次请求、4096 上下文、512 输出 Token，单次 120 秒、编程阶段 180 秒。可能占用显存，建议关闭其他占用显存的任务。</p>
        <ul>{resolved.map(({option,model},i)=><li key={i}>{option&&tools[option.harness]} / {option?.function} v{option?.datasetVersion} / {model?.name}<br/><code>{model?.digest}</code></li>)}</ul>
      </div><footer className="panel-footer dialog-actions"><button className="button secondary" onClick={()=>dialog.current?.close()}>返回</button><button className="button primary" disabled={!canStart} onClick={start}>确认调用并开始实验</button></footer>
    </AppDialog>
  </section>;
}

function ExperimentTrace({workspaceId,experimentId,runId,onClose}:{workspaceId:string;experimentId:string;runId:string;onClose:()=>void}) {
  const [trace,setTrace]=useState<Trace|null>(null);
  const [error,setError]=useState('');
  const [selected,setSelected]=useState<string|null>(null);
  const [canvas,setCanvas]=useState(false);
  const [failed,setFailed]=useState(false);
  const api=useRef<ViewportApi>(null);
  const dialog=useRef<HTMLDialogElement>(null);
  useEffect(()=>{
    let live=true;
    openDialog(dialog.current);
    void invoke<Trace>('get_harness_experiment_trace',{workspaceId,experimentId,runId}).then(data=>{if(live)setTrace(data);}).catch(value=>{if(live)setError(errorText(value));});
    return()=>{live=false;};
  },[workspaceId,experimentId,runId]);
  const graph=useMemo(()=>{
    const nodes=(trace?.events??[]).map((event,index)=>({id:String(index),title:eventTitle(event),kind:event.kind.includes('error')?'error':event.kind==='score'||event.kind==='candidate'?'output':event.kind==='model_response'?'context':'event'}));
    const edges=nodes.slice(1).map((node,i)=>({id:`edge-${i}`,fromNodeId:String(i),toNodeId:node.id,relation:'executed_before'}));
    const positions=Object.fromEntries(nodes.map((node,i)=>[node.id,[(i%6-2.5)*2.8,Math.floor(i/6)*-2.6,i%2*0.7] as Point]));
    return {nodes,edges,positions};
  },[trace]);
  const focus=(id:string)=>{setSelected(id);if(graph.positions[id])api.current?.focus(graph.positions[id]);};
  const event=selected===null?null:trace?.events[Number(selected)];
  return <AppDialog ref={dialog} className="panel-dialog fit-source-dialog harness-trace-dialog" aria-labelledby="experiment-trace-title" onClose={onClose}>
    <header className="panel-header"><h2 id="experiment-trace-title">任务执行过程</h2><button className="icon-button" aria-label="关闭任务执行过程" onClick={()=>dialog.current?.close()}><X size={18}/></button></header>
    <div className="panel-body">
      {error&&<p className="cloud-error" role="alert">{error}</p>}
      {!trace&&!error&&<p role="status">正在读取执行记录…</p>}
      {trace&&<>
        <div className="cloud-actions"><button className="button secondary" disabled={!graph.nodes.length||failed} onClick={()=>setCanvas(v=>!v)}>{canvas?'收起 3D 图谱':'打开 3D 图谱'}</button>{canvas&&!failed&&<button className="button text-button" onClick={()=>api.current?.fit()}>查看完整流程</button>}</div>
        {canvas&&!failed&&<div className="harness-trace-canvas"><Suspense fallback={<p>正在载入图谱…</p>}><Viewport label="实验任务 3D 执行图" nodes={graph.nodes} edges={graph.edges} positions={graph.positions} selected={selected} camera={[0,1,20]} target={[0,0,0]} apiRef={api} onReady={()=>api.current?.fit()} onSelect={focus} onChange={()=>{}} onFailure={()=>setFailed(true)}/></Suspense></div>}
        {failed&&<p>当前设备无法显示 3D 图谱，可从下方查看所有记录。</p>}
        <ol className="harness-trace-events" aria-label="实际执行事件">{graph.nodes.map(node=><li key={node.id}><button className="button text-button" aria-pressed={selected===node.id} onClick={()=>focus(node.id)}>{node.title}</button><time>{new Date(trace.events[Number(node.id)].createdAt).toLocaleTimeString()}</time></li>)}</ol>
        {event&&<pre aria-label="选中事件详情">{JSON.stringify(event.data,null,2)}</pre>}
      </>}
    </div>
  </AppDialog>;
}
