import { expect, test, type TestInfo } from '@playwright/test';
import { createServer } from 'node:http';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { launchNative, nativeInvoke } from './native';

const exec=promisify(execFile);
const executable=process.env.CORTEXOS_D4_EXE??resolve('../../artifacts/cortexos-harness-experiment.exe');
const workspaceId='6094f530-c6ad-4bbd-9867-692cb6d0e713';
const assessmentVersion=1;
const model='controlled-protocol-fixture';
const digest='a'.repeat(64);
const actualModel='qwen3:4b-instruct';
const actualDigest='0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0';
const secondModel='qwen3:4b';
const secondDigest='359d7dd4bcdab3d86b87d73ac27966f4dbb9f5efdfcc75d34a8764a09474fae7';
const secondFixtureModel='second-controlled-fixture';

const entry='def adjust_total(n):\n    return transform(n)\n';
const referenceEntry='def adjust_total(n):\n    return transform(n) * 2\n';
const fixEntry=`from project_math import transform\n\n${referenceEntry}`;
const fixMath='def transform(n):\n    return max(0, n)\n';
const projectFiles={
  'project_entry.py':`from project_math import transform\n\n${entry}`,
  'project_math.py':'from project_base import identity\n\ndef transform(n):\n    return identity(n)\n',
  'project_base.py':'def identity(n):\n    return n\n',
  'old_schema.sql':'CREATE TABLE items(value INTEGER);\n',
  'schema_check.py':"from pathlib import Path\nimport sqlite3\n\ndef read_value(n):\n    with sqlite3.connect(':memory:') as db:\n        db.executescript(Path(__file__).with_name('old_schema.sql').read_text())\n        db.execute('INSERT INTO items VALUES (?)', (n,))\n        return db.execute('SELECT value FROM items').fetchone()[0]\n",
  'sqlite_entry.py':'from schema_check import read_value\n\ndef check_legacy(n):\n    return read_value(n)\n',
  'test_values.py':'def verifies(implementation, n):\n    return True\n',
  'test_entry.py':'from test_values import verifies\n\ndef detects_bad_test(n):\n    return verifies(abs if n < 0 else int, n)\n',
};
const fixProject=`project_entry.py\n\`\`\`python\n${fixEntry}\`\`\`\n\nproject_math.py\n\`\`\`python\n${fixMath}\`\`\``;

type Check={id:string;datasetId:string;hash:string;harness:string;function:string;datasetVersion:number;taskKey:string};
type Listing={history:{id:string;status:string}[];options:Check[]};
type Report={id:string;status:string;error:string|null;plan:any;summary:any[];rows:any[];comparison:any};
const list=(app:Awaited<ReturnType<typeof launchNative>>)=>nativeInvoke<Listing>(app.page,'list_harness_experiments',{workspaceId,assessmentVersion});
const report=(app:Awaited<ReturnType<typeof launchNative>>,experimentId:string)=>nativeInvoke<Report>(app.page,'get_harness_experiment',{workspaceId,experimentId});
const trace=(app:Awaited<ReturnType<typeof launchNative>>,experimentId:string,runId:string)=>nativeInvoke<any>(app.page,'get_harness_experiment_trace',{workspaceId,experimentId,runId});
const selection=(o:Check,name=model,revision=digest)=>({runtimeId:o.id,runtimeHash:o.hash,model:name,digest:revision,weight:3});

async function prepare(info:TestInfo) {
  const source=resolve('../../artifacts/harness-aider-native-final');
  const external=process.env.CORTEXOS_D4_EXPORT_SOURCE??process.env.CORTEXOS_D4_PROJECT_SOURCE;
  const fixture=external?undefined:(await readdir(source,{withFileTypes:true})).find(d=>d.isDirectory()&&d.name.startsWith('harness-aider-runtime-Aide'));
  expect(external||fixture).toBeTruthy();
  const data=info.outputPath('data');await mkdir(data,{recursive:true});
  await exec('py',['-3.13','-c','import sqlite3,sys; a=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True); b=sqlite3.connect(sys.argv[2]); a.backup(b); b.close(); a.close()',external??join(source,fixture!.name,'data/cortexos.sqlite3'),join(data,'cortexos.sqlite3')],{windowsHide:true});
  return data;
}
async function open(app:Awaited<ReturnType<typeof launchNative>>) {
  await app.page.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},workspaceId);
  await app.page.reload();
  await app.page.getByRole('button',{name:'5. 组合实验',exact:true}).click();
}
function comparisonChecks(options:Check[]) {
  const tools=['codex','claude-code','aider'];
  const shared=options.find(o=>o.datasetId==='harness-python-03d419ce-ce1f-462b-b237-8fb8d0c5749a'&&o.datasetVersion===2);
  expect(shared).toBeTruthy();
  const checks=tools.map(tool=>options.find(o=>o.harness===tool&&o.taskKey===shared!.taskKey)!);
  expect(checks.every(Boolean),JSON.stringify(options.map(({harness,datasetId,datasetVersion})=>({harness,datasetId,datasetVersion})))).toBe(true);
  expect(new Set(checks.map(o=>o.taskKey)).size).toBe(1);
  return checks;
}
async function wait(app:Awaited<ReturnType<typeof launchNative>>,id:string,timeout=120000) {
  await expect.poll(async()=>(await report(app,id)).status,{timeout,intervals:[250,500,1000]}).not.toBe('running');
  return report(app,id);
}

