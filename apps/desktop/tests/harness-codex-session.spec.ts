import { expect, test, type TestInfo } from '@playwright/test';
import { createServer } from 'node:http';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { launchNative, nativeInvoke } from './native';
const exec = promisify(execFile);
const source=resolve('../../artifacts/harness-codex-policy-native/harness-codex-runtime-Linu-f820c-oped-cleanup-without-models-harness-codex-runtime');
const firstReal=resolve('../../artifacts/harness-codex-session-real/harness-codex-session-one--469b9-ion-keeps-the-actual-result-harness-codex-session');
type RecordItem={id:string;status:string;clean:boolean;pendingGraders:string[];modelCalls:number;error:string|null;plan:any;events:{kind:string;data:any}[]};
type State={selected:RecordItem|null;history:RecordItem[]};
async function prepare(info:TestInfo){
 const fixture=JSON.parse(await readFile(join(source,'runtime.json'),'utf8'));
 const data=info.outputPath('data');await mkdir(data,{recursive:true});
 const backing=process.env.CORTEXOS_HARNESS_SESSION_LEGACY==='1'?firstReal:source;
 await exec('py',['-3.13','-c','import sqlite3,sys\ns=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True)\nd=sqlite3.connect(sys.argv[2])\ns.backup(d)\nd.close()\ns.close()',join(backing,'data/cortexos.sqlite3'),join(data,'cortexos.sqlite3')],{windowsHide:true});
 return {fixture,data,scope:{workspaceId:fixture.workspaceId,assessmentVersion:1,checkId:fixture.checkId}};
}
async function open(app:Awaited<ReturnType<typeof launchNative>>,scope:any,runtimeId:string){
 await app.page.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},scope.workspaceId);await app.page.reload();
 await app.page.getByRole('button',{name:'4. 测试任务',exact:true}).click();await app.page.getByText('Codex 执行前检查',{exact:true}).click();
 await app.page.getByRole('combobox',{name:'执行检查历史',exact:true}).selectOption(scope.checkId);await app.page.getByText('Codex 隔离检查',{exact:true}).click();
 await app.page.getByRole('combobox',{name:'Codex隔离检查历史',exact:true}).selectOption(runtimeId);await app.page.getByText('Codex 单题执行',{exact:true}).click();
}
const input=(fixture:any,model:string,digest:string)=>({assessmentVersion:1,checkId:fixture.checkId,runtimeId:fixture.completed.id,runtimeHash:fixture.completed.planHash,model:{model,expectedDigest:digest,parameters:{numCtx:4096,numPredict:512,temperature:0,timeoutSeconds:120},requirements:['completion','tool_calls']},confirmed:true});

