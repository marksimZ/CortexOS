import { expect,test } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir,readFile,writeFile } from 'node:fs/promises';
import { resolve,join } from 'node:path';
import { launchNative,nativeInvoke } from './native';
const exec=promisify(execFile);
type RecordItem={id:string;status:string;clean:boolean;modelCalls:number;taskExecution:string;error:string|null;plan:any;events:{kind:string;data:any}[]};
type Payload={datasets:{id:string;version:number;contentHash:string;function:string}[];history:{id:string}[];selected:RecordItem|null;active:{id:string}|null};

test('Claude gate keeps distinct scopes, honest readiness and recovery without inference',async({},info)=>{
 test.setTimeout(180000);
 const prepared=process.env.CORTEXOS_HARNESS_CLAUDE_PREPARED==='1';
 const compatible=process.env.CORTEXOS_CLAUDE_COMPATIBLE==='1';
 const source=resolve('../../artifacts/harness-codex-session-real-text2/harness-codex-session-one--469b9-ion-keeps-the-actual-result-harness-codex-session');
 const old=JSON.parse(await readFile(join(source,'actual-session.json'),'utf8'));
 const previous=resolve('../../artifacts/harness-claude-runtime-prepared/harness-claude-runtime-Cla-84ee2--recovery-without-inference-harness-claude-runtime');
 const legacy=compatible?JSON.parse(await readFile(join(previous,'runtime.json'),'utf8')):null;
 const data=info.outputPath('data');await mkdir(data,{recursive:true});
 await exec('py',['-3.13','-c','import sqlite3,sys\ns=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True)\nd=sqlite3.connect(sys.argv[2])\ns.backup(d)\nd.close()\ns.close()',join(compatible?previous:source,'data/cortexos.sqlite3'),join(data,'cortexos.sqlite3')],{windowsHide:true});
 let app=await launchNative(info,{dataDirectory:data});
 const scope={workspaceId:old.scope.workspaceId,assessmentVersion:1};
 const state=(jobId:string|null=null)=>nativeInvoke<Payload>(app.page,'get_harness_claude_runtime',{...scope,jobId});
 const readOld=()=>nativeInvoke(app.page,'get_harness_codex_session',{...old.scope,jobId:old.actual.id});
 const open=async()=>{await app.page.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},scope.workspaceId);await app.page.reload();await app.page.getByRole('button',{name:'4. 测试任务',exact:true}).click();await app.page.getByText('Claude Code 运行前检查',{exact:true}).click();};
 const wait=async(id:string)=>{await expect.poll(async()=>(await state(id)).selected?.status,{timeout:45000,intervals:[200,500]}).not.toBe('checking');return (await state(id)).selected!;};
 const reject=async(command:string,args:any)=>{let reason='';try{await nativeInvoke(app.page,command,args);}catch(e){reason=String(e);}expect(reason).not.toBe('');};
 const jobs:string[]=[];let evidence:any;
 try{
  await open();const initial=await state();const baseCount=compatible?3:0;expect(initial.history).toHaveLength(baseCount);expect((await readOld() as any).selected).toEqual(old.actual);
  if(legacy)expect((await state(legacy.completed.id)).selected).toEqual(legacy.completed);
  const task=initial.datasets.find(t=>t.version===2)!;expect(task.function).toBe('adjust');
  const images=await nativeInvoke<{images:{imageId:string}[]}>(app.page,'get_harness_claude_images',{workspaceId:scope.workspaceId});
  const imageId=prepared?JSON.parse(await readFile(resolve(compatible?'../../artifacts/harness-claude-sandbox/runtime.json':'../../artifacts/harness-claude-runtime/runtime.json'),'utf8')).imageId:old.actual.plan.imageId;
  const input={assessmentVersion:1,datasetId:task.id,version:task.version,datasetHash:task.contentHash,imageId,confirmed:true,...(compatible?{securityPolicy:'claude-bwrap-1'}:{})};
  await reject('check_harness_claude_runtime',{workspaceId:scope.workspaceId,input:{...input,confirmed:false}});
  await reject('check_harness_claude_runtime',{workspaceId:scope.workspaceId,input:{...input,datasetHash:'forged'}});
  await reject('check_harness_claude_runtime',{workspaceId:scope.workspaceId,input:{...input,securityPolicy:'codex-bwrap-1'}});
  await reject('get_harness_claude_runtime',{...scope,jobId:old.actual.plan.runtimeId});
  await reject('cleanup_harness_claude_runtime',{workspaceId:scope.workspaceId,jobId:old.actual.plan.runtimeId});
  const foreign=(await nativeInvoke<{id:string}>(app.page,'create_workspace',{name:'Claude scope denial'})).id;
  await reject('check_harness_claude_runtime',{workspaceId:foreign,input});
  expect((await state()).history).toHaveLength(baseCount);
  if(!prepared){
   expect(images.images).toEqual([]);await reject('check_harness_claude_runtime',{workspaceId:scope.workspaceId,input});
   await expect(app.page.getByRole('button',{name:'检查程序与隔离条件',exact:true})).toBeDisabled();await expect(app.page.getByText('没有找到已准备的 Claude Code 专用镜像。工具准备完成后更新列表。',{exact:true})).toBeVisible();
   evidence={scope,mode:'unprepared',modelCalls:0,claudeExecution:'Not Run',oldCodexUnchanged:true};
  }else{
   expect(images.images.some(i=>i.imageId===imageId)).toBe(true);
   await app.page.getByRole('combobox',{name:'Claude检查任务版本',exact:true}).selectOption(`${task.id}:${task.version}`);await app.page.getByRole('combobox',{name:'Claude检查镜像',exact:true}).selectOption(imageId);
   if(compatible)await app.page.getByRole('combobox',{name:'Claude保护策略',exact:true}).selectOption('claude-bwrap-1');
   await app.page.getByRole('button',{name:'检查程序与隔离条件',exact:true}).click();const dialog=app.page.getByRole('dialog',{name:'检查 Claude Code 程序',exact:true});await expect(dialog).toBeVisible();await dialog.getByRole('button',{name:'确认检查，不调用模型',exact:true}).click();
   await expect.poll(async()=>(await state()).history.length,{timeout:15000}).toBe(baseCount+1);const id=(await state()).selected!.id;jobs.push(id);const completed=await wait(id);
   expect(completed.status,completed.error??'see runtime.json').toBe('completed');expect(completed.clean).toBe(true);expect(completed.modelCalls).toBe(0);expect(completed.taskExecution).toBe('Not Run');
   const observed=completed.events.find(e=>e.kind==='probe')!.data;expect(observed.canRun).toBe(false);expect(observed.sandboxReady).toBe(compatible);expect(observed.raw.cliSandboxExecution).toBe(compatible?'Trusted offline fixture':'Not Run');expect(observed.checks.find((c:any)=>c.id==='sandbox').state).toBe(compatible?'passed':'not_checked');expect(observed.verifiedConfiguration.hostConfig.NetworkMode).toBe('none');expect(observed.verifiedConfiguration.hostConfig.SecurityOpt.some((s:string)=>s.startsWith('seccomp='))).toBe(compatible);
   if(compatible){expect(completed.plan.version).toBe('claude-runtime-check-2');expect(completed.plan.securityPolicy.id).toBe('claude-bwrap-1');expect(observed.raw.sandbox.mode).toBe('trusted_offline_fixture_not_model');expect(observed.raw.sandbox.proof.checks.nestedUserNamespaceDenied).toBe(true);expect(Object.values(observed.raw.sandbox.proof.checks)).toEqual(Array(13).fill(true));expect(observed.raw.sandbox.settings.sandbox.allowUnsandboxedCommands).toBe(false);expect(observed.raw.sandbox.requests).toHaveLength(2);}
   await reject('get_harness_codex_runtime',{...scope,checkId:old.scope.checkId,jobId:id});await reject('cleanup_harness_codex_runtime',{workspaceId:scope.workspaceId,jobId:id});await reject('get_harness_claude_runtime',{workspaceId:foreign,assessmentVersion:1,jobId:id});
   const stoppedId=(await nativeInvoke<{id:string}>(app.page,'check_harness_claude_runtime',{workspaceId:scope.workspaceId,input})).id;jobs.push(stoppedId);await nativeInvoke(app.page,'cleanup_harness_claude_runtime',{workspaceId:scope.workspaceId,jobId:stoppedId});const stopped=await wait(stoppedId);expect(stopped.status).toBe('cancelled');expect(stopped.clean).toBe(true);
   const recoveryId=(await nativeInvoke<{id:string}>(app.page,'check_harness_claude_runtime',{workspaceId:scope.workspaceId,input})).id;jobs.push(recoveryId);const pending=(await state(recoveryId)).selected!;
   await expect.poll(async()=>{try{await exec('docker',['--host',pending.plan.environment.endpoint,'pause',pending.plan.containerName],{windowsHide:true,timeout:5000});return true;}catch{return false;}},{timeout:7000,intervals:[30,70]}).toBe(true);
   await exec('taskkill',['/PID',String(app.processId),'/T','/F'],{windowsHide:true});await app.stop();app=await launchNative(info,{dataDirectory:data,profileDirectory:info.outputPath('recovery-profile')});
   const interrupted=(await state(recoveryId)).selected!;expect(interrupted.status).toBe('interrupted');expect(interrupted.clean).toBe(false);await nativeInvoke(app.page,'cleanup_harness_claude_runtime',{workspaceId:scope.workspaceId,jobId:recoveryId});const recovered=(await state(recoveryId)).selected!;expect(recovered.status).toBe('closed');expect(recovered.clean).toBe(true);
   expect((await state(id)).selected).toEqual(completed);expect((await readOld() as any).selected).toEqual(old.actual);if(legacy)expect((await state(legacy.completed.id)).selected).toEqual(legacy.completed);
   await open();await app.page.getByRole('combobox',{name:'Claude运行前检查历史',exact:true}).selectOption(id);
   evidence={scope,mode:compatible?'trusted_sandbox_fixture_without_model':'actual_cli_without_model',completed,stopped,interrupted,recovered,modelCalls:0,taskExecution:'Not Run',oldCodexUnchanged:true,oldClaudeUnchanged:!!legacy};
  }
  for(const theme of ['light','dark']){await app.page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);await expect(app.page.locator('html')).toHaveAttribute('data-color-scheme',theme);for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await app.page.setViewportSize({width,height});await app.page.locator('.fit-scroll').evaluate(e=>{e.scrollTop+=e.querySelector('.claude-runtime')!.getBoundingClientRect().top-e.getBoundingClientRect().top-12;});expect(await app.page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);await app.page.screenshot({path:info.outputPath(`claude-${theme}-${width}.png`)});}}
  expect(app.consoleErrors).toEqual([]);expect((await readOld() as any).selected).toEqual(old.actual);await writeFile(info.outputPath('runtime.json'),JSON.stringify(evidence,null,2));
 }finally{for(const jobId of jobs)try{await nativeInvoke(app.page,'cleanup_harness_claude_runtime',{workspaceId:scope.workspaceId,jobId});}catch{}try{await app.closeFromUi();}catch{}await app.stop();}
});