test('G5 three tool queue isolates cancellation and recovers a forced exit', async ({}, info) => {
  test.setTimeout(140_000);
  const data = await prepare(info);
  const tools = ['codex', 'claude', 'aider'] as const;
  type Tool = typeof tools[number];
  const calls: string[] = [];
  let awaitingHolder = false;
  let releaseHolder!: () => void;
  const held = new Promise<void>(done => { releaseHolder = done; });
  const details = { format: 'gguf', family: 'qwen3' };
  const server = createServer(async (req, res) => {
    let raw = '';
    for await (const chunk of req) raw += chunk;
    const body = JSON.parse(raw || '{}');
    const reply = (value: unknown) => res.end(JSON.stringify(value) + '\n');
    res.setHeader('Content-Type', 'application/json');
    if (req.url === '/api/version') return reply({ version: 'g5-fixed' });
    if (req.url === '/api/tags') return reply({ models: ['holder', ...tools].map(name => ({name, digest, size:1, details})) });
    if (req.url === '/api/ps') return reply({ models: [] });
    if (req.url === '/api/show') return reply({ details, capabilities:['completion','tools'], model_info:{'general.architecture':'qwen3','qwen3.context_length':8192} });
    if (req.url === '/api/chat') {
      calls.push(body.model);
      if (body.model === 'holder') {
        res.setHeader('Content-Type', 'application/x-ndjson');
        res.write(JSON.stringify({message:{role:'assistant',content:'9'},done:false})+'\n');
        awaitingHolder = true;
        await held;
        return reply({message:{role:'assistant',content:''},done:true,done_reason:'stop',prompt_eval_count:5,eval_count:1});
      }
      if (body.model === 'claude') { res.statusCode=503; return reply({error:'controlled unavailable'}); }
      return; // Stop or forced exit interrupts the outstanding fixture response.
    }
    res.writeHead(404).end();
  });
  await new Promise<void>(done => server.listen(0,'127.0.0.1',done));
  let app: Awaited<ReturnType<typeof launchNative>> | undefined;
  try {
    const address = server.address();
    if (!address || typeof address === 'string') throw new Error('Fixture port unavailable');
    const endpoint = `http://127.0.0.1:${address.port}`;
    app = await launchNative(info,{dataDirectory:data,endpoint,executable});
    const page = app.page;
    const checks = comparisonChecks((await list(app)).options);
    const {stdout}=await exec('py',['-3.13','-c','import sqlite3,sys; c=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True); print(c.execute("SELECT json_extract(plan_json,\'$.checkId\') FROM benchmark_generation_jobs WHERE id=?", [sys.argv[2]]).fetchone()[0])',join(data,'cortexos.sqlite3'),checks[0].id],{windowsHide:true});
    const checkId = stdout.trim();
    const scope = (tool: Tool) => ({workspaceId,assessmentVersion,...(tool==='codex'?{checkId}:{})});
    const state = (tool: Tool, jobId?: string) => nativeInvoke<any>(app!.page,`get_harness_${tool}_session`,{...scope(tool),jobId});
    const cancel = (tool: Tool, jobId: string, onlyWaiting: boolean) => nativeInvoke<any>(app!.page,`cleanup_harness_${tool}_session`,{workspaceId,jobId,onlyWaiting});
    const datasets = await nativeInvoke<{id:string;version:number;contentHash:string}[]>(page,'list_benchmark_datasets',{workspaceId});
    const dataset = datasets.find(d => d.id==='cortex-core-60'&&d.version===2)!;
    const holder = await nativeInvoke<{id:string;planHash:string}>(page,'plan_standard_benchmark',{workspaceId,input:{
      datasetId:dataset.id,datasetVersion:2,datasetHash:dataset.contentHash,repetitions:1,caseIds:['math-001'],maxOutputTokens:64,releaseModels:[],
      models:[{kind:'local',local:{model:'holder',expectedDigest:digest,requirements:['completion'],parameters:{numCtx:4096,numPredict:64,timeoutSeconds:120,temperature:0}}}],
    }});
    await nativeInvoke(page,'start_standard_benchmark',{workspaceId,batchId:holder.id,approvedPlanHash:holder.planHash});
    await expect.poll(()=>awaitingHolder,{timeout:15000}).toBe(true);
    function enqueue(tool: Tool) {
      const runtime = checks[tools.indexOf(tool)];
      return nativeInvoke<{id:string;error?:string}>(app!.page,`start_harness_${tool}_session`,{workspaceId,waitForResources:true,input:{
        assessmentVersion,...(tool==='codex'?{checkId}:{}),runtimeId:runtime.id,runtimeHash:runtime.hash,confirmed:true,
        model:{model:tool,expectedDigest:digest,requirements:tool==='aider'?['completion']:['completion','tool_calls'],parameters:{numCtx:4096,numPredict:512,temperature:0,timeoutSeconds:120}},
      }}).catch(e=>({id:'',error:String(e)}));
    }
    async function waiting(tool: Tool) {
      await expect.poll(async()=>(await state(tool)).request?.status).toBe('waiting');
      return (await state(tool)).request.id as string;
    }
    const codex = enqueue('codex');
    const codexId = await waiting('codex');
    const cancelled = enqueue('claude');
    const cancelledId = await waiting('claude');
    await expect(cancel('aider',cancelledId,true)).rejects.toThrow('不在等待队列');
    await cancel('claude',cancelledId,true);
    await expect(cancelled).resolves.toHaveProperty('error',expect.stringContaining('已取消等待'));
    await expect(state('claude',cancelledId)).rejects.toBeTruthy();
    const claude = enqueue('claude'); const claudeId = await waiting('claude');
    const aider = enqueue('aider'); const aiderId = await waiting('aider');
    expect(calls).toEqual(['holder']);
    releaseHolder();
    await expect(codex).resolves.toMatchObject({id:codexId});
    await expect.poll(()=>calls,{timeout:30000}).toEqual(['holder','codex']);
    await cancel('codex',codexId,false);
    await expect(claude).resolves.toMatchObject({id:claudeId});
    await expect(aider).resolves.toMatchObject({id:aiderId});
    await expect.poll(()=>calls,{timeout:30000}).toEqual(['holder','codex','claude','aider']);
    const stopped = (await state('codex',codexId)).selected;
    const failed = (await state('claude',claudeId)).selected;
    expect(stopped).toMatchObject({status:'cancelled',clean:true});
    expect(failed).toMatchObject({status:'failed',clean:true});
    expect(failed.error).toBeTruthy();
    const queuedAtExit = enqueue('codex'); const queuedId = await waiting('codex');
    await exec('taskkill',['/PID',String(app.processId),'/T','/F'],{windowsHide:true});
    await app.stop();
    await queuedAtExit;
    app = await launchNative(info,{dataDirectory:data,endpoint,executable,profileDirectory:info.outputPath('reopened')});
    const recovered = (await state('aider',aiderId)).selected;
    expect(recovered.status).toBe('interrupted');
    await expect(state('codex',queuedId)).rejects.toBeTruthy();
    await cancel('aider',aiderId,false);
    expect((await state('aider',aiderId)).selected).toMatchObject({status:'closed',clean:true,pendingGraders:[]});
    expect((await state('codex',codexId)).selected.status).toBe('cancelled');
    expect((await state('claude',claudeId)).selected.error).toBe(failed.error);
    expect(calls).toEqual(['holder','codex','claude','aider']);
    await writeFile(info.outputPath('queue-evidence.json'),JSON.stringify({calls,codex:stopped.status,claude:failed.error,aider:recovered.status,cancelledWaitSaved:false,replayed:false},null,2));
  } finally {
    releaseHolder();
    await app?.stop();
    server.closeAllConnections();
    await new Promise<void>(done=>server.close(()=>done()));
  }
});

