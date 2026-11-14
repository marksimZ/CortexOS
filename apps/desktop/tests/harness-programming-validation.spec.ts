import { expect,test,type TestInfo } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir,writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { launchNative,nativeInvoke } from './native';
import type { Snapshot } from '../src/benchmark/harness/types';
import type { ProgrammingJob,ProgrammingPublished,ProgrammingSpec,ValidationPayload } from '../src/benchmark/harness/programmingTypes';

const exec=promisify(execFile);
async function setup(info:TestInfo){
 await mkdir(info.outputPath('fixtures'),{recursive:true});const doc=info.outputPath('fixtures/development.md'),zip=info.outputPath('fixtures/project.zip');
 await writeFile(doc,'Use Python pure functions.\nKeep immutable evidence.\n');
 await exec('py',['-3','-X','utf8','-c','import sys,zipfile\nwith zipfile.ZipFile(sys.argv[1],"w") as z:\n z.writestr("src/value.py","def adjust(n):\\n    return n\\n")',zip],{windowsHide:true});
 const app=await launchNative(info,{sourcePath:doc});const {page}=app;page.setDefaultTimeout(15000);
 const workspaceId=(await nativeInvoke<{id:string}>(page,'create_workspace',{name:'编程任务对照验收'})).id;
 const snapshot=await nativeInvoke<Snapshot>(page,'import_harness_document',{workspaceId});
 await nativeInvoke(page,'save_harness_fit',{workspaceId,input:{snapshotIds:[snapshot.id],requirements:[],weights:{compatibility:3,automation:3,governance:3,context:3},expectedVersion:0,note:''}});
 await page.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},workspaceId);await page.reload();
 await page.getByRole('button',{name:'4. 测试任务',exact:true}).click();await page.getByText('添加代码文件定位任务',{exact:true}).click();
 await page.getByRole('button',{name:'选择项目ZIP',exact:true}).click();
 await exec('pwsh.exe',['-NoProfile','-File',resolve('tests/save-report-dialog.ps1'),'-AppProcessId',String(app.processId),'-DialogTitle','选择项目ZIP','-OutputPath',zip],{windowsHide:true,timeout:25000});
 const repositoryId=await page.getByRole('combobox',{name:'任务项目快照',exact:true}).inputValue();
 const origin={assessmentVersion:1,repositoryId,path:'src/value.py',start:1,end:2};
 const spec:ProgrammingSpec={basis:'confirmed_requirement',rationale:'受控人工需求：只将负数归零，非负输入不变。',prompt:'返回非负输入本身，负数返回0。',function:'adjust',parameters:[{name:'n',kind:'integer'}],returnType:'integer',examples:Array.from({length:7},(_,i)=>({label:`输入 ${i-3}`,role:i<3?'target':i===3?'boundary':'regression',args:[i-3],output:Math.max(0,i-3)})),referenceCode:'def adjust(n):\n    return max(0, n)',wrongCode:'def adjust(n):\n    return 0'};
 const job=await nativeInvoke<ProgrammingJob>(page,'save_harness_programming_draft',{workspaceId,input:{jobId:null,expectedRevision:0,origin,spec}});
 const open=async()=>{await page.reload();await page.getByRole('button',{name:'4. 测试任务',exact:true}).click();await page.getByText('编程任务草案',{exact:true}).click();};
 await open();
 const panel=()=>page.getByRole('region',{name:'编程任务验证与发布',exact:true});
 const get=()=>nativeInvoke<ValidationPayload>(page,'get_harness_programming_validation',{workspaceId,draftId:job.id,validationId:null});
 const start=async()=>{await page.getByRole('button',{name:'读取本机Python镜像',exact:true}).click();await expect(page.getByRole('combobox',{name:'Python验证镜像',exact:true})).toBeVisible({timeout:30000});await expect(page.getByRole('button',{name:'开始容器验证',exact:true})).toBeEnabled({timeout:30000});await page.getByRole('button',{name:'开始容器验证',exact:true}).click();await page.getByRole('dialog',{name:'执行三组容器验证',exact:true}).getByRole('button',{name:'开始验证',exact:true}).click();};
 const wait=async()=>{await expect.poll(async()=>(await get()).selected?.status,{timeout:180000,intervals:[500,1000]}).not.toBe('validating');const state=await get();expect(state.selected).toBeTruthy();return state;};
 return {app,page,workspaceId,job,origin,spec,open,panel,get,start,wait};
}