test('controlled transport verifies actual CLI tools, budget, stop and restart without a model',async({},info)=>{
 test.setTimeout(240000);
 const {fixture,data,scope}=await prepare(info);
 const model='controlled-protocol-fixture',digest='a'.repeat(64);let mode='edit',calls=0;
 const requests:any[]=[];
 const server=createServer(async(req,res)=>{
  let raw='';for await(const c of req)raw+=c;const body=raw?JSON.parse(raw):{};
  const send=(data:any)=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify(data));};
  if(req.url==='/api/version')return send({version:'0.40.9'});
  if(req.url==='/api/ps')return send({models:[]});
  if(req.url==='/api/tags')return send({models:[{name:model,digest,size:1,details:{format:'gguf',family:'qwen3',parameter_size:'fixture',quantization_level:'fixture'}}]});
  if(req.url==='/api/show')return send({details:{format:'gguf'},capabilities:['completion','tools'],model_info:{'general.architecture':'qwen3','qwen3.context_length':4096}});
  if(req.url!=='/api/chat'){res.statusCode=404;return send({error:'unexpected_path'});}
  requests.push(body);calls++;
  if(mode==='hold')return;
  const command=mode==='budget'?'printf controlled-budget':`python3 -I -S -c 'from pathlib import Path; p=next(Path("/workspace/project").rglob("*.py")); p.write_text("def adjust(n):\\n    return max(0, n)\\n")'`;
  const message=mode==='edit'&&calls>1?{role:'assistant',content:'The existing target file has been updated.'}:{role:'assistant',content:'',tool_calls:[{function:{name:mode==='bad'?'unprovided_tool':'exec_command',arguments:{cmd:command,max_output_tokens:1000}}}]};
  send({done:true,done_reason:'stop',message,prompt_eval_count:200,eval_count:40});
 });
 await new Promise<void>(r=>server.listen(0,'127.0.0.1',r));const address=server.address();if(!address||typeof address==='string')throw new Error('fixture port');
 let app=await launchNative(info,{dataDirectory:data,endpoint:`http://127.0.0.1:${address.port}`}).catch(async error=>{server.closeAllConnections();await new Promise<void>(r=>server.close(()=>r()));throw error;});
 const state=(jobId:string|null=null)=>nativeInvoke<State>(app.page,'get_harness_codex_session',{...scope,jobId});
 const start=()=>nativeInvoke<{id:string}>(app.page,'start_harness_codex_session',{workspaceId:scope.workspaceId,input:input(fixture,model,digest)});
 const wait=async(id:string)=>{await expect.poll(async()=>(await state(id)).selected?.status,{timeout:60000,intervals:[300,700]}).not.toBe('running');return (await state(id)).selected!;};
 const ids:string[]=[];
 try{
  await open(app,scope,fixture.completed.id);await expect(app.page.getByRole('button',{name:'运行一个任务…',exact:true})).toBeEnabled();
  const initial=(await state()).history.length;
  const legacy=process.env.CORTEXOS_HARNESS_SESSION_LEGACY==='1'?JSON.parse(await readFile(join(firstReal,'actual-session.json'),'utf8')).actual:null;
  if(legacy)expect((await state(legacy.id)).selected).toEqual(legacy);
  const rejected=async(value:any)=>{let reason='';try{await nativeInvoke(app.page,'start_harness_codex_session',{workspaceId:scope.workspaceId,input:value});}catch(e){reason=String(e);}expect(reason).not.toBe('');};
  await rejected({...input(fixture,model,digest),confirmed:false});await rejected({...input(fixture,model,digest),runtimeHash:'forged'});
  const other=await nativeInvoke<{id:string}>(app.page,'create_workspace',{name:'isolated scope'});
  let denied=false;try{await nativeInvoke(app.page,'start_harness_codex_session',{workspaceId:other.id,input:input(fixture,model,digest)});}catch{denied=true;}expect(denied).toBe(true);expect((await state()).history).toHaveLength(initial);
  await app.page.getByRole('button',{name:'运行一个任务…',exact:true}).click();const dialog=app.page.getByRole('dialog',{name:'确认本次本地模型调用',exact:true});await expect(dialog).toBeVisible();expect(requests).toHaveLength(0);await dialog.getByRole('button',{name:'返回',exact:true}).click();expect((await state()).history).toHaveLength(initial);
  const id=(await start()).id;ids.push(id);const completed=await wait(id);
  await writeFile(info.outputPath('first-session.json'),JSON.stringify(completed,null,2));
  expect(completed.status,completed.error??'read first-session.json for evidence').toBe('completed');expect(completed.clean).toBe(true);expect(completed.pendingGraders).toEqual([]);expect(completed.modelCalls).toBe(2);
  const score=completed.events.find(e=>e.kind==='score')?.data.score;expect(score?.state,'read first-session.json for actual CLI and assertion evidence').toBe('Passed');expect(completed.events.some(e=>e.kind==='cli'&&e.data.parsed.item?.type==='command_execution')).toBe(true);
  for(const request of requests){expect(request.options).toEqual({num_ctx:4096,num_predict:512,temperature:0});expect(request.keep_alive).toBe(0);expect(JSON.stringify(request)).not.toContain('referenceCode');expect(JSON.stringify(request)).not.toContain('assertions');}
  mode='budget';calls=0;const budgetId=(await start()).id;ids.push(budgetId);const budget=await wait(budgetId);expect(budget.status).toBe('failed');expect(budget.error).toBe('request_budget_exceeded');expect(budget.modelCalls).toBe(4);expect(calls).toBe(4);expect(budget.clean).toBe(true);expect(budget.events.some(e=>e.kind==='score')).toBe(false);
  mode='bad';calls=0;const badId=(await start()).id;ids.push(badId);const bad=await wait(badId);expect(bad.error).toBe('unknown_tool_call');expect(bad.clean).toBe(true);expect(bad.events.some(e=>e.kind==='score')).toBe(false);
  mode='hold';calls=0;const stopId=(await start()).id;ids.push(stopId);await expect.poll(()=>calls).toBe(1);await nativeInvoke(app.page,'cleanup_harness_codex_session',{workspaceId:scope.workspaceId,jobId:stopId});const stopped=await wait(stopId);expect(stopped.status).toBe('cancelled');expect(stopped.clean).toBe(true);
  calls=0;const recoveryId=(await start()).id;ids.push(recoveryId);await expect.poll(()=>calls).toBe(1);await exec('taskkill',['/PID',String(app.processId),'/T','/F'],{windowsHide:true});await app.stop();
  app=await launchNative(info,{dataDirectory:data,endpoint:`http://127.0.0.1:${address.port}`,profileDirectory:info.outputPath('reopen')});
  const interrupted=(await state(recoveryId)).selected!;expect(interrupted.status).toBe('interrupted');expect(interrupted.clean).toBe(false);const before=calls;
  await nativeInvoke(app.page,'cleanup_harness_codex_session',{workspaceId:scope.workspaceId,jobId:recoveryId});const recovered=(await state(recoveryId)).selected!;expect(recovered.status).toBe('closed');expect(recovered.clean).toBe(true);expect(calls).toBe(before);
  expect((await state(id)).selected).toEqual(completed);
  if(legacy)expect((await state(legacy.id)).selected).toEqual(legacy);
  await open(app,scope,fixture.completed.id);await app.page.getByRole('combobox',{name:'Codex任务执行历史',exact:true}).selectOption(id);
  for(const theme of ['light','dark']){await app.page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);await expect(app.page.locator('html')).toHaveAttribute('data-color-scheme',theme);for(const size of [{width:1366,height:768},{width:1600,height:900},{width:1920,height:1080}]){await app.page.setViewportSize(size);await app.page.locator('.fit-scroll').evaluate(e=>{e.scrollTop+=e.querySelector('.codex-session')!.getBoundingClientRect().top-e.getBoundingClientRect().top-12;});expect(await app.page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);await app.page.screenshot({path:info.outputPath(`session-${theme}-${size.width}.png`)});}}
  expect(app.consoleErrors).toEqual([]);
  await writeFile(info.outputPath('session.json'),JSON.stringify({scope,provenance:'controlled_protocol_fixture_not_model_quality',realModelCalls:0,completed,budget,bad,stopped,interrupted,recovered},null,2));
 }finally{
  for(const jobId of ids){try{await nativeInvoke(app.page,'cleanup_harness_codex_session',{workspaceId:scope.workspaceId,jobId});}catch{}}
  try{await app.closeFromUi();}catch{}await app.stop();server.closeAllConnections();await new Promise<void>(r=>server.close(()=>r()));
 }
});