async function authorProject(app:Awaited<ReturnType<typeof launchNative>>, info:TestInfo) {
  const page=app.page;
  const zip=info.outputPath('project.zip');
  await exec('py',['-3.13','-X','utf8','-c','import sys,json,zipfile\nwith zipfile.ZipFile(sys.argv[1],"w") as z:\n for path,content in json.loads(sys.argv[2]).items(): z.writestr(path,content)',zip,JSON.stringify(projectFiles)],{windowsHide:true});
  await page.getByRole('button',{name:'4. 测试任务',exact:true}).click();
  const generator=page.getByText('生成候选任务',{exact:true});
  if(!await generator.evaluate(node=>(node.parentElement as HTMLDetailsElement).open))await generator.click();
  await page.getByText('添加代码文件定位任务',{exact:true}).click();
  await page.getByRole('button',{name:'选择项目ZIP',exact:true}).click();
  await exec('pwsh.exe',['-NoProfile','-File',resolve('tests/save-report-dialog.ps1'),'-AppProcessId',String(app.processId),'-DialogTitle','选择项目ZIP','-OutputPath',zip],{windowsHide:true,timeout:25000});
  await expect.poll(async()=>await page.getByRole('combobox',{name:'任务项目快照',exact:true}).inputValue()).not.toBe('');
  const repositoryId=await page.getByRole('combobox',{name:'任务项目快照',exact:true}).inputValue();
  const scenario=(kind:string,symbols:string[]=[])=>({kind,symbols,path:'project_entry.py',start:3,end:4});
  const input={assessmentVersion,repositoryId,requirementIds:[],locations:[],scenarios:[scenario('call_chain',['project_math.transform','project_base.identity']),scenario('tool_call'),scenario('permission_review')]};
  const draft=await nativeInvoke<any>(page,'create_harness_tasks',{workspaceId,input});
  expect(draft.candidates).toHaveLength(3);
  for(const item of draft.candidates)expect(item.validation.valid,JSON.stringify(item.validation)).toBe(true);
  const published=await nativeInvoke<any>(page,'publish_harness_tasks',{workspaceId,jobId:draft.id,selection:draft.candidates.map((row:any)=>({id:row.id,revision:row.revision,approved:true})),reviewer:'Phase D 固定验收',expectedVersion:0});
  expect(published.tasks[0].task.sources).toHaveLength(3);
  await expect(nativeInvoke(page,'create_harness_tasks',{workspaceId,input:{...input,scenarios:[scenario('call_chain',['project_base.identity','project_math.transform'])]}})).rejects.toBeTruthy();
  const job=await nativeInvoke<any>(page,'save_harness_programming_draft',{workspaceId,input:{jobId:null,expectedRevision:0,origin:{assessmentVersion,repositoryId,path:'project_entry.py',start:3,end:4},spec:{basis:'confirmed_requirement',rationale:'固定验收项目，入口负责倍增，模块负责将负数归零。',prompt:'In project_entry.py, double the result of transform. In project_math.py, clamp negatives to zero. Keep names and signatures. Leave project_base.py unchanged.',function:'adjust_total',parameters:[{name:'n',kind:'integer'}],returnType:'integer',examples:[-3,-2,-1,0,1,2,3].map(n=>({label:`输入 ${n}`,role:n===0?'regression':n===1?'boundary':'target',args:[n],output:2*Math.max(0,n)})),referenceCode:'',wrongCode:'',project:{kind:'multi_file',files:['project_entry.py','project_math.py','project_base.py'],reference:{'project_entry.py':fixEntry,'project_math.py':fixMath},negative:{'project_entry.py':fixEntry,'project_math.py':'def transform(n):\n    return abs(n)\n'}}}}});
  return {job,published,datasetId:`harness-${draft.id}`,repositoryId};
}

