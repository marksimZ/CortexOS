import { useEffect,useRef,useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { AppDialog,openDialog } from '../../components/AppDialog';
import { SelectMenu } from '../../components/SelectMenu';
import { FitStatus } from './FitStatus';
import { errorText } from '../personalized';
import { CodexSession } from './CodexSession';

interface Observation {checks:{id:string;state:'passed'|'blocked';reason?:string}[];cliVersion:string;sandboxReady:boolean;canRun:false}
interface RecordItem {id:string;planHash:string;status:string;clean:boolean;error:string|null;plan:{imageId:string;checkId:string;datasetVersion:number;securityPolicy?:{id:string;hash:string|null}};events:{kind:string;data:Observation}[]}
interface Payload {history:{id:string;status:string;createdAt:string}[];selected:RecordItem|null;active:{id:string;checkId:string;status:string}|null}
const labels:Record<string,string>={binary:'程序指纹',version:'Linux 程序版本',automation:'自动化参数',sandbox:'目录权限保护'};
const states:Record<string,string>={checking:'正在检查',completed:'检查完成',failed:'检查出错',cancelled:'已停止',interrupted:'检查中断',cleanup_required:'待清理',closed:'已清理'};
const policyName=(id?:string)=>id==='codex-bwrap-1'?'Codex 兼容策略 1':id==='docker-default'?'Docker 默认':'Docker 默认（旧记录）';
const reasons:Record<string,string>={docker_unavailable:'Docker 不可用，请先启动 Docker Desktop。',codex_runtime_image_mismatch:'镜像版本或类型不符合本次检查要求。',namespace_unavailable:'当前 Docker 权限不允许 Codex 创建隔离目录所需的环境，自动编程保持关闭。',wall_timeout:'环境检查超时，已停止本次检查。',output_limit:'程序输出超出上限，已停止本次检查。',cancelled:'本次检查已停止。',container_cleanup_failed:'容器清理失败，请重新清理。'};
export function CodexRuntime({workspaceId,assessmentVersion,checkId,checkHash,disabled}:{workspaceId:string;assessmentVersion:number;checkId:string;checkHash:string;disabled:boolean}){
 const [data,setData]=useState<Payload|null>(null),[images,setImages]=useState<{imageId:string;version:string}[]>([]),[image,setImage]=useState(''),[securityPolicy,setSecurityPolicy]=useState('docker-default'),[busy,setBusy]=useState(false),[error,setError]=useState(''),[stopping,setStopping]=useState(false);
 const alive=useRef(true),epoch=useRef(0),selected=useRef<string|null>(null),dialog=useRef<HTMLDialogElement>(null);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;epoch.current++;};},[]);
 const read=async(id:string|null=selected.current)=>{
  const ticket=++epoch.current;
  const next=await invoke<Payload>('get_harness_codex_runtime',{workspaceId,assessmentVersion,checkId,jobId:id});
  if(alive.current&&ticket===epoch.current){selected.current=next.selected?.id??null;setData(next);if(next.selected?.status!=='checking')setStopping(false);}
 };
 const act=async(action:()=>Promise<void>)=>{setBusy(true);setError('');try{await action();}catch(e){if(alive.current)setError(reasons[String(e)]??errorText(e));}finally{if(alive.current)setBusy(false);}};
 const load=async()=>{
  const results=await Promise.allSettled([read(),invoke<{images:typeof images}>('get_harness_codex_images',{workspaceId})]);
  if(!alive.current)return;
  if(results[1].status==='fulfilled'){const list=results[1].value.images;setImages(list);setImage(old=>list.some(i=>i.imageId===old)?old:list[0]?.imageId??'');}
  const failed=results.find(r=>r.status==='rejected');if(failed?.status==='rejected')throw failed.reason;
 };
 const record=data?.selected,working=record?.status==='checking';
 useEffect(()=>{
  if(!working)return;let disposed=false;let timer:ReturnType<typeof setTimeout>;
  const poll=async()=>{try{await read();}catch(e){if(!disposed&&alive.current)setError(errorText(e));}if(!disposed)timer=setTimeout(()=>void poll(),700);};
  timer=setTimeout(()=>void poll(),700);return()=>{disposed=true;clearTimeout(timer);};
 },[working,workspaceId,assessmentVersion,checkId]);
 const observation=record?.events.find(e=>e.kind==='probe')?.data;
 const check=()=>act(async()=>{
  dialog.current?.close();
  const next=await invoke<{id:string}>('check_harness_codex_runtime',{workspaceId,input:{assessmentVersion,checkId,checkHash,imageId:image,securityPolicy,confirmed:true}});
  if(alive.current)await read(next.id);
 });
 const clean=(id:string)=>act(async()=>{const reply=await invoke<{stopping?:boolean}>('cleanup_harness_codex_runtime',{workspaceId,jobId:id});if(alive.current){setStopping(!!reply.stopping);await read();}});
 return <><details className="task-setup codex-runtime" onToggle={e=>{if(e.currentTarget.open&&!data&&!busy)void act(load);}}>
  <summary>Codex 隔离检查</summary>
  <section aria-label="Codex隔离检查" aria-busy={busy||working}>
   <div className="cloud-actions"><label>本机专用镜像<SelectMenu aria-label="Codex隔离镜像" value={image} disabled={busy||working||disabled} onChange={e=>setImage(e.target.value)}><option value="">选择已准备的镜像</option>{images.map(i=><option key={i.imageId} value={i.imageId}>Codex {i.version} / {i.imageId.slice(7,19)}</option>)}</SelectMenu></label><label>保护策略<SelectMenu aria-label="Codex保护策略" value={securityPolicy} disabled={busy||working||disabled} onChange={e=>setSecurityPolicy(e.target.value)}><option value="docker-default">Docker 默认</option><option value="codex-bwrap-1">Codex 兼容策略 1</option></SelectMenu></label><button className="button secondary" disabled={busy||working||disabled} onClick={()=>void act(load)}>更新镜像与记录</button><button className="button primary" disabled={!image||busy||working||disabled||!!data?.active} onClick={()=>openDialog(dialog.current)}>检查隔离环境</button>{working?<button className="button secondary" disabled={busy||stopping} onClick={()=>void clean(record.id)}>停止并清理检查</button>:record&&!record.clean&&<button className="button secondary" disabled={busy||disabled} onClick={()=>void clean(record.id)}>清理检查容器</button>}</div>
   {error&&<p className="cloud-error" role="alert">{error}</p>}
   {data?.active&&data.active.id!==record?.id&&<div className="fit-meta-line"><span>此工作区还有未结束的检查。</span>{data.active.checkId===checkId?<button className="button secondary" disabled={busy} onClick={()=>void act(()=>read(data.active!.id))}>查看未结束检查</button>:<button className="button secondary" disabled={busy||disabled} onClick={()=>void clean(data.active!.id)}>清理未完成检查</button>}</div>}
   {!!data?.history.length&&<label className="profile-field">隔离检查历史<SelectMenu aria-label="Codex隔离检查历史" value={record?.id??''} disabled={busy||working} onChange={e=>void act(()=>read(e.target.value))}>{data.history.map(r=><option key={r.id} value={r.id}>{new Date(r.createdAt).toLocaleString()} / {states[r.status]??r.status}</option>)}</SelectMenu></label>}
   <p role="status">{stopping?'正在停止并清理…':busy?'正在读取或准备检查…':working?'正在检查程序及目录权限，没有调用模型…':observation?observation.sandboxReady?'本次隔离检查已通过。':'目录权限保护未就绪，自动编程仍不可用。':record?states[record.status]??record.status:'先检查隔离环境，再配置模型调用。'}</p>
   {data&&images.length===0&&!busy&&<p>没有找到已准备的 Codex 专用镜像。准备完成后更新列表。</p>}
   {record&&<>
    <div className="fit-meta-line"><span>本次记录：{policyName(record.plan.securityPolicy?.id)}</span><FitStatus tone="neutral">检查不调用模型</FitStatus><FitStatus tone="neutral">检查不执行编程任务</FitStatus><FitStatus tone={record.clean?'success':'attention'}>{record.clean?'检查容器已清理':'检查容器待清理'}</FitStatus></div>
    {record.error&&<p className="cloud-error">{reasons[record.error]??record.error}</p>}
    {observation&&<ul className="preflight-checks">{observation.checks.map(c=><li key={c.id}><div><strong>{labels[c.id]??c.id}</strong><FitStatus tone={c.state==='passed'?'success':'attention'}>{c.state==='passed'?'已核对':'未就绪'}</FitStatus></div>{c.id==='version'&&c.state==='passed'&&<p>{observation.cliVersion}</p>}{c.id==='sandbox'&&c.state==='blocked'&&<p>{reasons[c.reason??'']??'目录权限保护尚未通过检查；查看记录后处理环境问题。'}</p>}</li>)}</ul>}
    <details className="personal-details"><summary>检查范围与完整记录</summary><p>仅执行固定的版本、参数及临时目录权限检查。检查包不包含项目代码、参考答案或测试，不读取个人登录。镜像保持无网络、非管理员、只读系统目录和原有资源上限；检查通过也不会自动运行任务或模型。</p><pre>{JSON.stringify(record,null,2)}</pre></details>
   </>}
  </section>
  <AppDialog ref={dialog} className="panel-dialog form-dialog" aria-labelledby="codex-runtime-title"><header className="panel-header"><h2 id="codex-runtime-title">检查 Codex 隔离环境</h2><button className="icon-button" aria-label="关闭隔离检查确认" onClick={()=>dialog.current?.close()}><X size={18}/></button></header><div className="panel-body"><p>本次只在临时容器中检查 Codex 程序和目录写入限制，不调用模型、不执行项目代码。完成后清理本次容器并保留检查记录。</p><p>所选策略：{policyName(securityPolicy)}</p>{securityPolicy==='codex-bwrap-1'&&<p>仅本次 Codex 容器额外允许六类内部隔离所需的系统调用，会增加内核接口暴露面。继续保留非管理员、无网络、无宿主挂载及资源限额，不改变系统设置或其他功能。</p>}<p>Codex {images.find(i=>i.imageId===image)?.version} / 当前任务版本 {record?.plan.datasetVersion??'以所选任务检查为准'}</p></div><footer className="panel-footer dialog-actions"><button className="button secondary" onClick={()=>dialog.current?.close()}>返回</button><button className="button primary" disabled={busy||!image||!!data?.active} onClick={()=>void check()}>确认检查，不调用模型</button></footer></AppDialog>
 </details>{record&&<CodexSession key={record.id} workspaceId={workspaceId} assessmentVersion={assessmentVersion} checkId={checkId} runtimeId={record.id} runtimeHash={record.planHash} ready={!!observation?.sandboxReady&&record.clean&&record.status==='completed'&&record.plan.securityPolicy?.id==='codex-bwrap-1'} disabled={disabled||busy||working}/>}</>;
}
