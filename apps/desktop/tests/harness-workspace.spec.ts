import { expect,test,type TestInfo } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir,readFile,writeFile } from 'node:fs/promises';
import { resolve,join } from 'node:path';
import { launchNative,nativeInvoke } from './native';
const exec=promisify(execFile);
interface Copy {id:string;planHash:string;status:string;pendingContainers:string[];error:string|null;plan:{containerName:string;imageId:string;bundle:{taskJson:string;originalCode:string};datasetVersion:number};events:{kind:string;data:Record<string,any>}[]}
interface Payload {selected:Copy|null;history:{id:string;status:string}[];active:{id:string;checkId:string}|null}
async function setup(info:TestInfo){
 const source=resolve('../../artifacts/harness-preflight-fixed/harness-preflight-Codex-pr-ea7ea-ions-without-starting-tasks-harness-preflight');
 const fixture=JSON.parse(await readFile(join(source,'preflight.json'),'utf8'));
 const data=info.outputPath('data');await mkdir(data,{recursive:true});
 await exec('py',['-3','-c','import sqlite3,sys\ns=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True)\nd=sqlite3.connect(sys.argv[2])\ns.backup(d)\nd.close()\ns.close()',join(source,'data/cortexos.sqlite3'),join(data,'cortexos.sqlite3')],{windowsHide:true});
 const scope={workspaceId:fixture.workspaceId as string,assessmentVersion:1,checkId:fixture.records.selected.id as string};
 const ctx={app:await launchNative(info,{dataDirectory:data}),scope,data,jobs:[] as string[]};
 const state=(id:string|null=null)=>nativeInvoke<Payload>(ctx.app.page,'get_harness_workspace',{...scope,jobId:id});
 const open=async()=>{const p=ctx.app.page;await p.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},scope.workspaceId);await p.reload();await p.getByRole('button',{name:'4. 测试任务',exact:true}).click();await p.getByText('Codex 执行前检查',{exact:true}).click();await expect(p.getByRole('combobox',{name:'执行检查历史',exact:true})).toBeVisible();await p.getByRole('combobox',{name:'执行检查历史',exact:true}).selectOption(scope.checkId);await p.getByText('任务副本与独立测试',{exact:true}).click();};
 const wait=async(id:string,status:string)=>{const terminal=status==='ready'?['ready','failed','cancelled','cleanup_required']:status==='closed'?['closed','cleanup_required']:['completed','failed','cancelled','cleanup_required','interrupted'];await expect.poll(async()=>terminal.includes((await state(id)).selected?.status??''),{timeout:90000,intervals:[300,700]}).toBe(true);const r=(await state(id)).selected!;expect(r.status,JSON.stringify({error:r.error,events:r.events.map(e=>({kind:e.kind,reason:e.data.reason}))})).toBe(status);return r;};
 const prepare=async()=>{const p=ctx.app.page;await expect(p.getByRole('button',{name:'新建任务副本',exact:true})).toBeEnabled({timeout:15000});await p.getByRole('button',{name:'新建任务副本',exact:true}).click();await p.getByRole('dialog',{name:'准备任务副本',exact:true}).getByRole('button',{name:'准备副本',exact:true}).click();await expect.poll(async()=>(await state()).history.length,{timeout:30000}).toBe(ctx.jobs.length+1);const r=(await state()).selected!;ctx.jobs.push(r.id);return wait(r.id,'ready');};
 const submit=async(code:string)=>{const p=ctx.app.page;await expect(p.getByRole('textbox',{name:'副本函数修改',exact:true})).toBeEditable();await p.getByRole('textbox',{name:'副本函数修改',exact:true}).fill(code);await p.getByRole('button',{name:'收集修改并独立测试',exact:true}).click();await p.getByRole('dialog',{name:'收集修改并独立测试',exact:true}).getByRole('button',{name:'确认并独立测试',exact:true}).click();};
 const docker=async(r:Copy,args:string[])=>{const env=r.events.find(e=>e.kind==='environment')!.data;return exec('docker',['--host',env.endpoint,...args],{windowsHide:true,timeout:15000});};
 const noContainers=async(r:Copy)=>{expect((await docker(r,['container','ls','-a','--filter',`label=com.cortexos.validation=${r.id}`,'--format','{{.Names}}'])).stdout.trim()).toBe('');};
 const cleanup=async()=>{for(const id of ctx.jobs){try{await nativeInvoke(ctx.app.page,'close_harness_workspace',{workspaceId:scope.workspaceId,jobId:id});await expect.poll(async()=>(await state(id)).selected?.pendingContainers.length,{timeout:20000}).toBe(0);}catch{/* Failure evidence remains; normal close attempts owned cleanup again. */}}try{await ctx.app.closeFromUi();}catch{}await ctx.app.stop();};
 await open();return Object.assign(ctx,{fixture,state,open,wait,prepare,submit,docker,noContainers,cleanup});
}

