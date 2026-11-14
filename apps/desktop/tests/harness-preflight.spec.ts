import { expect,test } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir,readFile,writeFile } from 'node:fs/promises';
import { resolve,join } from 'node:path';
import { launchNative,nativeInvoke } from './native';
const exec=promisify(execFile);
type RecordValue={id:string;contentHash:string;body:{canRun:boolean;taskExecution:string;datasetVersion:number;cli:{state:string;version?:string;cleanup?:boolean;command?:string[]};docker:{state:string};checks:{id:string;state:string}[]}};
type Payload={datasets:{id:string;version:number;contentHash:string}[];history:RecordValue[];selected:RecordValue|null};

test('Codex preflight binds real environment observations to immutable task versions without starting tasks',async({},info)=>{
 test.setTimeout(180000);
 const source=resolve('../../artifacts/harness-validation-final/harness-programming-valida-f8e50-ound-to-each-draft-revision-harness-docker-real');
 const fixture=JSON.parse(await readFile(join(source,'acceptance.json'),'utf8')) as {workspaceId:string;datasetId:string};
 const data=info.outputPath('data');await mkdir(data,{recursive:true});
 // Consistent backup of earlier actual Docker acceptance evidence; never use production data.
 await exec('py',['-3','-c','import sqlite3,sys\ns=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True)\nd=sqlite3.connect(sys.argv[2])\ns.backup(d)\nd.close()\ns.close()',join(source,'data/cortexos.sqlite3'),join(data,'cortexos.sqlite3')],{windowsHide:true});
 const doc=info.outputPath('development.md');await writeFile(doc,'Use Python pure functions.\n');
 let app=await launchNative(info,{dataDirectory:data,sourcePath:doc});
 const scope={workspaceId:fixture.workspaceId,assessmentVersion:1};
 const state=()=>nativeInvoke<Payload>(app.page,'get_harness_execution_preflight',{...scope,checkId:null});
 const open=async(workspaceId=scope.workspaceId)=>{await app.page.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},workspaceId);await app.page.reload();await app.page.getByRole('button',{name:'4. 测试任务',exact:true}).click();await app.page.getByText('Codex 执行前检查',{exact:true}).click();};
 try{
  const other=(await nativeInvoke<{id:string}>(app.page,'create_workspace',{name:'空态与隔离'})).id;
  const snapshot=await nativeInvoke<{id:string}>(app.page,'import_harness_document',{workspaceId:other});
  await nativeInvoke(app.page,'save_harness_fit',{workspaceId:other,input:{snapshotIds:[snapshot.id],requirements:[],weights:{compatibility:3,automation:3,governance:3,context:3},expectedVersion:0,note:''}});
  await open(other);await expect(app.page.getByText('当前分析版本还没有已发布的编程任务。先完成草案验证与人工发布，再更新列表。',{exact:true})).toBeVisible();await expect(app.page.getByRole('button',{name:'检查执行条件',exact:true})).toBeDisabled();
  await open();const initial=await state();expect(initial.history).toEqual([]);expect(initial.datasets).toHaveLength(2);
  const v1=initial.datasets.find(d=>d.version===1)!,v2=initial.datasets.find(d=>d.version===2)!;
  const firstVersion=await nativeInvoke(app.page,'get_harness_programming_taskset',{workspaceId:scope.workspaceId,datasetId:v1.id,version:1});
  const input={assessmentVersion:1,datasetId:v1.id,version:1,datasetHash:v1.contentHash};
  expect(await nativeInvoke(app.page,'check_harness_execution_preflight',{workspaceId:other,input}).then(()=>false,()=>true)).toBe(true);
  expect(await nativeInvoke(app.page,'check_harness_execution_preflight',{workspaceId:scope.workspaceId,input:{...input,datasetHash:'forged'}}).then(()=>false,()=>true)).toBe(true);
  expect((await state()).history).toEqual([]);
  await app.page.getByRole('combobox',{name:'执行检查任务版本',exact:true}).selectOption(`${v1.id}:1`);
  await app.page.getByRole('button',{name:'检查执行条件',exact:true}).click();
  await expect(app.page.getByRole('combobox',{name:'执行检查历史',exact:true})).toBeVisible({timeout:60000});await expect(app.page.getByText('检查记录已保存，仍有执行条件未就绪。',{exact:true})).toBeVisible();
  const first=(await state()).selected!;expect(first.body.canRun).toBe(false);expect(first.body.taskExecution).toBe('Not Run');expect(first.body.cli.state).toBe('detected');expect(first.body.cli.version).toMatch(/^\d+\.\d+\.\d+/);expect(first.body.cli.cleanup).toBe(true);expect(first.body.cli.command).toEqual(['--version']);expect(first.body.docker.state).toBe('available');
  expect(first.body.checks.filter(c=>c.state==='passed').map(c=>c.id)).toEqual(['published_task','host_cli','docker_image']);
  await app.page.getByRole('combobox',{name:'执行检查任务版本',exact:true}).selectOption(`${v2.id}:2`);
  await expect(app.page.locator('.preflight-checks')).toHaveCount(0);
  await app.page.getByRole('button',{name:'检查执行条件',exact:true}).click();await expect.poll(async()=>(await state()).history.length,{timeout:60000}).toBe(2);
  await app.page.getByRole('combobox',{name:'执行检查历史',exact:true}).selectOption(first.id);
  await expect(app.page.getByRole('combobox',{name:'执行检查任务版本',exact:true})).toHaveValue(`${v1.id}:1`);
  expect((await nativeInvoke<Payload>(app.page,'get_harness_execution_preflight',{...scope,checkId:first.id})).selected).toEqual(first);
  expect(await nativeInvoke(app.page,'get_harness_execution_preflight',{workspaceId:other,assessmentVersion:1,checkId:first.id}).then(()=>false,()=>true)).toBe(true);
  for(const theme of ['light','dark']){
   await app.page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);
   for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await app.page.setViewportSize({width,height});await app.page.locator('.fit-scroll').evaluate(e=>{e.scrollTop+=e.querySelector('.execution-preflight')!.getBoundingClientRect().top-e.getBoundingClientRect().top-12;});expect(await app.page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);await app.page.screenshot({path:info.outputPath(`preflight-${theme}-${width}.png`)});}
  }
  expect(app.consoleErrors).toEqual([]);await app.stop();
  app=await launchNative(info,{dataDirectory:data,profileDirectory:info.outputPath('reopened-profile')});await open();
  await expect(app.page.getByRole('combobox',{name:'执行检查历史',exact:true})).toBeVisible();expect((await state()).history).toHaveLength(2);
  expect(await nativeInvoke(app.page,'get_harness_programming_taskset',{workspaceId:scope.workspaceId,datasetId:v1.id,version:1})).toEqual(firstVersion);
  const counts=await exec('py',['-3','-c','import sqlite3,sys,json\nc=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True)\nprint(json.dumps({t:c.execute("select count(*) from "+t).fetchone()[0] for t in ["messages","knowledge_nodes","execution_runs","benchmark_runs"]}))',join(data,'cortexos.sqlite3')],{windowsHide:true});
  expect(JSON.parse(counts.stdout)).toEqual({messages:0,knowledge_nodes:0,execution_runs:0,benchmark_runs:0});
  await writeFile(info.outputPath('preflight.json'),JSON.stringify({workspaceId:scope.workspaceId,assessmentVersion:1,datasetId:v1.id,firstCheck:first,records:await state(),noModelCalls:true,noTaskExecution:true,counts:JSON.parse(counts.stdout)},null,2));expect(app.consoleErrors).toEqual([]);
 }finally{await app.stop();}
});