test('project tasks and native tool requests use real Docker and fixed responses',async({},info)=>{
  test.setTimeout(370000);
  const data=await prepare(info);
  const calls:any[]=[];
  let badEdit=false;
  const server=createServer(async(req,res)=>{
    let raw='';for await(const chunk of req)raw+=chunk;
    const body=raw?JSON.parse(raw):{};
    const send=(value:any)=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify(value));};
    if(req.url==='/api/version')return send({version:'0.40.9'});
    if(req.url==='/api/ps')return send({models:[]});
    if(req.url==='/api/tags')return send({models:[model,secondFixtureModel].map((name,i)=>({name,digest:i?'b'.repeat(64):digest,size:1,details:{format:'gguf'}}))});
    if(req.url==='/api/show')return send({details:{format:'gguf'},capabilities:['completion','tools'],model_info:{'general.architecture':'qwen3','qwen3.context_length':4096}});
    if(req.url!=='/api/chat'){res.statusCode=404;return send({error:'unexpected_path'});}
    calls.push(body);
    const name=body.tools?.[0]?.function?.name;
    const tool=['locate_source','request_review'].includes(name);
    const containsProject=JSON.stringify(body.messages).includes('adjust_total');
    let message:any={role:'assistant',content:containsProject?fixProject:'value.py\n```python\ndef adjust(n):\n    return max(0, n)\n```'};
    if(badEdit&&containsProject)message={role:'assistant',content:'project_base.py\n```python\ndef identity(n):\n    return 0\n```'};
    if(tool)message={role:'assistant',content:'',tool_calls:[{function:{name,arguments:{path:'project_entry.py',start:3,end:4}}}]};
    send({model:body.model,done:true,done_reason:'stop',message,prompt_eval_count:200,eval_count:40});
  });
  await new Promise<void>(done=>server.listen(0,'127.0.0.1',done));
  const address=server.address();if(!address||typeof address==='string')throw new Error('fixture address unavailable');
  const app=await launchNative(info,{dataDirectory:data,endpoint:`http://127.0.0.1:${address.port}`,executable});
  try{
    await open(app);
    const {job,published,datasetId,repositoryId}=await authorProject(app,info);
    const toolItem=published.tasks[1];
    await nativeInvoke(app.page,'run_harness_readonly_task',{workspaceId,input:{datasetId,version:1,contentHash:published.content_hash,caseId:toolItem.id,model,digest,confirmed:true}});
    const readOnly=()=>nativeInvoke<any[]>(app.page,'get_harness_readonly_runs',{workspaceId,datasetId,version:1});
    await expect.poll(async()=>(await readOnly())[0]?.result?.score.state).toBe('Passed');
    expect((await readOnly())[0].result.toolsExecuted).toBe(false);
    const images=await nativeInvoke<{images:{imageId:string}[]}>(app.page,'get_harness_python_environment',{});
    const imageId=images.images[0]?.imageId;expect(imageId).toBeTruthy();
    const {id:validationId}=await nativeInvoke<{id:string}>(app.page,'start_harness_programming_validation',{workspaceId,input:{draftId:job.id,expectedRevision:1,draftHash:job.revisions[0].contentHash,imageId}});
    const validationState=()=>nativeInvoke<any>(app.page,'get_harness_programming_validation',{workspaceId,draftId:job.id,validationId});
    await expect.poll(async()=>(await validationState()).selected.status,{timeout:160000,intervals:[500,1000]}).not.toBe('validating');
    const validation=(await validationState()).selected;
    await writeFile(info.outputPath('validation.json'),JSON.stringify({repositoryId,validation},null,2));
    expect(validation.publishable,JSON.stringify(validation.comparison)).toBe(true);
    expect(validation.records.reference.score.details.buildStatus).toBe('passed');
    expect(validation.records.reference.score.metrics.assertionsPassed).toBe(7);
    expect(validation.records.unchanged.score.state).toBe('Failed');
    expect(validation.pendingContainers).toEqual([]);
    const {content_hash}=await nativeInvoke<any>(app.page,'publish_harness_programming_task',{workspaceId,input:{draftId:job.id,expectedRevision:1,validationId,validationHash:validation.contentHash,reviewer:'Phase D 固定验收',approved:true,expectedVersion:0}});
    const projectDatasetId=`harness-python-${job.id}`;
    const known=(await list(app)).options.find(o=>o.harness==='aider'&&o.datasetVersion===2&&o.function==='adjust');
    expect(known).toBeTruthy();
    const runtimeListing=await nativeInvoke<any>(app.page,'get_harness_aider_images',{workspaceId});
    const {id:runtimeId}=await nativeInvoke<{id:string}>(app.page,'check_harness_aider_runtime',{workspaceId,input:{assessmentVersion,datasetId:projectDatasetId,version:1,datasetHash:content_hash,imageId:runtimeListing.images[0].imageId,confirmed:true}});
    await expect.poll(async()=>(await nativeInvoke<any>(app.page,'get_harness_aider_runtime',{workspaceId,assessmentVersion,jobId:runtimeId})).selected?.status,{timeout:50000}).toBe('completed');
    const items=(await list(app)).options;
    const project=items.find(o=>o.harness==='aider'&&o.datasetId===projectDatasetId);
    expect(project).toBeTruthy();
    await writeFile(info.outputPath('ready.json'),JSON.stringify({projectDatasetId,projectVersion:1,singleDatasetId:known!.datasetId},null,2));
    await open(app);
    const {id}=await nativeInvoke<{id:string}>(app.page,'start_harness_experiment',{workspaceId,input:{assessmentVersion,repetitions:1,confirmed:true,selections:[selection(known!),selection(project!),selection(project!,secondFixtureModel,'b'.repeat(64))]}});
    const comparison=await wait(app,id,140000);
    await writeFile(info.outputPath('controlled-comparison.json'),JSON.stringify(comparison,null,2));
    expect(comparison.rows).toHaveLength(3);
    expect(comparison.rows.every(row=>row.passed)).toBe(true);
    expect(comparison.rows.filter(row=>row.buildStatus==='passed')).toHaveLength(2);
    for(const row of comparison.rows.filter(row=>row.buildStatus==='passed')){
      const run=await trace(app,id,row.id);
      expect(run.events.find((event:any)=>event.kind==='candidate').data.patch.files.map((file:any)=>file.path)).toEqual(['project_entry.py','project_math.py']);
      expect(run.clean).toBe(true);expect(run.pendingGraders).toEqual([]);
    }
    badEdit=true;
    const {id:denied}=await nativeInvoke<{id:string}>(app.page,'start_harness_aider_session',{workspaceId,input:{assessmentVersion,runtimeId:project!.id,runtimeHash:project!.hash,model:{model,expectedDigest:digest,parameters:{numCtx:4096,numPredict:512,temperature:0,timeoutSeconds:120},requirements:['completion']},confirmed:true}});
    const deniedRecord=async()=>(await nativeInvoke<any>(app.page,'get_harness_aider_session',{workspaceId,assessmentVersion,jobId:denied})).selected;
    await expect.poll(async()=>(await deniedRecord()).status,{timeout:50000}).toBe('failed');
    const blocked=await deniedRecord();expect(blocked.error).toBe('aider_edit_outside_format');expect(blocked.clean).toBe(true);expect(blocked.events.some((event:any)=>event.kind==='candidate')).toBe(false);
    await writeFile(info.outputPath('read-only-and-denied.json'),JSON.stringify({runs:await readOnly(),blocked},null,2));
    expect(calls.every(call=>!JSON.stringify(call).includes('referenceCode'))).toBe(true);
    await app.page.getByRole('button',{name:'刷新检查与记录',exact:true}).click();
    await expect(app.page.getByRole('combobox',{name:'组合图表指标'})).toBeVisible();
    await app.page.getByRole('combobox',{name:'组合图表指标'}).selectOption('time');
    for(const theme of ['light','dark']){
      await app.page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);
      for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
        await app.page.setViewportSize({width,height});
        await app.page.locator('.harness-comparison-chart').scrollIntoViewIfNeeded();
        expect(await app.page.locator('.fit-scroll').evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
        await app.page.screenshot({path:info.outputPath(`comparison-${theme}-${width}.png`)});
      }
    }
  }finally{await app.stop();server.closeAllConnections();await new Promise<void>(done=>server.close(()=>done()));}
});