test('task copy protects instructions, collects manual changes and grades in fresh isolated containers',async({},info)=>{
 test.setTimeout(240000);const ctx=await setup(info);const {scope}=ctx;
 try{
  const before=await nativeInvoke(ctx.app.page,'get_harness_programming_taskset',{workspaceId:scope.workspaceId,datasetId:ctx.fixture.datasetId,version:2});
  const r=await ctx.prepare();
  const config=JSON.parse((await ctx.docker(r,['inspect',r.plan.containerName,'--format','{{json .}}'])).stdout);
  expect(config.HostConfig.NetworkMode).toBe('none');expect(config.HostConfig.ReadonlyRootfs).toBe(true);expect(config.HostConfig.Privileged).toBe(false);expect(config.HostConfig.Memory).toBe(268435456);expect(config.HostConfig.NanoCpus).toBe(1000000000);expect(config.HostConfig.PidsLimit).toBe(64);expect(config.HostConfig.Binds??[]).toEqual([]);expect(config.Mounts.every((m:{Type:string})=>m.Type==='tmpfs')).toBe(true);expect(config.Config.Cmd.at(-1)).toBe('import time; time.sleep(600)');
  const safety=JSON.parse((await ctx.docker(r,['exec','--user','65534:65534',r.plan.containerName,'python3','-I','-S','-c',`import os,json,pathlib
task=json.loads(pathlib.Path('/workspace/task.json').read_text())
denied=[]
for p in ['/workspace/task.json','/workspace/new.py','/workspace/project/new.py']:
 try:
  fd=os.open(p,os.O_WRONLY|os.O_CREAT);os.close(fd)
 except PermissionError:denied.append(p)
print(json.dumps({'uid':os.getuid(),'denied':denied,'task':task,'files':sorted(str(p.relative_to('/workspace')) for p in pathlib.Path('/workspace').rglob('*') if p.is_file()),'socket':pathlib.Path('/var/run/docker.sock').exists()}))`])).stdout);
  expect(safety.uid).toBe(65534);expect(safety.denied).toHaveLength(3);expect(safety.files).toEqual(['project/src/value.py','task.json']);expect(safety.socket).toBe(false);expect(safety.task.target.contentScope).toBe('selected_function_only');
  for(const key of ['examples','expected','referenceCode','wrongCode','validation'])expect(Object.keys(safety.task)).not.toContain(key);
  const other=(await nativeInvoke<{id:string}>(ctx.app.page,'create_workspace',{name:'副本隔离'})).id;
  expect(await nativeInvoke(ctx.app.page,'close_harness_workspace',{workspaceId:other,jobId:r.id}).then(()=>false,()=>true)).toBe(true);
  expect(await nativeInvoke(ctx.app.page,'submit_harness_workspace',{workspaceId:scope.workspaceId,input:{jobId:r.id,planHash:'forged',code:'def adjust(n):\n return 0',confirmed:true}}).then(()=>false,()=>true)).toBe(true);
  const code='def adjust(n):\n    return max(0, n)';await ctx.submit(code);const passed=await ctx.wait(r.id,'completed');
  expect(passed.pendingContainers).toEqual([]);const score=passed.events.find(e=>e.kind==='score')!.data.score;expect(score.state).toBe('Passed');expect(score.metrics.assertionsRun).toBe(7);expect(score.metrics.assertionsPassed).toBe(7);
  const patch=passed.events.find(e=>e.kind==='candidate')!.data.patch;expect(patch.after).toBe(code);expect(patch.unchangedOutsideRange).toBe(true);expect(patch.writtenToOriginal).toBe(false);await ctx.noContainers(passed);
  await expect(ctx.app.page.getByRole('button',{name:'查看本次修改',exact:true})).toBeVisible();await ctx.app.page.getByRole('button',{name:'查看本次修改',exact:true}).click();const dialog=ctx.app.page.getByRole('dialog',{name:'本次函数修改',exact:true});await dialog.locator('.panel-body').evaluate(e=>{e.scrollTop=e.scrollHeight;});await expect(dialog.getByRole('button',{name:'关闭本次修改',exact:true})).toBeVisible();await dialog.getByRole('button',{name:'关闭本次修改',exact:true}).click();
  for(const theme of ['light','dark']){await ctx.app.page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await ctx.app.page.setViewportSize({width,height});await ctx.app.page.locator('.fit-scroll').evaluate(e=>{e.scrollTop+=e.querySelector('.workspace-handoff')!.getBoundingClientRect().top-e.getBoundingClientRect().top-12;});expect(await ctx.app.page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);await ctx.app.page.screenshot({path:info.outputPath(`workspace-${theme}-${width}.png`)});}}
  expect(await nativeInvoke(ctx.app.page,'close_harness_workspace',{workspaceId:scope.workspaceId,jobId:r.id})).toEqual(passed);
  const second=await ctx.prepare();await ctx.submit('def adjust(n):\n    return 0');const failed=await ctx.wait(second.id,'completed');expect(failed.events.find(e=>e.kind==='score')!.data.score.state).toBe('Failed');expect(failed.pendingContainers).toEqual([]);await ctx.noContainers(failed);
  await ctx.app.page.getByRole('combobox',{name:'任务副本历史',exact:true}).selectOption(r.id);await ctx.app.page.getByText('查看已提交代码',{exact:true}).click();await expect(ctx.app.page.getByRole('textbox',{name:'副本函数修改',exact:true})).toHaveValue(code);expect((await ctx.state(r.id)).selected).toEqual(passed);
  expect(await nativeInvoke(ctx.app.page,'get_harness_programming_taskset',{workspaceId:scope.workspaceId,datasetId:ctx.fixture.datasetId,version:2})).toEqual(before);
  await writeFile(info.outputPath('workspace.json'),JSON.stringify({workspaceId:scope.workspaceId,checkId:scope.checkId,assessmentVersion:1,passed,failed,safety,configuration:config,noModelCalls:true,noHarnessExecution:true},null,2));expect(ctx.app.consoleErrors).toEqual([]);
 }finally{await ctx.cleanup();}
});