test('real controls, human approval and immutable published versions stay bound to each draft revision',async({},info)=>{
 test.setTimeout(300000);const ctx=await setup(info);const {app,page,workspaceId,job,start,wait,get}=ctx;
 try{
  await start();const first=(await wait()).selected!;expect(first.publishable).toBe(true);expect(first.pendingContainers).toEqual([]);expect(first.evidenceBound).toBe(true);
  await expect(page.getByText('三组对照通过',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'发布任务新版本',exact:true})).toBeDisabled();
  await page.getByLabel('我已核对题意、来源、用例和三组实际结果，批准此修订',{exact:true}).check();await page.getByRole('textbox',{name:'编程任务审核人',exact:true}).fill('原生验收审核人');
  await page.getByRole('button',{name:'发布任务新版本',exact:true}).click();await expect.poll(async()=>(await get()).published.length).toBe(1);
  const datasetId=`harness-python-${job.id}`,v1=await nativeInvoke<ProgrammingPublished>(page,'get_harness_programming_taskset',{workspaceId,datasetId,version:1});
  await page.getByRole('textbox',{name:'编程任务预期行为',exact:true}).fill('返回非负输入本身，负数返回0；不得修改输入。');
  await page.getByRole('button',{name:'保存编程草案',exact:true}).click();await expect(page.getByRole('combobox',{name:'编程草案修订记录',exact:true})).toHaveValue('2');
  await expect(page.getByText('当前修订未验证',{exact:true})).toBeVisible();
  expect(await nativeInvoke(page,'publish_harness_programming_task',{workspaceId,input:{draftId:job.id,expectedRevision:2,validationId:first.id,validationHash:first.contentHash,reviewer:'不得复用旧验证',approved:true,expectedVersion:1}}).then(()=>false,()=>true)).toBe(true);
  await start();const second=(await wait()).selected!;expect(second.publishable).toBe(true);expect(second.id).not.toBe(first.id);
  await page.getByLabel('我已核对题意、来源、用例和三组实际结果，批准此修订',{exact:true}).check();await page.getByRole('textbox',{name:'编程任务审核人',exact:true}).fill('二次核对审核人');await page.getByRole('button',{name:'发布任务新版本',exact:true}).click();
  await expect.poll(async()=>(await get()).published.length).toBe(2);
  expect(await nativeInvoke(page,'get_harness_programming_taskset',{workspaceId,datasetId,version:1})).toEqual(v1);
  await page.getByRole('combobox',{name:'已发布编程任务',exact:true}).selectOption('1');
  const dialog=page.getByRole('dialog',{name:'已发布任务版本 1',exact:true});await expect(dialog).toBeVisible();await expect(dialog).toContainText('原生验收审核人');
  await dialog.getByRole('button',{name:'查看发布来源',exact:true}).click();await expect(page.getByRole('dialog',{name:/src\/value.py/})).toBeVisible();await page.getByRole('button',{name:'关闭任务原文',exact:true}).click();
  await dialog.locator('.panel-body').evaluate(e=>{e.scrollTop=e.scrollHeight;});await expect(dialog.getByRole('button',{name:'关闭已发布编程任务',exact:true})).toBeVisible();await dialog.getByRole('button',{name:'关闭已发布编程任务',exact:true}).click();
  for(const theme of ['light','dark']){
   await page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);
   for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await page.setViewportSize({width,height});await page.locator('.fit-scroll').evaluate(e=>{e.scrollTop+=e.querySelector('.programming-validation')!.getBoundingClientRect().top-e.getBoundingClientRect().top-12;});expect(await page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);await page.screenshot({path:info.outputPath(`validation-${theme}-${width}.png`)});}
  }
  const other=(await nativeInvoke<{id:string}>(page,'create_workspace',{name:'隔离'})).id;
  expect(await nativeInvoke(page,'get_harness_programming_taskset',{workspaceId:other,datasetId,version:1}).then(()=>false,()=>true)).toBe(true);
  expect(await nativeInvoke(page,'get_harness_programming_validation',{workspaceId:other,draftId:job.id,validationId:second.id}).then(()=>false,()=>true)).toBe(true);
  expect(await nativeInvoke(page,'list_conversations',{workspaceId})).toEqual([]);
  await writeFile(info.outputPath('acceptance.json'),JSON.stringify({workspaceId,draftId:job.id,datasetId,validationId:second.id,versions:2,realDocker:true,noModelCalls:true,originalVersionPreserved:true},null,2));expect(app.consoleErrors).toEqual([]);
 }catch(error){await page.screenshot({path:info.outputPath('failure.png')});await writeFile(info.outputPath('failure.json'),JSON.stringify({body:await page.locator('body').innerText(),errors:app.consoleErrors},null,2));throw error;}finally{await app.stop();}
});