test('project regression, SQLite compatibility and test authorship use independent controls',async({},info)=>{
  test.setTimeout(420000);
  const data=await prepare(info);
  const app=await launchNative(info,{dataDirectory:data,executable});
  try{
    await open(app);
    const {repositoryId}=await authorProject(app,info);
    const imageId=(await nativeInvoke<any>(app.page,'get_harness_python_environment')).images[0].imageId;
    const originalSqlEntry=projectFiles['sqlite_entry.py'];
    const changes=[
      {kind:'regression',path:'project_entry.py',function:'adjust_total',files:['project_entry.py','project_math.py','project_base.py'],editPath:'project_math.py',reference:fixMath,negative:'def transform(n):\n    return abs(n)',output:(n:number)=>Math.max(0,n),returnType:'integer'},
      {kind:'database_compatibility',path:'sqlite_entry.py',function:'check_legacy',files:['sqlite_entry.py','schema_check.py','old_schema.sql'],editPath:'sqlite_entry.py',reference:originalSqlEntry.replace('return read_value(n)','return max(0, read_value(n))'),negative:originalSqlEntry.replace('return read_value(n)','return abs(read_value(n))'),output:(n:number)=>Math.max(0,n),returnType:'integer'},
      {kind:'add_tests',path:'test_entry.py',function:'detects_bad_test',files:['test_entry.py','test_values.py'],editPath:'test_values.py',reference:'def verifies(implementation, n):\n    return implementation(n) == n\n',negative:'def verifies(implementation, n):\n    return n != -2\n',output:(n:number)=>n>=0,returnType:'boolean'},
    ];
    const reports:unknown[]=[];
    for(const item of changes){
      const spec={basis:'reported_bug',rationale:'固定验收：实际复现目标失败，并保留非目标回归。',prompt:`Validate the ${item.kind} behavior defined by the reviewed repository.`,function:item.function,parameters:[{name:'n',kind:'integer'}],returnType:item.returnType,examples:[-3,-2,-1,0,1,2,3].map(n=>({label:`输入 ${n}`,role:n>0?'regression':n===0?'boundary':'target',args:[n],output:item.output(n)})),referenceCode:'',wrongCode:'',project:{kind:item.kind,files:item.files,reference:{[item.editPath]:item.reference},negative:{[item.editPath]:item.negative}}};
      const origin={assessmentVersion,repositoryId,path:item.path,start:3,end:4};
      const save=(draftSpec:object)=>nativeInvoke<any>(app.page,'save_harness_programming_draft',{workspaceId,input:{jobId:null,expectedRevision:0,origin,spec:draftSpec}});
      const job=await save(spec);
      if(item.kind==='add_tests')await expect(save({...spec,project:{...spec.project,reference:{[item.path]:item.reference},negative:{[item.path]:item.negative}}})).rejects.toBeTruthy();
      const {id}=await nativeInvoke<{id:string}>(app.page,'start_harness_programming_validation',{workspaceId,input:{draftId:job.id,expectedRevision:1,draftHash:job.revisions[0].contentHash,imageId}});
      const record=async()=>(await nativeInvoke<any>(app.page,'get_harness_programming_validation',{workspaceId,draftId:job.id,validationId:id})).selected;
      await expect.poll(async()=>(await record()).status,{timeout:120000,intervals:[500,1000]}).not.toBe('validating');
      const validation=await record();
      reports.push({kind:item.kind,validation});
      await writeFile(info.outputPath('project-controls.json'),JSON.stringify(reports,null,2));
      expect(validation.publishable,JSON.stringify(validation.comparison)).toBe(true);
      expect(validation.records.reference.score.metrics.assertionsPassed).toBe(7);
      expect(validation.pendingContainers).toEqual([]);
    }
  } finally {await app.stop();}
});

