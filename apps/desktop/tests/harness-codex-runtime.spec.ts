import { expect,test } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir,readFile,writeFile } from 'node:fs/promises';
import { resolve,join } from 'node:path';
import { launchNative,nativeInvoke } from './native';
const exec=promisify(execFile);
interface Check {id:string;planHash:string;status:string;clean:boolean;error:string|null;plan:{containerName:string;imageId:string;environment:{endpoint:string}};events:{kind:string;data:Record<string,any>}[];modelCalls:number;taskExecution:string}
interface Payload {selected:Check|null;history:{id:string;status:string}[];active:{id:string}|null}

test('Linux Codex gate verifies the selected policy, history and scoped cleanup without models',async({},info)=>{
 test.setTimeout(180000);
 const source=resolve('../../artifacts/harness-preflight-fixed/harness-preflight-Codex-pr-ea7ea-ions-without-starting-tasks-harness-preflight');
 const fixture=JSON.parse(await readFile(join(source,'preflight.json'),'utf8'));
 const compatible=process.env.CORTEXOS_CODEX_COMPATIBLE==='1';
 const legacySource=resolve('../../artifacts/harness-codex-runtime-recovery/harness-codex-runtime-Linu-a2957-oped-cleanup-without-models-harness-codex-runtime');
 const legacy=compatible?JSON.parse(await readFile(join(legacySource,'runtime.json'),'utf8')):null;
 const data=info.outputPath('data');await mkdir(data,{recursive:true});
 await exec('py',['-3','-c','import sqlite3,sys\ns=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True)\nd=sqlite3.connect(sys.argv[2])\ns.backup(d)\nd.close()\ns.close()',join(compatible?legacySource:source,'data/cortexos.sqlite3'),join(data,'cortexos.sqlite3')],{windowsHide:true});
 let app=await launchNative(info,{dataDirectory:data});
 const scope={workspaceId:fixture.workspaceId,assessmentVersion:1,checkId:fixture.records.selected.id};
 const state=(jobId:string|null=null)=>nativeInvoke<Payload>(app.page,'get_harness_codex_runtime',{...scope,jobId});
 const open=async()=>{await app.page.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},scope.workspaceId);await app.page.reload();await app.page.getByRole('button',{name:'4. 测试任务',exact:true}).click();await app.page.getByText('Codex 执行前检查',{exact:true}).click();await expect(app.page.getByRole('combobox',{name:'执行检查历史',exact:true})).toBeVisible();await app.page.getByRole('combobox',{name:'执行检查历史',exact:true}).selectOption(scope.checkId);await app.page.getByText('Codex 隔离检查',{exact:true}).click();};
 const wait=async(id:string)=>{await expect.poll(async()=>(await state(id)).selected?.status,{timeout:60000,intervals:[200,500]}).not.toBe('checking');return (await state(id)).selected!;};
 const jobs:string[]=[];
 try{
  await open();await expect(app.page.getByRole('button',{name:'检查隔离环境',exact:true})).toBeEnabled({timeout:20000});
  const listing=await nativeInvoke<{images:{imageId:string;version:string}[]}>(app.page,'get_harness_codex_images',{workspaceId:scope.workspaceId});expect(listing.images.length).toBeGreaterThan(0);
  const manifest=JSON.parse(await readFile(resolve('../../artifacts/harness-codex-runtime/runtime.json'),'utf8'));
  const imageId=manifest.imageId;expect(listing.images.some(i=>i.imageId===imageId)).toBe(true);
  const input={assessmentVersion:1,checkId:scope.checkId,checkHash:fixture.records.selected.contentHash,imageId,confirmed:true,securityPolicy:compatible?'codex-bwrap-1':'docker-default'};
  const initial=(await state()).history.length;
  if(legacy)expect((await state(legacy.completed.id)).selected).toEqual(legacy.completed);
  const other=(await nativeInvoke<{id:string}>(app.page,'create_workspace',{name:'Codex隔离边界'})).id;
  const rejected=async(command:string,args:Record<string,unknown>)=>expect(await nativeInvoke(app.page,command,args).then(()=>false,()=>true)).toBe(true);
  await rejected('check_harness_codex_runtime',{workspaceId:scope.workspaceId,input:{...input,checkHash:'forged'}});
  await rejected('check_harness_codex_runtime',{workspaceId:scope.workspaceId,input:{...input,confirmed:false}});
  await rejected('check_harness_codex_runtime',{workspaceId:other,input});
  await rejected('check_harness_codex_runtime',{workspaceId:scope.workspaceId,input:{...input,securityPolicy:'unconfined'}});
  expect((await state()).history).toHaveLength(initial);
  const oldCheck=await nativeInvoke(app.page,'get_harness_execution_preflight',{...scope});
  await app.page.getByRole('combobox',{name:'Codex隔离镜像',exact:true}).selectOption(imageId);
  await app.page.getByRole('combobox',{name:'Codex保护策略',exact:true}).selectOption(input.securityPolicy);
  await app.page.getByRole('button',{name:'检查隔离环境',exact:true}).click();
  const dialog=app.page.getByRole('dialog',{name:'检查 Codex 隔离环境',exact:true});await expect(dialog).toBeVisible();await dialog.getByRole('button',{name:'确认检查，不调用模型',exact:true}).click();
  await expect.poll(async()=>(await state()).history.length,{timeout:30000}).toBe(initial+1);
  const id=(await state()).selected!.id;jobs.push(id);const completed=await wait(id);
  expect(completed.status,JSON.stringify(completed)).toBe('completed');expect(completed.clean).toBe(true);expect(completed.modelCalls).toBe(0);expect(completed.taskExecution).toBe('Not Run');
  const observed=completed.events.find(e=>e.kind==='probe')!.data;
  expect(observed.cliVersion).toBe('0.166.0');expect(observed.raw.runtimeFilesVerified).toBe(true);expect(observed.checks.slice(0,3).every((c:{state:string})=>c.state==='passed')).toBe(true);
  expect(observed.sandboxReady).toBe(compatible);expect(observed.canRun).toBe(false);
  const statusText=compatible?'本次隔离检查已通过。':'目录权限保护未就绪，自动编程仍不可用。';
  if(compatible){const proof=JSON.parse(observed.raw.sandbox.stdout);for(const key of ['insideWrite','outsideDenied','readonlyRootDenied','networkDenied','mountDenied','bpfDenied','zeroCapabilities','noNewPrivileges','seccompActive','noDockerSocket'])expect(proof[key],key).toBe(true);expect(observed.policy.seccompPolicy.hash).toBe('46a3ec985105a85a8d6bb5d21974a5dd5093f176411d766f8acf51bf5f2d9f13');expect(observed.verifiedConfiguration.hostConfig.CapDrop).toContain('ALL');expect(observed.verifiedConfiguration.mounts.every((m:{Type:string})=>m.Type==='tmpfs')).toBe(true);}else{expect(observed.checks.find((c:{id:string})=>c.id==='sandbox').reason).toBe('namespace_unavailable');expect(observed.raw.sandbox.stdout).toBe('');}
  await expect(app.page.getByText(statusText,{exact:true})).toBeVisible();
  await expect(app.page.getByText('检查容器已清理',{exact:true})).toBeVisible();
  await rejected('get_harness_codex_runtime',{...scope,jobId:scope.checkId});
  await rejected('cleanup_harness_codex_runtime',{workspaceId:other,jobId:id});
  await rejected('get_harness_codex_runtime',{...scope,checkId:fixture.records.history[1].id,jobId:id});
  expect(await nativeInvoke(app.page,'cleanup_harness_codex_runtime',{workspaceId:scope.workspaceId,jobId:id})).toEqual(completed);
  expect(await nativeInvoke(app.page,'get_harness_execution_preflight',{...scope})).toEqual(oldCheck);
  for(const theme of ['light','dark']){await app.page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await app.page.setViewportSize({width,height});await app.page.locator('.fit-scroll').evaluate(e=>{e.scrollTop+=e.querySelector('.codex-runtime')!.getBoundingClientRect().top-e.getBoundingClientRect().top-12;});expect(await app.page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);await app.page.screenshot({path:info.outputPath(`runtime-${theme}-${width}.png`)});}}
  const stopId=(await nativeInvoke<{id:string}>(app.page,'check_harness_codex_runtime',{workspaceId:scope.workspaceId,input})).id;jobs.push(stopId);
  await nativeInvoke(app.page,'cleanup_harness_codex_runtime',{workspaceId:scope.workspaceId,jobId:stopId});
  const stopped=await wait(stopId);expect(stopped.status).toBe('cancelled');expect(stopped.clean).toBe(true);expect(stopped.events.some(e=>e.kind==='probe')).toBe(false);
  await app.closeFromUi();await app.stop();app=await launchNative(info,{dataDirectory:data,profileDirectory:info.outputPath('reopen-profile')});await open();
  expect((await state()).history).toHaveLength(initial+2);expect((await state(id)).selected).toEqual(completed);
  await app.page.getByRole('combobox',{name:'Codex隔离检查历史',exact:true}).selectOption(id);await expect(app.page.getByText(statusText,{exact:true})).toBeVisible();
  const absent=await exec('docker',['--host',completed.plan.environment.endpoint,'container','ls','-a','--filter',`label=com.cortexos.validation=${id}`,'--format','{{.Names}}'],{windowsHide:true,timeout:10000});expect(absent.stdout.trim()).toBe('');
  // Freeze only this test's owned container so forced-restart recovery is observable.
  const recoveryId=(await nativeInvoke<{id:string}>(app.page,'check_harness_codex_runtime',{workspaceId:scope.workspaceId,input})).id;jobs.push(recoveryId);
  const pending=(await state(recoveryId)).selected!;
  await expect.poll(async()=>{try{await exec('docker',['--host',pending.plan.environment.endpoint,'pause',pending.plan.containerName],{windowsHide:true,timeout:5000});return true;}catch{return false;}},{timeout:7000,intervals:[30,50]}).toBe(true);
  await exec('taskkill',['/PID',String(app.processId),'/T','/F'],{windowsHide:true});await app.stop();
  app=await launchNative(info,{dataDirectory:data,profileDirectory:info.outputPath('recovery-profile')});await open();
  const interrupted=(await state(recoveryId)).selected!;expect(interrupted.status).toBe('interrupted');expect(interrupted.clean).toBe(false);
  await expect(app.page.getByRole('button',{name:'检查隔离环境',exact:true})).toBeDisabled();
  await app.page.getByRole('button',{name:'清理检查容器',exact:true}).click();
  await expect.poll(async()=>(await state(recoveryId)).selected?.status,{timeout:20000}).toBe('closed');
  const recovered=(await state(recoveryId)).selected!;expect(recovered.clean).toBe(true);expect(recovered.events.some(e=>e.kind==='probe')).toBe(false);
  expect((await state(id)).selected).toEqual(completed);expect((await state(stopId)).selected).toEqual(stopped);
  if(legacy)expect((await state(legacy.completed.id)).selected).toEqual(legacy.completed);
  await app.page.getByRole('combobox',{name:'Codex隔离检查历史',exact:true}).selectOption(id);
  expect(app.consoleErrors).toEqual([]);
  await writeFile(info.outputPath('runtime.json'),JSON.stringify({workspaceId:scope.workspaceId,checkId:scope.checkId,assessmentVersion:1,completed,stopped,interrupted,recovered,modelCalls:0,taskExecution:'Not Run'},null,2));
 }finally{
  for(const jobId of jobs){try{await nativeInvoke(app.page,'cleanup_harness_codex_runtime',{workspaceId:scope.workspaceId,jobId});}catch{/* Ownership remains persisted if cleanup failed. */}}
  try{await app.closeFromUi();}catch{}await app.stop();
 }
});
