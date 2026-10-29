import { expect,test } from '@playwright/test';
import { writeFile,mkdir,readFile } from 'node:fs/promises';
import { resolve,sep } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { launchNative,nativeInvoke } from './native';
import type { Batch,ModelRequest } from '../src/benchmark/personalized';

test('installed local identities complete fixed smoke plans with auditable recovery and grades',async({},info)=>{
 test.setTimeout(2_700_000);
 const savedDirectory=process.env.CORTEXOS_BENCHMARK_ACCEPTANCE_SAVED;
 if(savedDirectory&&!resolve(savedDirectory).startsWith(resolve('../../artifacts')+sep))throw Error('Saved acceptance directory must be in repository artifacts');
 const app=await launchNative(info,savedDirectory?{dataDirectory:resolve(savedDirectory,'data')}:{});
 let activeBatch:string|undefined,workspaceId:string|undefined;
 const save=async(name:string,value:unknown)=>{await mkdir(info.outputDir,{recursive:true});await writeFile(info.outputPath(name),JSON.stringify(value,null,2));};
 try{
  const {page}=app;page.setDefaultTimeout(10000);
  const before=await nativeInvoke<{settings:unknown;workspaces:{id:string;name:string}[]}>(page,'bootstrap');
  const workspace=savedDirectory?before.workspaces.find(w=>w.name==='Core60 v2 真实本地验收')!:await nativeInvoke<{id:string}>(page,'create_workspace',{name:'Core60 v2 真实本地验收'});workspaceId=workspace.id;
  const catalog=await nativeInvoke<{models:{name:string;digest:string|null}[]}>(page,'list_benchmark_models',{workspaceId});
  const unique=catalog.models.filter((m,i,a)=>m.digest&&a.findIndex(n=>n.digest===m.digest)===i).slice(0,2);
  expect(unique.length).toBeGreaterThan(0);
  const datasets=await nativeInvoke<{id:string;version:number;contentHash:string}[]>(page,'list_benchmark_datasets',{workspaceId});
  const dataset=datasets.find(d=>d.id==='cortex-core-60'&&d.version===2)!;expect(dataset).toBeTruthy();
  const models=unique.map(m=>({kind:'local',local:{model:m.name,expectedDigest:m.digest,parameters:{numCtx:4096,numPredict:1024,temperature:0,timeoutSeconds:120},requirements:['completion']} satisfies ModelRequest}));
  await save('environment.json',{catalog,dataset,models,scope:'isolated SQLite and WebView2; no cloud or model downloads',pythonImage:null});
  const get=()=>nativeInvoke<Batch>(page,'get_standard_benchmark',{workspaceId,batchId:activeBatch});
  const overview=()=>nativeInvoke<Batch>(page,'get_benchmark_overview',{workspaceId,batchId:activeBatch,scope:'standard'});
  const plan=async(caseIds:string[],repetitions:number)=>{
   const b=await nativeInvoke<Batch>(page,'plan_standard_benchmark',{workspaceId,input:{datasetId:dataset.id,datasetVersion:2,datasetHash:dataset.contentHash,models,maxOutputTokens:1024,price:null,pythonImage:null,caseIds,repetitions,releaseModels:[]}});
   activeBatch=b.id;await save(`plan-${b.id}.json`,b);return b;
  };
  const start=async(b:Batch)=>nativeInvoke(page,'start_standard_benchmark',{workspaceId,batchId:b.id,approvedPlanHash:b.planHash});
  const wait=async(label:string)=>{
   let last='';const deadline=Date.now()+2_100_000;
   while(Date.now()<deadline){
    const b=await overview();
    const progress={label,id:b.id,status:b.status,models:b.reports?.map(r=>({name:r.name,statistics:r.statistics}))};
    const signature=JSON.stringify(progress);
    if(signature!==last){last=signature;console.log(signature);await save('progress.json',progress);}
    if(!['running','stopping'].includes(b.status)){expect(b.status).toBe('completed');return await get();}
    await page.waitForTimeout(3000);
   }
   throw Error('Bounded real acceptance deadline reached; persisted batch retained');
  };
  let stopped=false,preserved:NonNullable<Batch['items']>=[],small:Batch,full:Batch,report:Batch;
  if(savedDirectory){
   const previous=JSON.parse(await readFile(resolve(savedDirectory,'small.json'),'utf8')) as {stopped:boolean;completedItemsPreserved:NonNullable<Batch['items']>;report:Batch};
   small=previous.report;stopped=previous.stopped;preserved=previous.completedItemsPreserved;
   full=JSON.parse(await readFile(resolve(savedDirectory,'report.json'),'utf8')) as Batch;activeBatch=full.id;report=await get();
   expect(report.status).toBe('completed');
  }else{
  small=await plan(['math-001','rag-001'],2);await start(small);
  const stopDeadline=Date.now()+240000;
  while(Date.now()<stopDeadline){
   const b=await get();if(!['running','stopping'].includes(b.status))break;
   if(b.items?.some(i=>i.status==='finished')){await nativeInvoke(page,'stop_standard_benchmark',{workspaceId,batchId:b.id});stopped=true;break;}
   await page.waitForTimeout(250);
  }
  if(stopped){
   await expect.poll(async()=>(await get()).status,{timeout:150000,intervals:[500,1000]}).not.toMatch(/^(running|stopping)$/);
   const paused=await get();preserved=paused.items!.filter(i=>i.status==='finished');await save('stopped.json',paused);
   await start(paused);
  }
  const smallResult=await wait('bounded repetitions');
  for(const item of preserved)expect(smallResult.items!.find(i=>i.ordinal===item.ordinal)?.runId).toBe(item.runId);
  await save('small.json',{stopped,completedItemsPreserved:preserved,report:smallResult});
  expect(smallResult.items).toHaveLength(unique.length*4);
  full=await plan([],1);expect(full.items).toHaveLength(unique.length*60);await start(full);
  report=await wait('complete Core60 v2 plan');await save('report.json',report);
  }
  expect(report.items?.every(i=>i.status==='finished')).toBe(true);
  const details=[];
  for(const item of report.items??[]){
   expect(item.runId).toBeTruthy();
   const d=await nativeInvoke<Record<string,unknown>>(page,'get_benchmark_run',{workspaceId,experimentId:item.runId});
   details.push(d);
  }
  await save('details.json',details);
  for(const r of report.reports??[]){
   expect(r.statistics.planned).toBe(60);
   expect(Object.values(r.statistics.states).reduce((a,b)=>a+b,0)).toBe(60);
   expect(r.statistics.states['In Progress']).toBe(0);expect(r.statistics.states['Not Run']).toBe(0);
  }
  const original=(await overview()).reports!.map(r=>r.statistics);
  const preview=await nativeInvoke<{sessionId:string;planHash:string;eligible:number}>(page,'preview_benchmark_regrade',{workspaceId,batchId:full.id,scope:'standard'});
  const grade=await nativeInvoke(page,'run_benchmark_regrade',{workspaceId,batchId:full.id,scope:'standard',sessionId:preview.sessionId,approvedPlanHash:preview.planHash});
  expect((await overview()).reports!.map(r=>r.statistics)).toEqual(original);
  await save('regrade.json',{preview,result:grade,originalStatisticsUnchanged:true});
  expect(await nativeInvoke(page,'list_conversations',{workspaceId})).toEqual([]);
  expect((await nativeInvoke<{settings:unknown}>(page,'bootstrap')).settings).toEqual(before.settings);
  await page.evaluate(({workspaceId,batchId})=>{location.hash=`/benchmark?workspace=${workspaceId}&batch=${batchId}`;},{workspaceId,batchId:full.id});
  await page.reload();await expect(page.getByRole('button',{name:'导出报告',exact:true})).toBeVisible();
  for(const format of ['json','csv']){
   const file=info.outputPath(`export.${format}`);
   const saving=nativeInvoke<boolean>(page,'export_benchmark_report',{workspaceId,batchId:full.id,scope:'standard',format});
   await promisify(execFile)('pwsh.exe',['-NoProfile','-File',resolve('tests/save-report-dialog.ps1'),'-AppProcessId',String(app.processId),'-OutputPath',file],{windowsHide:true,timeout:20000});
   expect(await saving).toBe(true);
   expect((await readFile(file)).length).toBeGreaterThan(100);
  }
  const exported=JSON.parse(await readFile(info.outputPath('export.json'),'utf8'));
  expect(exported.reportSchema).toBe('cortexos.benchmark-report');expect(exported.payload.items).toHaveLength(full.items!.length);
  await page.setViewportSize({width:1600,height:900});await page.screenshot({path:info.outputPath('real-core60-report.png')});
  await save('acceptance.json',{workspaceId,batchId:full.id,smallBatchId:small.id,actualModelIdentities:unique,stopped,completedItemsPreserved:preserved.length,plannedItems:full.items!.length,settingsUnchanged:true,ordinaryChatHistoryEmpty:true,dockerLinuxVerified:false,reportOnly:'Level 1 Smoke; not a model capability leaderboard'});
 }finally{
  if(activeBatch&&workspaceId)await nativeInvoke(app.page,'stop_standard_benchmark',{workspaceId,batchId:activeBatch}).catch(()=>{});
  await app.stop();
 }
});