test('failed controls, cancellation and forced restart keep audit records and clean only owned containers',async({},info)=>{
 test.setTimeout(300000);const ctx=await setup(info);let app=ctx.app;const {workspaceId,job,origin,spec}=ctx;const owned:string[]=[];
 const get=()=>nativeInvoke<ValidationPayload>(app.page,'get_harness_programming_validation',{workspaceId,draftId:job.id,validationId:null});
 try{
  const bad={...spec,referenceCode:'def adjust(n):\n    return n + 0'};
  await nativeInvoke(app.page,'save_harness_programming_draft',{workspaceId,input:{jobId:job.id,expectedRevision:1,origin,spec:bad}});await ctx.open();await ctx.start();const failed=(await ctx.wait()).selected!;expect(failed.publishable).toBe(false);expect(failed.comparison.checks.referencePassed).toBe(false);await expect(app.page.getByText('对照未通过',{exact:true})).toBeVisible();await expect(app.page.getByRole('button',{name:'发布任务新版本',exact:true})).toHaveCount(0);
  const looping={...spec,referenceCode:'def adjust(n):\n    while True:\n        pass'};
  await nativeInvoke(app.page,'save_harness_programming_draft',{workspaceId,input:{jobId:job.id,expectedRevision:2,origin,spec:looping}});await ctx.open();await ctx.start();
  await expect.poll(async()=>Object.keys((await get()).selected?.records??{}).some(k=>k.startsWith('container-'))).toBe(true);await app.page.getByRole('button',{name:'停止验证',exact:true}).click();await expect.poll(async()=>(await get()).selected?.status,{timeout:30000}).toBe('cancelled');expect((await get()).selected!.pendingContainers).toEqual([]);
  const waiting={...spec,referenceCode:'def adjust(n):\n    import time\n    time.sleep(120)\n    return max(0, n)'};
  await nativeInvoke(app.page,'save_harness_programming_draft',{workspaceId,input:{jobId:job.id,expectedRevision:3,origin,spec:waiting}});await ctx.open();await ctx.start();
  await expect.poll(async()=>{const r=(await get()).selected!;const name=r.pendingContainers[0];if(!name)return false;try{const check=await exec('docker',['inspect',name,'--format','{{.State.Running}}'],{windowsHide:true,timeout:5000});if(check.stdout.trim()==='true'){owned.push(name);return true;}}catch{}return false;},{timeout:10000,intervals:[100]}).toBe(true);
  await exec('taskkill',['/PID',String(app.processId),'/T','/F'],{windowsHide:true});await app.stop();
  app=await launchNative(info,{dataDirectory:info.outputPath('data'),profileDirectory:info.outputPath('restarted-webview')});
  await app.page.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},workspaceId);await app.page.reload();await app.page.getByRole('button',{name:'4. 测试任务',exact:true}).click();await app.page.getByText('编程任务草案',{exact:true}).click();
  await expect.poll(async()=>(await get()).selected?.status).toBe('interrupted');expect((await get()).selected!.pendingContainers.length).toBeGreaterThan(0);
  await app.page.getByRole('button',{name:'清理本次遗留容器',exact:true}).click();await expect.poll(async()=>(await get()).selected?.pendingContainers.length).toBe(0);expect((await get()).selected!.publishable).toBe(false);
  const history=(await get()).history;expect(history).toHaveLength(3);await writeFile(info.outputPath('recovery.json'),JSON.stringify(await get(),null,2));expect(app.consoleErrors).toEqual([]);
 }finally{
  await app.stop();
  for(const name of owned){try{const check=await exec('docker',['inspect',name,'--format','{{index .Config.Labels "com.cortexos.validation"}}'],{windowsHide:true,timeout:5000});if(check.stdout.trim())await exec('docker',['rm','--force','--volumes',name],{windowsHide:true,timeout:5000});}catch{/* The product cleanup normally removed it already. */}}
 }
});