test('one explicitly authorized real local model session keeps the actual result',async({},info)=>{
 test.skip(process.env.CORTEXOS_HARNESS_SESSION_REAL!=='qwen3:4b-instruct','Real inference requires a fresh user popup approval for this exact bounded task.');
 test.setTimeout(240000);
 const {fixture,data,scope}=await prepare(info);const app=await launchNative(info,{dataDirectory:data});let id='';
 try{
  await open(app,scope,fixture.completed.id);
  // This invocation is bounded to the one task approved in the current conversation; no retries.
  id=(await nativeInvoke<{id:string}>(app.page,'start_harness_codex_session',{workspaceId:scope.workspaceId,input:input(fixture,'qwen3:4b-instruct','0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0')})).id;
  const state=()=>nativeInvoke<State>(app.page,'get_harness_codex_session',{...scope,jobId:id});
  await expect.poll(async()=>(await state()).selected?.status,{timeout:215000,intervals:[1000,2000]}).not.toBe('running');const actual=(await state()).selected!;
  await writeFile(info.outputPath('actual-session.json'),JSON.stringify({scope,actual,authorization:'one session, at most four requests; no retry'},null,2));
  expect(actual.modelCalls).toBeLessThanOrEqual(4);expect(actual.clean).toBe(true);expect(actual.pendingGraders).toEqual([]);
  // A failed/unsupported model answer is valid evidence, not an infrastructure test success claim.
  await app.page.getByRole('button',{name:'更新模型与执行记录',exact:true}).click();await app.page.getByRole('combobox',{name:'Codex任务执行历史',exact:true}).selectOption(id);
  await app.page.getByText('Codex 单题执行',{exact:true}).scrollIntoViewIfNeeded();await app.page.screenshot({path:info.outputPath('actual-session.png')});
 }finally{if(id)try{await nativeInvoke(app.page,'cleanup_harness_codex_session',{workspaceId:scope.workspaceId,jobId:id});}catch{}try{await app.closeFromUi();}catch{}await app.stop();}
});
