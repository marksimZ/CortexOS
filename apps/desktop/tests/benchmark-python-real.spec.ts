import { expect,test } from '@playwright/test';
import { createHash } from 'node:crypto';
import { mkdir,readFile,writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { launchNative,nativeInvoke } from './native';
import type { Batch,ModelRequest } from '../src/benchmark/personalized';

test('Python-only rerun links the original blocked cases without changing historical runs',async({},info)=>{
 test.setTimeout(2_700_000);
 const previous=JSON.parse(await readFile(resolve('../../artifacts/harness-runtime-setup/previous-python-blocks.json'),'utf8')) as {originalReport:string;originalReportSHA256:string;originalBatchId:string;originalPlanHash:string;dataset:{id:string;version:number;contentHash:string};originalModels:{kind:string;local:ModelRequest}[];blockedItems:{model:string;caseId:string;runId:string;caseHash:string}[]};
 const originalHash=async()=>createHash('sha256').update(await readFile(resolve('../..',previous.originalReport))).digest('hex');
 expect(await originalHash()).toBe(previous.originalReportSHA256);
 const environment=JSON.parse(await readFile(resolve('../../artifacts/harness-runtime-setup/docker-environment.json'),'utf8')) as {ImageId:string;DockerHost:string};
 const safety=JSON.parse(await readFile(resolve('../../artifacts/harness-docker-live/summary.json'),'utf8')) as {scenario:string;expected:string;actual:string;containersRemoved:boolean}[];
 expect(safety).toHaveLength(9);expect(safety.every(s=>s.actual===s.expected&&s.containersRemoved)).toBe(true);
 const deadlineCheck=JSON.parse(await readFile(resolve('../../artifacts/harness-docker-live/case-deadline.json'),'utf8')) as {score:{state:string};elapsedMs:number};
 expect(deadlineCheck.score.state).toBe('Execution Error');expect(deadlineCheck.elapsedMs).toBeGreaterThanOrEqual(55000);expect(deadlineCheck.elapsedMs).toBeLessThan(75000);
 const save=async(name:string,value:unknown)=>{await mkdir(info.outputDir,{recursive:true});await writeFile(info.outputPath(name),JSON.stringify(value,null,2));};
 const onlyTruncated=process.env.CORTEXOS_D0_APPROVAL==='qwen3-4b-ten-2048';
 const previousEvidence=process.env.CORTEXOS_D0_PRIOR_DETAILS;
 if(onlyTruncated){
  expect(previousEvidence).toBeTruthy();
  const previousDetails=JSON.parse(await readFile(previousEvidence!,'utf8'));
  expect(previousDetails).toHaveLength(20);
  expect(previousDetails.filter((item:any)=>JSON.stringify(item).includes('output_truncated'))).toHaveLength(10);
  previous.originalModels=previous.originalModels.filter(m=>m.local.model==='qwen3:4b'&&m.local.expectedDigest==='359d7dd4bcdab3d86b87d73ac27966f4dbb9f5efdfcc75d34a8764a09474fae7');
  previous.blockedItems=previous.blockedItems.filter(item=>item.model==='qwen3:4b');
  expect(previous.originalModels).toHaveLength(1);expect(previous.blockedItems).toHaveLength(10);
  const settings=previous.originalModels[0].local.parameters;
  expect(settings).toMatchObject({numCtx:4096,numPredict:1024,timeoutSeconds:120});
  settings.numPredict=2048;
 }
 const expectedCount=onlyTruncated?10:20;
 const maxOutputTokens=onlyTruncated?2048:1024;
 const app=await launchNative(info,{executable:process.env.CORTEXOS_D4_EXE});
 let workspaceId:string|undefined,batchId:string|undefined;
 try{
  const {page}=app;page.setDefaultTimeout(15000);
  const before=await nativeInvoke<{settings:unknown}>(page,'bootstrap');
  workspaceId=(await nativeInvoke<{id:string}>(page,'create_workspace',{name:'Python题 Docker 独立补验'})).id;
  const catalog=await nativeInvoke<{models:{name:string;digest:string|null}[]}>(page,'list_benchmark_models',{workspaceId});
  for(const m of previous.originalModels)expect(catalog.models.some(n=>n.name===m.local.model&&n.digest===m.local.expectedDigest)).toBe(true);
  const datasets=await nativeInvoke<{id:string;version:number;contentHash:string}[]>(page,'list_benchmark_datasets',{workspaceId});
  const dataset=datasets.find(d=>d.id===previous.dataset.id&&d.version===previous.dataset.version)!;
  expect(dataset.contentHash).toBe(previous.dataset.contentHash);
  const cases=await nativeInvoke<{id:string;contentHash:string}[]>(page,'list_standard_benchmark_cases',{workspaceId});
  for(const old of previous.blockedItems)expect(cases.find(c=>c.id===old.caseId)?.contentHash).toBe(old.caseHash);
  const caseIds=[...new Set(previous.blockedItems.map(i=>i.caseId))];
  const plan=await nativeInvoke<Batch>(page,'plan_standard_benchmark',{workspaceId,input:{datasetId:dataset.id,datasetVersion:dataset.version,datasetHash:dataset.contentHash,models:previous.originalModels,maxOutputTokens,price:null,pythonImage:environment.ImageId,caseIds,repetitions:1,releaseModels:[]}});
  batchId=plan.id;expect(plan.items).toHaveLength(expectedCount);
  await save('plan.json',plan);await save('environment.json',{...environment,catalog,dataset,models:previous.originalModels,oldBatchId:previous.originalBatchId,oldPlanHash:previous.originalPlanHash,scope:'new isolated SQLite; same model/case identities; local only'});
  await nativeInvoke(page,'start_standard_benchmark',{workspaceId,batchId,approvedPlanHash:plan.planHash});
  let signature='',report:Batch|undefined;
  const deadline=Date.now()+2_400_000;
  while(Date.now()<deadline){
   const overview=await nativeInvoke<Batch>(page,'get_benchmark_overview',{workspaceId,batchId,scope:'standard'});
   const progress={batchId,status:overview.status,models:overview.reports?.map(r=>({name:r.name,statistics:r.statistics}))};
   if(JSON.stringify(progress)!==signature){signature=JSON.stringify(progress);console.log(signature);await save('progress.json',progress);}
   if(!['running','stopping'].includes(overview.status)){report=await nativeInvoke<Batch>(page,'get_standard_benchmark',{workspaceId,batchId});break;}
   await page.waitForTimeout(3000);
  }
  expect(report?.status).toBe('completed');await save('report.json',report);
  expect(report!.items).toHaveLength(expectedCount);expect(report!.items!.every(i=>i.status==='finished'&&i.runId)).toBe(true);
  const details:unknown[]=[],links:unknown[]=[];
  for(const item of report!.items!){
   const detail=await nativeInvoke(page,'get_benchmark_run',{workspaceId,experimentId:item.runId});details.push(detail);
   const model=previous.originalModels[item.modelIndex].local.model,old=previous.blockedItems.find(b=>b.model===model&&b.caseId===item.caseId)!;
   expect(old).toBeTruthy();expect(item.runId).not.toBe(old.runId);
   links.push({model,caseId:item.caseId,caseHash:old.caseHash,previousRunId:old.runId,newRunId:item.runId});
  }
  await save('details.json',details);await save('links.json',{originalReport:previous.originalReport,originalReportSHA256:previous.originalReportSHA256,oldBatchId:previous.originalBatchId,newBatchId:batchId,links});
  expect(await originalHash()).toBe(previous.originalReportSHA256);
  expect(await nativeInvoke(page,'list_conversations',{workspaceId})).toEqual([]);
  expect((await nativeInvoke<{settings:unknown}>(page,'bootstrap')).settings).toEqual(before.settings);
  await page.evaluate(({workspaceId,batchId})=>{location.hash=`/benchmark?workspace=${workspaceId}&batch=${batchId}`;},{workspaceId,batchId});await page.reload();
  await expect(page.getByRole('button',{name:'导出报告',exact:true})).toBeVisible();
  await page.setViewportSize({width:1600,height:900});await page.screenshot({path:info.outputPath('python-rerun-report.png')});
  await save('acceptance.json',{workspaceId,batchId,oldBatchId:previous.originalBatchId,planned:expectedCount,maxOutputTokens,historicalReportUnchanged:true,ordinaryChatHistoryEmpty:true,settingsUnchanged:true,pythonImage:environment.ImageId,statistics:report!.reports?.map(r=>({model:r.name,statistics:r.statistics})),reportOnly:'Core60 v2 Python smoke; raw statuses retained; no claim of model ranking'});
  expect(app.consoleErrors).toEqual([]);
 }finally{
  if(workspaceId&&batchId)await nativeInvoke(app.page,'stop_standard_benchmark',{workspaceId,batchId}).catch(()=>{});
  await app.stop();
 }
});