test('task copy rejects foreign files and preserves cancelled, normal-close and forced-restart history',async({},info)=>{
 test.setTimeout(240000);const ctx=await setup(info);const {scope}=ctx;
 try{
  const bad=await ctx.prepare();await ctx.docker(bad,['exec','--user','0:0',bad.plan.containerName,'python3','-I','-S','-c',"import os,pathlib;os.chmod('/workspace',0o755);pathlib.Path('/workspace/extra.py').write_text('unexpected');os.chmod('/workspace',0o555)"]);
  await ctx.submit('def adjust(n):\n    return max(0, n)');const rejected=await ctx.wait(bad.id,'failed');expect(rejected.events.some(e=>e.kind==='candidate'||e.kind==='score')).toBe(false);expect(rejected.error).toBe('workspace_unexpected_file');expect(rejected.pendingContainers).toEqual([]);await ctx.noContainers(rejected);
  const looping=await ctx.prepare();await ctx.submit('def adjust(n):\n    while True:\n        pass');await expect.poll(async()=>(await ctx.state(looping.id)).selected!.events.some(e=>e.kind==='container'&&e.data.role==='grader'),{timeout:30000}).toBe(true);
  await ctx.app.page.getByRole('button',{name:'停止并清理',exact:true}).click();const cancelled=await ctx.wait(looping.id,'cancelled');expect(cancelled.pendingContainers).toEqual([]);expect(cancelled.events.find(e=>e.kind==='submission')!.data.code).toContain('while True');await ctx.noContainers(cancelled);
  const idle=await ctx.prepare();await ctx.app.closeFromUi();await ctx.app.stop();
  ctx.app=await launchNative(info,{dataDirectory:ctx.data,profileDirectory:info.outputPath('normal-close-profile')});await ctx.open();const closed=(await ctx.state(idle.id)).selected!;expect(closed.status).toBe('closed');expect(closed.pendingContainers).toEqual([]);await ctx.noContainers(closed);
  const interrupted=await ctx.prepare();await exec('taskkill',['/PID',String(ctx.app.processId),'/T','/F'],{windowsHide:true});await ctx.app.stop();
  ctx.app=await launchNative(info,{dataDirectory:ctx.data,profileDirectory:info.outputPath('forced-restart-profile')});await ctx.open();const recovered=(await ctx.state(interrupted.id)).selected!;expect(recovered.status).toBe('interrupted');expect(recovered.pendingContainers).toHaveLength(1);await expect(ctx.app.page.getByRole('button',{name:'新建任务副本',exact:true})).toBeDisabled();
  await ctx.app.page.getByRole('button',{name:'关闭并清理副本',exact:true}).click();await ctx.app.page.getByRole('dialog',{name:'关闭任务副本',exact:true}).getByRole('button',{name:'关闭并放弃未提交编辑',exact:true}).click();const cleaned=await ctx.wait(interrupted.id,'closed');expect(cleaned.pendingContainers).toEqual([]);await ctx.noContainers(cleaned);
  expect((await ctx.state()).history).toHaveLength(4);expect((await ctx.state(looping.id)).selected).toEqual(cancelled);expect((await ctx.state(bad.id)).selected).toEqual(rejected);
  await writeFile(info.outputPath('recovery.json'),JSON.stringify({workspaceId:scope.workspaceId,checkId:scope.checkId,rejected,cancelled,closed,recovered,cleaned,noModelCalls:true,noHarnessExecution:true},null,2));expect(ctx.app.consoleErrors).toEqual([]);
 }finally{await ctx.cleanup();}
});