test('fixed replies exercise the comparison, task graph and export',async({},info)=>{
  test.setTimeout(270000);
  const data=await prepare(info);
  const exportSource=process.env.CORTEXOS_D4_EXPORT_SOURCE;
  let hold=false;
  const requests:any[]=[];
  const server=createServer(async(req,res)=>{
    let raw='';for await(const chunk of req)raw+=chunk;
    const body=raw?JSON.parse(raw):{};
    const send=(value:any)=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify(value));};
    if(req.url==='/api/version')return send({version:'0.40.9'});
    if(req.url==='/api/ps')return send({models:[]});
    if(req.url==='/api/tags')return send({models:[{name:model,digest,size:1,details:{format:'gguf'}},{name:'no-tools-fixture',digest:'b'.repeat(64),size:1,details:{format:'gguf'}}]});
    if(req.url==='/api/show')return send({details:{format:'gguf'},capabilities:body.model==='no-tools-fixture'?['completion']:['completion','tools'],model_info:{'general.architecture':'qwen3','qwen3.context_length':4096}});
    if(req.url!=='/api/chat'){res.statusCode=404;return send({error:'unexpected_path'});}
    requests.push(body);
    if(hold)return;
    const reply='project/src/value.py\n```python\ndef adjust(n):\n    return max(0, n)\n```';
    send({model:body.model,done:true,done_reason:'stop',message:{role:'assistant',content:reply},prompt_eval_count:200,eval_count:40});
  });
  await new Promise<void>(done=>server.listen(0,'127.0.0.1',done));
  const address=server.address();if(!address||typeof address==='string')throw new Error('fixture address unavailable');
  const endpoint=`http://127.0.0.1:${address.port}`;
  let app=await launchNative(info,{dataDirectory:data,endpoint,executable});
  try {
    await open(app);
    const panel=app.page.getByRole('region',{name:'组合对照实验'});
    let id:string;
    let complete:Report;
    let checks:Check[];
    if(exportSource) {
      const listing=await list(app);
      id=listing.history.find(h=>h.status==='completed')!.id;
      await panel.getByRole('combobox',{name:'组合实验历史',exact:true}).selectOption(id);
      complete=await report(app,id);
      checks=comparisonChecks(listing.options);
    } else {
    const initial=await list(app);
    checks=comparisonChecks(initial.options);
    const reject=async(input:any)=>{
      let denied=false;
      try{await nativeInvoke(app.page,'start_harness_experiment',{workspaceId,input});}catch{denied=true;}
      expect(denied).toBe(true);
    };
    await reject({assessmentVersion,repetitions:1,confirmed:false,selections:checks.slice(1).map(o=>selection(o))});
    await reject({assessmentVersion,repetitions:1,confirmed:true,selections:[selection(checks[2]),selection(checks[2])]});
    await reject({assessmentVersion,repetitions:1,confirmed:true,selections:[selection(checks[1],'no-tools-fixture','b'.repeat(64)),selection(checks[2])]});
    expect(requests).toHaveLength(0);
    expect((await list(app)).history).toHaveLength(initial.history.length);
    await panel.getByRole('combobox',{name:'第 1 行工具与任务',exact:true}).selectOption(checks[2].id);
    await panel.getByRole('combobox',{name:'第 2 行工具与任务',exact:true}).selectOption(checks[2].id);
    await panel.getByRole('combobox',{name:'第 1 行模型',exact:true}).selectOption(model);
    await panel.getByRole('combobox',{name:'第 2 行模型',exact:true}).selectOption('no-tools-fixture');
    await panel.getByRole('combobox',{name:'第 1 行任务权重',exact:true}).selectOption('3');
    await panel.getByRole('combobox',{name:'实验重复次数',exact:true}).selectOption('2');
    await panel.getByRole('button',{name:'确认运行 4 个任务…',exact:true}).click();
    const dialog=app.page.getByRole('dialog',{name:'确认组合实验调用'});
    await dialog.getByRole('button',{name:'返回',exact:true}).click();
    expect(requests).toHaveLength(0);
    await panel.getByRole('button',{name:'确认运行 4 个任务…',exact:true}).click();
    await dialog.getByRole('button',{name:'确认调用并开始实验',exact:true}).click();
    await expect.poll(async()=>{
      const error=await panel.getByRole('alert').allTextContents();
      if(error.length)return error.join(' / ');
      return (await list(app)).history.length;
    },{timeout:25000}).toBe(initial.history.length+1);
    id=(await list(app)).history[0].id;
    complete=await wait(app,id);
    await writeFile(info.outputPath('comparison-observed.json'),JSON.stringify(complete,null,2));
    expect(complete.status,complete.error??'comparison completion').toBe('completed');
    expect(complete.plan.evidence).toBe('test_endpoint');
    expect(complete.comparison).toMatchObject({sameTasks:true,complete:true,comparable:false,suggested:null});
    expect(complete.rows).toHaveLength(4);
    for(const row of complete.rows){expect(row).toMatchObject({passed:true,weight:3,scoreState:'Passed',requests:1,clean:true,pendingGraders:[]});expect(row.assertions.assertionsPassed).toBe(7);}
    expect(requests).toHaveLength(4);
    for(const request of requests){expect(request.options).toEqual({num_ctx:4096,num_predict:512,temperature:0});expect(JSON.stringify(request)).not.toContain('referenceCode');expect(JSON.stringify(request)).not.toContain('assertions');}
    }
    await expect(panel.getByText('测试端点记录',{exact:true})).toBeVisible();
    await panel.getByRole('button',{name:'查看执行过程',exact:true}).first().click();
    const replay=app.page.getByRole('dialog',{name:'任务执行过程'});
    await replay.getByRole('button',{name:'保存候选修改',exact:true}).click();
    await expect(replay.getByLabel('选中事件详情')).toContainText('return max(0, n)');
    await replay.getByRole('button',{name:'打开 3D 图谱',exact:true}).click();
    await expect(replay.getByLabel('实验任务 3D 执行图')).toBeVisible();
    await expect(replay.locator('canvas')).toBeVisible();
    await app.page.screenshot({path:info.outputPath('trace.png')});
    await replay.getByRole('button',{name:'关闭任务执行过程'}).click();
    for(const format of ['JSON','CSV']){
      const file=info.outputPath(`comparison.${format.toLowerCase()}`);
      await panel.getByRole('button',{name:`导出 ${format}`,exact:true}).click();
      await exec('pwsh.exe',['-NoProfile','-File',resolve('tests/save-report-dialog.ps1'),'-AppProcessId',String(app.processId),'-DialogTitle','保存组合实验报告','-OutputPath',file],{windowsHide:true,timeout:25000});
      await expect.poll(async()=>readFile(file,'utf8').catch(()=>''),{timeout:10000}).toContain(format==='JSON'?'cortexos.harness-experiment':'experiment_id');
      if(format==='JSON')expect(JSON.parse(await readFile(file,'utf8')).payload.traces).toHaveLength(4);
    }
    for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
      for(const theme of ['light','dark']){
        await app.page.setViewportSize({width,height});
        await app.page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);
        await expect(app.page.locator('html')).toHaveAttribute('data-color-scheme',theme);
        expect(await app.page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);
        await app.page.screenshot({path:info.outputPath(`comparison-${theme}-${width}.png`)});
      }
    }
    if(exportSource) { await writeFile(info.outputPath('export-verified.json'),JSON.stringify(complete,null,2));return; }
    hold=true;
    const payload={workspaceId,input:{assessmentVersion,repetitions:1,confirmed:true,selections:[selection(checks[2]),selection(checks[2],'no-tools-fixture','b'.repeat(64))]}};
    const stop=(await nativeInvoke<{id:string}>(app.page,'start_harness_experiment',payload)).id;
    await expect.poll(()=>requests.length,{timeout:25000}).toBe(5);
    await nativeInvoke(app.page,'stop_harness_experiment',{workspaceId,experimentId:stop});
    const stopped=await wait(app,stop);
    expect(stopped.status).toBe('cancelled');expect(stopped.rows.map(r=>r.status)).toEqual(['cancelled','not_run']);
    const restart=(await nativeInvoke<{id:string}>(app.page,'start_harness_experiment',payload)).id;
    await expect.poll(()=>requests.length,{timeout:25000}).toBe(6);
    await exec('taskkill',['/PID',String(app.processId),'/T','/F'],{windowsHide:true});await app.stop();
    app=await launchNative(info,{dataDirectory:data,endpoint,executable,profileDirectory:info.outputPath('reopen')});
    const interrupted=await report(app,restart);
    expect(interrupted.status).toBe('interrupted');expect(interrupted.rows.map(r=>r.status)).toEqual(['interrupted','not_run']);
    await nativeInvoke(app.page,'cleanup_harness_aider_session',{workspaceId,jobId:interrupted.rows[0].id});
    const recovered=await report(app,restart);
    expect(recovered.rows[0]).toMatchObject({status:'closed',clean:true});expect(requests).toHaveLength(6);
    expect(await report(app,id)).toEqual(complete);
    const other=await nativeInvoke<{id:string}>(app.page,'create_workspace',{name:'comparison scope verification'});
    await expect(nativeInvoke(app.page,'get_harness_experiment',{workspaceId:other.id,experimentId:id})).rejects.toBeTruthy();
    await expect(trace(app,id,recovered.rows[0].id)).rejects.toBeTruthy();
    expect(app.consoleErrors).toEqual([]);
    await writeFile(info.outputPath('verified.json'),JSON.stringify({workspaceId,assessmentVersion,complete,stopped,recovered,actualModelCalls:0},null,2));
  } finally {
    try{await app.closeFromUi();}catch{}await app.stop();
    server.closeAllConnections();await new Promise<void>(done=>server.close(()=>done()));
  }
});

