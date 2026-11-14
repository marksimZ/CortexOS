import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { AppDialog, openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { AiderSession, ClaudeSession } from './CodexSession';
import { FitStatus } from './FitStatus';
import { errorText } from '../personalized';

interface Task {id:string;version:number;contentHash:string;function:string|null}
interface Observation {checks:{id:string;state:'passed'|'blocked'|'not_checked';reason?:string}[];raw:{dependencies?:Record<string,string|null>};definition:{version:string;probeVersion:string};canRun:false;sandboxReady?:boolean}
interface RuntimeRecord {id:string;planHash:string;status:string;clean:boolean;error:string|null;modelCalls:0;plan:{version:string;datasetId:string;datasetVersion:number;function:string;imageId:string;securityPolicy:{id:string}};events:{kind:string;data:Observation}[]}
interface Payload {datasets:Task[];history:{id:string;status:string;createdAt:string}[];selected:RuntimeRecord|null;active:{id:string;assessmentVersion:number;status:string}|null}
const status:Record<string,string>={checking:'正在检查',completed:'检查完成',cancelled:'已停止',failed:'检查出错',interrupted:'检查中断',cleanup_required:'待清理',closed:'已清理'};
const labels:Record<string,string>={binary:'程序指纹',version:'程序版本',automation:'自动化参数',dependencies:'沙盒依赖',namespace:'隔离权限',sandbox:'内层沙盒实测',model_channel:'本地模型通道',container:'容器隔离',inner_sandbox:'工具自身隔离'};
const reasons:Record<string,string>={docker_unavailable:'Docker 不可用，请先启动 Docker Desktop。',docker_environment_error:'无法连接本地 Docker 引擎。',claude_runtime_image_mismatch:'所选镜像的 Claude Code 版本或类型不符合本次检查。',claude_sandbox_dependencies_image_required:'请先选择含沙盒依赖的 Claude Code 镜像。',wall_timeout:'检查超时，已停止并清理。',output_limit:'程序输出超出检查上限，已停止。',claude_probe_failed:'检查程序未正常退出，查看完整记录后处理。',aider_runtime_image_mismatch:'镜像的 Aider 版本或 Python 环境不匹配。',aider_probe_failed:'Aider 检查未正常退出，请查看原始记录。'};

interface RuntimeProps {workspaceId:string;assessmentVersion:number;disabled:boolean}
export function ClaudeRuntime(props:RuntimeProps){return <ToolRuntime {...props} tool="claude"/>;}
export function AiderRuntime(props:RuntimeProps){return <ToolRuntime {...props} tool="aider"/>;}
function ToolRuntime({workspaceId,assessmentVersion,disabled,tool}:RuntimeProps&{tool:'claude'|'aider'}) {
 const label=tool==='claude'?'Claude':'Aider',name=tool==='claude'?'Claude Code':'Aider';
 const [data,setData]=useState<Payload|null>(null),[task,setTask]=useState(''),[images,setImages]=useState<{imageId:string;version:string;dependencies?:boolean}[]>([]),[image,setImage]=useState(''),[policy,setPolicy]=useState('docker-default'),[busy,setBusy]=useState(false),[error,setError]=useState(''),[stopping,setStopping]=useState(false);
 const alive=useRef(true),epoch=useRef(0),selected=useRef<string|null>(null),dialog=useRef<HTMLDialogElement>(null);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;epoch.current++;};},[]);
 const read=async(id:string|null=selected.current)=>{
  const ticket=++epoch.current;
  const next=await invoke<Payload>(`get_harness_${tool}_runtime`,{workspaceId,assessmentVersion,jobId:id});
  if(!alive.current||ticket!==epoch.current)return;
  selected.current=next.selected?.id??null;setData(next);
  setTask(old=>next.selected?`${next.selected.plan.datasetId}:${next.selected.plan.datasetVersion}`:next.datasets.some(t=>`${t.id}:${t.version}`===old)?old:next.datasets[0]?`${next.datasets[0].id}:${next.datasets[0].version}`:'');
  if(next.selected?.status!=='checking')setStopping(false);
 };
 const act=async(action:()=>Promise<void>)=>{setBusy(true);setError('');try{await action();}catch(e){if(alive.current)setError(reasons[String(e)]??errorText(e));}finally{if(alive.current)setBusy(false);}};
 const load=async()=>{
  const results=await Promise.allSettled([read(),invoke<{images:typeof images}>(`get_harness_${tool}_images`,{workspaceId})]);
  if(!alive.current)return;
  if(results[1].status==='fulfilled'){const list=results[1].value.images;setImages(list);setImage(old=>list.some(i=>i.imageId===old)?old:list[0]?.imageId??'');}
  const failed=results.find(r=>r.status==='rejected');if(failed?.status==='rejected')throw failed.reason;
 };
 const record=data?.selected,working=record?.status==='checking',chosen=data?.datasets.find(t=>`${t.id}:${t.version}`===task);
 const observation=record?.events.find(e=>e.kind==='probe')?.data;
 const missing=observation?.raw.dependencies?Object.entries(observation.raw.dependencies).filter(([,value])=>!value).map(([name])=>name):null;
 useEffect(()=>{if(!working)return;let disposed=false;let timer:ReturnType<typeof setTimeout>;const poll=async()=>{try{await read();}catch(e){if(!disposed&&alive.current)setError(errorText(e));}if(!disposed)timer=setTimeout(()=>void poll(),700);};timer=setTimeout(()=>void poll(),700);return()=>{disposed=true;clearTimeout(timer);};},[working,workspaceId,assessmentVersion]);
 const check=()=>act(async()=>{dialog.current?.close();if(!chosen)return;const result=await invoke<{id:string}>(`check_harness_${tool}_runtime`,{workspaceId,input:{assessmentVersion,datasetId:chosen.id,version:chosen.version,datasetHash:chosen.contentHash,imageId:image,...(tool==='claude'?{securityPolicy:policy}:{}),confirmed:true}});if(alive.current)await read(result.id);});
 const clean=(id:string)=>act(async()=>{const result=await invoke<{stopping?:boolean}>(`cleanup_harness_${tool}_runtime`,{workspaceId,jobId:id});if(alive.current){setStopping(!!result.stopping);await read();}});
 return <details className={`task-setup ${tool}-runtime`} onToggle={e=>{if(e.currentTarget.open&&!data&&!busy)void act(load);}}>
  <summary>{name} 运行前检查</summary>
  <section aria-label={`${name}运行前检查`} aria-busy={busy||working}>
   <div className="cloud-actions">
    <label>已发布编程任务<SelectMenu aria-label={`${label}检查任务版本`} value={task} disabled={busy||working||disabled} onChange={e=>setTask(e.target.value)}><option value="">选择已发布版本</option>{data?.datasets.map(t=><option key={`${t.id}:${t.version}`} value={`${t.id}:${t.version}`}>{t.function??t.id.slice(-8)} / 版本 {t.version}</option>)}</SelectMenu></label>
    <label>专用镜像<SelectMenu aria-label={`${label}检查镜像`} value={image} disabled={busy||working||disabled} onChange={e=>{setImage(e.target.value);setPolicy('docker-default');}}><option value="">选择已准备的镜像</option>{images.map(i=><option key={i.imageId} value={i.imageId}>{name} {i.version} / {tool==='aider'?'Python 3.12':i.dependencies?'含沙盒依赖':'基础镜像'} / {i.imageId.slice(7,19)}</option>)}</SelectMenu></label>
    {tool==='claude'&&<label>保护策略<SelectMenu aria-label={`${label}保护策略`} value={policy} disabled={busy||working||disabled} onChange={e=>setPolicy(e.target.value)}><option value="docker-default">Docker 默认</option><option value="claude-bwrap-1" disabled={!images.find(i=>i.imageId===image)?.dependencies}>Claude 沙盒兼容</option></SelectMenu></label>}
    <button className="button secondary" disabled={busy||working} onClick={()=>void act(load)}>更新任务与镜像</button><button className="button primary" disabled={!chosen||!image||busy||working||disabled||!!data?.active} onClick={()=>openDialog(dialog.current)}>检查程序与隔离条件</button>{working?<button className="button secondary" disabled={busy||stopping} onClick={()=>void clean(record.id)}>停止并清理本次检查</button>:record&&!record.clean&&<button className="button secondary" disabled={busy||disabled} onClick={()=>void clean(record.id)}>清理本次检查容器</button>}
   </div>
   {error&&<p className="cloud-error" role="alert">{error}</p>}
   {data?.datasets.length===0&&<p>当前分析还没有已发布的函数任务。先完成草案验证和人工发布。</p>}
   {data&&images.length===0&&!busy&&<p>没有找到已准备的 {name} 专用镜像。工具准备完成后更新列表。</p>}
   {data?.active&&data.active.id!==record?.id&&<div className="fit-meta-line"><span>此工作区还有未结束的检查。</span>{data.active.assessmentVersion===assessmentVersion?<button className="button secondary" disabled={busy} onClick={()=>void act(()=>read(data.active!.id))}>查看未结束检查</button>:<button className="button secondary" disabled={busy||disabled} onClick={()=>void clean(data.active!.id)}>停止或清理原检查</button>}</div>}
   {!!data?.history.length&&<label className="profile-field">检查历史<SelectMenu aria-label={`${label}运行前检查历史`} disabled={busy||working} value={record?.id??''} onChange={e=>void act(()=>read(e.target.value))}>{data.history.map(h=><option key={h.id} value={h.id}>{new Date(h.createdAt).toLocaleString()} / {status[h.status]??h.status}</option>)}</SelectMenu></label>}
   <p role="status">{stopping?'正在停止并清理…':busy?'正在读取或准备检查…':working?'正在核对程序、参数及隔离条件，不调用模型…':observation?'检查记录已保存。':record?status[record.status]??record.status:'只检查程序和安全条件，不会开始编程或模型调用。'}</p>
   {record&&<><div className="fit-meta-line"><strong>{record.plan.function} / 任务版本 {record.plan.datasetVersion}</strong><FitStatus tone="neutral">{record.plan.securityPolicy.id==='claude-bwrap-1'?'Claude 沙盒兼容':'Docker 默认'}</FitStatus>{record.plan.version==='claude-runtime-check-1'&&<FitStatus tone="neutral">旧版检查</FitStatus>}<FitStatus tone="neutral">模型未调用</FitStatus><FitStatus tone="neutral">任务未运行</FitStatus><FitStatus tone={record.clean?'success':'attention'}>{record.clean?'检查容器已清理':'检查容器待清理'}</FitStatus></div>
    {record.error&&<p className="cloud-error">{reasons[record.error]??record.error}</p>}
    {observation&&<ul className="preflight-checks">{observation.checks.map(c=><li key={c.id}><div><strong>{labels[c.id]??c.id}</strong><FitStatus tone={c.state==='passed'?'success':c.state==='blocked'?'attention':'neutral'}>{c.state==='passed'?'已核对':c.state==='blocked'?'未就绪':'未验证'}</FitStatus></div>{c.id==='version'&&c.state==='passed'&&<p>{observation.definition.version}</p>}{c.id==='automation'&&observation.definition.probeVersion==='claude-program-2'&&c.state==='passed'&&<p>预算参数已识别；实际模型回合限制尚未验证。</p>}{c.id==='dependencies'&&<p>{missing?missing.length?`缺少 ${missing.join('、')}；本轮没有自动安装。`:'已发现依赖；这不能代替内层沙盒实测。':'没有获得依赖检查结果，请查看原始记录。'}</p>}{c.id==='namespace'&&<p>{c.state==='passed'?'允许创建内部隔离环境，仍需实际沙盒验证。':'当前策略下无法创建内部隔离环境；未修改系统设置。'}</p>}{c.id==='sandbox'&&<p>{c.state==='passed'?'固定回复驱动的真实命令检查通过；没有调用模型或运行项目任务。':c.state==='blocked'?'内层隔离检查未通过，完整原因保留在原始记录。':'尚未运行 Claude 内层沙盒验证。'}</p>}{c.id==='model_channel'&&<p>{tool==='claude'?'这份检查不调用模型；实际通道结果保存在下方单题执行记录中。':'运行前检查的历史结果。实际请求见单题执行记录。'}</p>}{c.id==='container'&&<p>检查非管理员、只读根目录、权限限制、网络不可达及无 Docker Socket。</p>}{c.id==='inner_sandbox'&&<p>尚无工具内部沙盒的验证依据；容器隔离结果单独记录。</p>}</li>)}</ul>}
    <details className="personal-details"><summary>固定版本、检查范围与原始记录</summary><p>当前记录保存当时的版本、镜像和策略。{tool==='claude'?'兼容检查用固定回复驱动真实 Claude Bash 工具，只运行可信检查代码；不读取项目代码、标准答案、评分断言或个人登录。固定回复的 Token 和费用字段是夹具数据，不计入模型成绩。':'本片仅运行实际版本、帮助与可信容器探针，没有模型地址、密钥或项目代码；没有内层沙盒或模型成绩的通过结论。'}</p><pre>{JSON.stringify(record,null,2)}</pre></details>
   </>}
   {tool==='claude'&&<ClaudeSession key={`${workspaceId}:${assessmentVersion}`} workspaceId={workspaceId} assessmentVersion={assessmentVersion} runtimeId={record?.id??''} runtimeHash={record?.planHash??''} ready={record?.status==='completed'&&record.clean&&record.plan.securityPolicy.id==='claude-bwrap-1'&&observation?.sandboxReady===true} disabled={disabled||busy||!!working}/>}
   {tool==='aider'&&<AiderSession key={`${workspaceId}:${assessmentVersion}`} workspaceId={workspaceId} assessmentVersion={assessmentVersion} runtimeId={record?.id??''} runtimeHash={record?.planHash??''} ready={record?.status==='completed'&&record.clean&&['version','automation','container'].every(id=>observation?.checks.some(check=>check.id===id&&check.state==='passed'))} disabled={disabled||busy||!!working}/>}
  </section>
  <AppDialog ref={dialog} className="panel-dialog form-dialog" aria-labelledby={`${tool}-check-title`}><header className="panel-header"><h2 id={`${tool}-check-title`}>检查 {name} 程序</h2><button className="icon-button" aria-label={`关闭${label}检查确认`} onClick={()=>dialog.current?.close()}><X size={18}/></button></header><div className="panel-body"><p>本次仅在无网络的临时容器内检查程序版本、自动化参数和隔离条件，不调用模型，不运行项目代码，不读取个人登录。</p><p>{chosen?.function} / 任务版本 {chosen?.version} / {name} {images.find(i=>i.imageId===image)?.version}</p>{policy==='claude-bwrap-1'&&<p className="cloud-error">Claude 沙盒兼容会额外允许受限的 unshare、clone 及 mount、umount2、pivot_root、chroot，增加内核接口暴露面；保留非 root、只读、无网络和资源限制。使用固定回复驱动可信命令，禁止未隔离重试。</p>}<p>缺少依赖或权限时保留具体原因。完成后清理本次容器，检查记录继续保留。</p></div><footer className="panel-footer dialog-actions"><button className="button secondary" onClick={()=>dialog.current?.close()}>返回</button><button className="button primary" disabled={busy||!chosen||!image||disabled||!!data?.active} onClick={()=>void check()}>确认检查，不调用模型</button></footer></AppDialog>
 </details>;
}