test('one authorized local comparison across three tools',async({},info)=>{
  test.skip(process.env.CORTEXOS_D4_APPROVAL!=='three-adjust-tasks','Requires explicit approval for three actual tasks.');
  test.setTimeout(840000);
  const data=await prepare(info);
  const app=await launchNative(info,{dataDirectory:data,executable});
  try{
    await open(app);
    const checks=comparisonChecks((await list(app)).options);
    const {id}=await nativeInvoke<{id:string}>(app.page,'start_harness_experiment',{workspaceId,input:{assessmentVersion,repetitions:1,confirmed:true,selections:checks.map(o=>selection(o,actualModel,actualDigest))}});
    const actual=await wait(app,id,780000);
    const traces=await Promise.all(actual.rows.filter(r=>r.recorded).map(r=>trace(app,id,r.id)));
    await writeFile(info.outputPath('actual-comparison.json'),JSON.stringify({workspaceId,assessmentVersion,actual,traces},null,2));
    expect(actual.plan.evidence).toBe('local_model');
    for(const row of actual.rows){expect(row.requests).toBeLessThanOrEqual(4);if(row.recorded){expect(row.clean).toBe(true);expect(row.pendingGraders).toEqual([]);}}
    await app.page.getByRole('button',{name:'刷新检查与记录',exact:true}).click();
    await expect(app.page.getByText('本地模型实测',{exact:true})).toBeVisible();
    await app.page.screenshot({path:info.outputPath('actual-comparison.png')});
  } finally {try{await app.closeFromUi();}catch{}await app.stop();}
});

test('one authorized two-model two-task Aider comparison',async({},info)=>{
  const ready=process.env.CORTEXOS_D4_PROJECT_EVIDENCE;
  test.skip(process.env.CORTEXOS_D4_APPROVAL!=='two-model-two-task-aider'||!ready||!process.env.CORTEXOS_D4_PROJECT_SOURCE,'Requires fresh approval and previously verified project evidence.');
  test.setTimeout(1080000);
  const verified=JSON.parse(await readFile(ready!,'utf8'));
  const data=await prepare(info);
  const app=await launchNative(info,{dataDirectory:data,executable});
  try{
    await open(app);
    const options=(await list(app)).options.filter(o=>o.harness==='aider');
    const single=options.find(o=>o.datasetId===verified.singleDatasetId&&o.function==='adjust'&&o.datasetVersion===2);
    const project=options.find(o=>o.datasetId===verified.projectDatasetId&&o.function==='adjust_total'&&o.datasetVersion===verified.projectVersion);
    expect(single&&project).toBeTruthy();
    const {id}=await nativeInvoke<{id:string}>(app.page,'start_harness_experiment',{workspaceId,input:{assessmentVersion,repetitions:1,confirmed:true,selections:[selection(single!,actualModel,actualDigest),selection(project!,actualModel,actualDigest),selection(single!,secondModel,secondDigest),selection(project!,secondModel,secondDigest)]}});
    await writeFile(info.outputPath('started.json'),JSON.stringify({id,workspaceId,assessmentVersion},null,2));
    const actual=await wait(app,id,1000000);
    const traces=await Promise.all(actual.rows.filter(r=>r.recorded).map(r=>trace(app,id,r.id)));
    await writeFile(info.outputPath('actual-comparison.json'),JSON.stringify({workspaceId,assessmentVersion,actual,traces},null,2));
    expect(actual.plan.evidence).toBe('local_model');
    expect(actual.rows).toHaveLength(4);
    for(const row of actual.rows){expect(row.requests).toBeLessThanOrEqual(4);if(row.recorded){expect(row.clean).toBe(true);expect(row.pendingGraders).toEqual([]);}}
    await app.page.getByRole('button',{name:'刷新检查与记录',exact:true}).click();
    await app.page.getByRole('combobox',{name:'组合实验历史',exact:true}).selectOption(id);
    await expect(app.page.getByText('本地模型实测',{exact:true})).toBeVisible();
    await app.page.locator('.harness-comparison-chart').scrollIntoViewIfNeeded();
    await app.page.screenshot({path:info.outputPath('actual-comparison.png')});
  } finally {try{await app.closeFromUi();}catch{}await app.stop();}
});

test('reopen and export the stored two-model report without inference',async({},info)=>{
  const source=process.env.CORTEXOS_D4_PROJECT_SOURCE;
  const evidence=process.env.CORTEXOS_D4_PROJECT_EVIDENCE;
  test.skip(!source||!evidence,'Requires the stored isolated real report.');
  test.setTimeout(120000);
  const saved=JSON.parse(await readFile(evidence!,'utf8')).actual;
  const data=await prepare(info);
  const app=await launchNative(info,{dataDirectory:data,executable});
  try{
    await open(app);
    await app.page.getByRole('combobox',{name:'组合实验历史',exact:true}).selectOption(saved.id);
    await expect(app.page.getByText('本地模型实测',{exact:true})).toBeVisible();
    const before=await report(app,saved.id);
    expect(before.rows).toEqual(saved.rows);
    for(const format of ['JSON','CSV']){
      const file=info.outputPath(`D4-expanded.${format.toLowerCase()}`);
      await app.page.getByRole('button',{name:`导出 ${format}`,exact:true}).click();
      await exec('pwsh.exe',['-NoProfile','-File',resolve('tests/save-report-dialog.ps1'),'-AppProcessId',String(app.processId),'-DialogTitle','保存组合实验报告','-OutputPath',file],{windowsHide:true,timeout:25000});
      await expect.poll(async()=>readFile(file,'utf8').then(body=>body.length,()=>0)).toBeGreaterThan(0);
    }
    const exported=JSON.parse(await readFile(info.outputPath('D4-expanded.json'),'utf8'));
    expect(exported.payload.report.rows).toEqual(before.rows);
    expect(exported.payload.traces).toHaveLength(4);
    expect(await report(app,saved.id)).toEqual(before);
    await app.page.getByRole('combobox',{name:'组合图表指标'}).selectOption('success');
    await app.page.locator('.harness-comparison-chart').scrollIntoViewIfNeeded();
    await app.page.screenshot({path:info.outputPath('D4-expanded.png')});
  } finally { await app.stop(); }
});

test('Codex and Claude edit the approved multi-file snapshot with fixed replies',async({},info)=>{
  const ready=process.env.CORTEXOS_D4_PROJECT_EVIDENCE;
  test.skip(!ready||!process.env.CORTEXOS_D4_PROJECT_SOURCE,'Requires an already published isolated project fixture.');
  test.setTimeout(300000);
  const verified=JSON.parse(await readFile(ready!,'utf8'));
  const datasetId=verified.projectDatasetId;
  const data=await prepare(info);
  let deny=false;
  const calls=new Map<string,number>();
  const server=createServer(async(req,res)=>{
    let raw='';for await(const chunk of req)raw+=chunk;
    const body=raw?JSON.parse(raw):{};
    const send=(value:any)=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify(value));};
    if(req.url==='/api/version')return send({version:'0.40.9'});
    if(req.url==='/api/ps')return send({models:[]});
    if(req.url==='/api/tags')return send({models:[{name:model,digest,size:1,details:{format:'gguf'}}]});
    if(req.url==='/api/show')return send({details:{format:'gguf'},capabilities:['completion','tools'],model_info:{'general.architecture':'qwen3','qwen3.context_length':4096}});
    if(req.url!=='/api/chat'){res.statusCode=404;return send({error:'unexpected_path'});}
    const kind=body.tools?.[0]?.function?.name==='Bash'?'claude':'codex';
    const count=(calls.get(kind)??0)+1;calls.set(kind,count);
    const targets={'project_entry.py':fixEntry,'project_math.py':fixMath};
    const script=Object.entries(targets).map(([name,code])=>`Path(${JSON.stringify('/workspace/project/'+name)}).write_text(${JSON.stringify(code)})`).join('; ');
    const command=`python3 -I -S -c 'from pathlib import Path; ${script}'`;
    const tool=kind==='claude'?{name:'Bash',arguments:{command,...(deny?{dangerouslyDisableSandbox:true}:{})}}:{name:'exec_command',arguments:{cmd:command,...(deny?{sandbox_permissions:'require_escalated'}:{})}};
    const message=count===1?{role:'assistant',content:'',tool_calls:[{function:tool}]}:{role:'assistant',content:'Implemented the requested changes in the two approved files.'};
    send({model:body.model,done:true,done_reason:'stop',message,prompt_eval_count:200,eval_count:40});
  });
  await new Promise<void>(done=>server.listen(0,'127.0.0.1',done));
  const address=server.address();if(!address||typeof address==='string')throw new Error('fixture address unavailable');
  const app=await launchNative(info,{dataDirectory:data,endpoint:`http://127.0.0.1:${address.port}`,executable});
  try{
    const page=app.page;
    const set=await nativeInvoke<any>(page,'get_harness_programming_taskset',{workspaceId,datasetId,version:1});
    const input={assessmentVersion,datasetId,version:1,datasetHash:set.content_hash};
    const check=await nativeInvoke<any>(page,'check_harness_execution_preflight',{workspaceId,input});
    const checkScope={workspaceId,assessmentVersion,checkId:check.id};
    const point=await nativeInvoke<any>(page,'get_harness_execution_preflight',checkScope);
    const checks:Record<string,{id:string;hash:string}>={};
    for(const tool of ['codex','claude']){
      const images=await nativeInvoke<any>(page,`get_harness_${tool}_images`,{workspaceId});
      const imageId=images.images[0]?.imageId;expect(imageId).toBeTruthy();
      const scope=tool==='codex'?checkScope:{workspaceId,assessmentVersion};
      const binding=tool==='codex'?{assessmentVersion,checkId:check.id,checkHash:point.selected.contentHash}:input;
      const {id}=await nativeInvoke<{id:string}>(page,`check_harness_${tool}_runtime`,{workspaceId,input:{...binding,securityPolicy:`${tool}-bwrap-1`,imageId,confirmed:true}});
      const current=async()=>(await nativeInvoke<any>(page,`get_harness_${tool}_runtime`,{...scope,jobId:id})).selected;
      await expect.poll(async()=>(await current()).status,{timeout:60000}).not.toBe('checking');
      const result=await current();
      expect(result.status,JSON.stringify(result.events)).toBe('completed');
      checks[tool]={id,hash:result.planHash};
    }
    const options=(await list(app)).options;
    const items=['codex','claude-code'].map(tool=>options.find(item=>item.harness===tool&&item.datasetId===datasetId));
    expect(items.every(Boolean)).toBe(true);
    const start=async()=>nativeInvoke<{id:string}>(page,'start_harness_experiment',{workspaceId,input:{assessmentVersion,repetitions:1,confirmed:true,selections:items.map(item=>selection(item!))}});
    const {id}=await start();const completed=await wait(app,id,140000);
    await writeFile(info.outputPath('project-cli-comparison.json'),JSON.stringify(completed,null,2));
    expect(completed.rows.every(row=>row.passed&&row.buildStatus==='passed'&&row.assertions.assertionsPassed===7)).toBe(true);
    deny=true;calls.clear();
    const {id:negativeId}=await start();const denied=await wait(app,negativeId,140000);
    const traces=await Promise.all(denied.rows.map(row=>trace(app,negativeId,row.id)));
    await writeFile(info.outputPath('permission-denials.json'),JSON.stringify({denied,traces},null,2));
    expect(denied.rows.every(row=>row.error==='tool_approval_required'&&row.blockedActions>=1&&row.clean)).toBe(true);
    expect(traces.every(run=>run.events.some((event:any)=>event.kind==='permission_decision')&&!run.events.some((event:any)=>event.kind==='score'))).toBe(true);
  } finally {await app.stop();server.closeAllConnections();await new Promise<void>(done=>server.close(()=>done()));}
});
