import { expect, test, type TestInfo } from '@playwright/test';
import { createServer } from 'node:http';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { launchNative, nativeInvoke } from './native';
const exec = promisify(execFile);
const source=resolve('../../artifacts/harness-aider-native-final/harness-aider-runtime-Aide-e3010-restores-interrupted-checks-harness-aider-runtime');
type RecordItem={id:string;status:string;clean:boolean;pendingGraders:string[];modelCalls:number;error:string|null;plan:any;events:{kind:string;data:any}[]};
type State={selected:RecordItem|null;history:RecordItem[]};
async function prepare(info:TestInfo){
 const fixture=JSON.parse(await readFile(join(source,'runtime.json'),'utf8'));
 const data=info.outputPath('data');await mkdir(data,{recursive:true});
 const backing=source;
 await exec('py',['-3.13','-c','import sqlite3,sys\ns=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True)\nd=sqlite3.connect(sys.argv[2])\ns.backup(d)\nd.close()\ns.close()',join(backing,'data/cortexos.sqlite3'),join(data,'cortexos.sqlite3')],{windowsHide:true});
 return {fixture,data,scope:{workspaceId:fixture.scope.workspaceId,assessmentVersion:1}};
}
async function open(app:Awaited<ReturnType<typeof launchNative>>,scope:any,runtimeId:string){
 await app.page.evaluate(w=>{location.hash=`/benchmark?workspace=${w}&view=harness`;},scope.workspaceId);await app.page.reload();
 await app.page.getByRole('button',{name:'4. 测试任务',exact:true}).click();await app.page.getByText('Aider 运行前检查',{exact:true}).click();
 await app.page.getByRole('combobox',{name:'Aider运行前检查历史',exact:true}).selectOption(runtimeId);await app.page.getByText('Aider 单题执行',{exact:true}).click();
}
const input=(fixture:any,model:string,digest:string)=>({assessmentVersion:1,runtimeId:fixture.completed.id,runtimeHash:fixture.completed.planHash,model:{model,expectedDigest:digest,parameters:{numCtx:4096,numPredict:512,temperature:0,timeoutSeconds:120},requirements:['completion']},confirmed:true});

test('fixed reply edits the isolated function, scores it and supports stop and recovery',async({},info)=>{
 test.setTimeout(240000);
 const {fixture,data,scope}=await prepare(info);
 const model='controlled-protocol-fixture',digest='a'.repeat(64);let mode='edit',calls=0;
 const executable=process.env.CORTEXOS_AIDER_TEST_EXE;
 const recordedReplyPath=process.env.CORTEXOS_AIDER_REPLAY_REPLY;
 const recordedReply=recordedReplyPath?JSON.parse(await readFile(recordedReplyPath,'utf8')).actual.events.find((e:any)=>e.kind==='model_response').data.protocolReply.choices[0].message.content as string:null;
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
  const prompt=body.messages.map((m:any)=>m.content).join('\n');
  const filename=/Return exactly the line ([^,\n]+), then/.exec(prompt)?.[1];
  const content=`${mode==='bad'?'out-of-scope.py':filename}\n\x60\x60\x60python\ndef adjust(n):\n    return max(0, n)${mode==='signature'?'\ndef extra():\n    pass':''}\n\x60\x60\x60`;
  send({model,done:true,done_reason:'stop',message:{role:'assistant',content:mode==='edit'?(recordedReply??content):content},prompt_eval_count:200,eval_count:40});
 });
 await new Promise<void>(r=>server.listen(0,'127.0.0.1',r));const address=server.address();if(!address||typeof address==='string')throw new Error('fixture port');
 let app=await launchNative(info,{executable,dataDirectory:data,endpoint:`http://127.0.0.1:${address.port}`}).catch(async error=>{server.closeAllConnections();await new Promise<void>(r=>server.close(()=>r()));throw error;});
 const state=(jobId:string|null=null)=>nativeInvoke<State>(app.page,'get_harness_aider_session',{...scope,jobId});
 const start=()=>nativeInvoke<{id:string}>(app.page,'start_harness_aider_session',{workspaceId:scope.workspaceId,input:input(fixture,model,digest)});
 const wait=async(id:string)=>{await expect.poll(async()=>(await state(id)).selected?.status,{timeout:60000,intervals:[300,700]}).not.toMatch(/^(running|grading)$/);return (await state(id)).selected!;};
 const ids:string[]=[];
 try{
  await open(app,scope,fixture.completed.id);await expect(app.page.getByRole('button',{name:'运行一个任务…',exact:true})).toBeEnabled();
  const legacy=await nativeInvoke<any>(app.page,'get_harness_claude_session',{...scope,jobId:null});
  const oldRuntime=await nativeInvoke<any>(app.page,'get_harness_aider_runtime',{...scope,jobId:fixture.completed.id});
  expect(oldRuntime.selected).toEqual(fixture.completed);
  let wrongPurpose=false;try{await nativeInvoke(app.page,'get_harness_aider_session',{...scope,jobId:fixture.completed.id});}catch{wrongPurpose=true;}expect(wrongPurpose).toBe(true);

  const initial=(await state()).history.length;

  const rejected=async(value:any)=>{let reason='';try{await nativeInvoke(app.page,'start_harness_aider_session',{workspaceId:scope.workspaceId,input:value});}catch(e){reason=String(e);}expect(reason).not.toBe('');};
  await rejected({...input(fixture,model,digest),confirmed:false});await rejected({...input(fixture,model,digest),runtimeHash:'forged'});
  const other=await nativeInvoke<{id:string}>(app.page,'create_workspace',{name:'isolated scope'});
  let denied=false;try{await nativeInvoke(app.page,'start_harness_aider_session',{workspaceId:other.id,input:input(fixture,model,digest)});}catch{denied=true;}expect(denied).toBe(true);expect((await state()).history).toHaveLength(initial);
  await app.page.getByRole('button',{name:'运行一个任务…',exact:true}).click();const dialog=app.page.getByRole('dialog',{name:'确认本次本地模型调用',exact:true});await expect(dialog).toBeVisible();expect(requests).toHaveLength(0);await dialog.getByRole('button',{name:'返回',exact:true}).click();expect((await state()).history).toHaveLength(initial);
  await app.page.getByRole('button',{name:'运行一个任务…',exact:true}).click();
  await dialog.getByRole('button',{name:'确认调用并运行一题',exact:true}).click();
  await expect.poll(async()=>(await state()).history.length).toBe(initial+1);
  const id=(await state()).selected!.id;ids.push(id);const completed=await wait(id);
  await writeFile(info.outputPath('first-session.json'),JSON.stringify(completed,null,2));
  expect(completed.status,completed.error??'read first-session.json for evidence').toBe('completed');expect(completed.clean).toBe(true);expect(completed.pendingGraders).toEqual([]);expect(completed.modelCalls).toBe(1);
  const score=completed.events.find(e=>e.kind==='score')?.data.score;expect(score?.state,'read first-session.json for actual CLI and assertion evidence').toBe('Passed');expect(completed.events.find(e=>e.kind==='candidate')?.data.patch.after).toContain(recordedReply?'return max(n, 0)':'return max(0, n)');
  for(const request of requests){expect(request.options).toEqual({num_ctx:4096,num_predict:512,temperature:0});expect(request.keep_alive).toBe(0);expect(JSON.stringify(request)).not.toContain('referenceCode');expect(JSON.stringify(request)).not.toContain('assertions');}
  mode='bad';calls=0;const badId=(await start()).id;ids.push(badId);const bad=await wait(badId);expect(bad.error).toBe('aider_edit_outside_format');expect(bad.events.some(e=>e.kind==='candidate')).toBe(false);expect(bad.clean).toBe(true);expect(bad.events.some(e=>e.kind==='score')).toBe(false);
  mode='signature';calls=0;const scopeId=(await start()).id;ids.push(scopeId);const scopeFailure=await wait(scopeId);expect(scopeFailure.error).toBe('aider_edit_outside_function');expect(scopeFailure.clean).toBe(true);expect(scopeFailure.events.some(e=>e.kind==='candidate'||e.kind==='score')).toBe(false);
  mode='hold';calls=0;const stopId=(await start()).id;ids.push(stopId);await expect.poll(()=>calls).toBe(1);await nativeInvoke(app.page,'cleanup_harness_aider_session',{workspaceId:scope.workspaceId,jobId:stopId});const stopped=await wait(stopId);expect(stopped.status).toBe('cancelled');expect(stopped.clean).toBe(true);
  calls=0;const recoveryId=(await start()).id;ids.push(recoveryId);await expect.poll(()=>calls).toBe(1);await exec('taskkill',['/PID',String(app.processId),'/T','/F'],{windowsHide:true});await app.stop();
  app=await launchNative(info,{executable,dataDirectory:data,endpoint:`http://127.0.0.1:${address.port}`,profileDirectory:info.outputPath('reopen')});
  const interrupted=(await state(recoveryId)).selected!;expect(interrupted.status).toBe('interrupted');expect(interrupted.clean).toBe(false);const before=calls;
  await nativeInvoke(app.page,'cleanup_harness_aider_session',{workspaceId:scope.workspaceId,jobId:recoveryId});const recovered=(await state(recoveryId)).selected!;expect(recovered.status).toBe('closed');expect(recovered.clean).toBe(true);expect(calls).toBe(before);
  expect((await state(id)).selected).toEqual(completed);

  await open(app,scope,fixture.completed.id);await app.page.getByRole('combobox',{name:'Aider任务执行历史',exact:true}).selectOption(id);
  for(const theme of ['light','dark']){await app.page.getByRole('combobox',{name:'外观',exact:true}).selectOption(theme);await expect(app.page.locator('html')).toHaveAttribute('data-color-scheme',theme);for(const size of [{width:1366,height:768},{width:1600,height:900},{width:1920,height:1080}]){await app.page.setViewportSize(size);await app.page.locator('.fit-scroll').evaluate(e=>{e.scrollTop+=e.querySelector('.aider-session')!.getBoundingClientRect().top-e.getBoundingClientRect().top-12;});expect(await app.page.locator('.fit-scroll').evaluate(e=>e.scrollWidth<=e.clientWidth+1)).toBe(true);await app.page.screenshot({path:info.outputPath(`session-${theme}-${size.width}.png`)});}}
  expect(await nativeInvoke<any>(app.page,'get_harness_claude_session',{...scope,jobId:null})).toEqual(legacy);
  expect(app.consoleErrors).toEqual([]);
  await writeFile(info.outputPath('session.json'),JSON.stringify({scope,provenance:'controlled_protocol_fixture_not_model_quality',realModelCalls:0,completed,bad,scopeFailure,stopped,interrupted,recovered},null,2));
 }finally{
  for(const jobId of ids){try{await nativeInvoke(app.page,'cleanup_harness_aider_session',{workspaceId:scope.workspaceId,jobId});}catch{}}
  try{await app.closeFromUi();}catch{}await app.stop();server.closeAllConnections();await new Promise<void>(r=>server.close(()=>r()));
 }
});


// Explicit opt-in is set only after a fresh popup approval for this bounded batch.
test('one explicitly authorized Aider local session preserves the actual outcome',async({},info)=>{
 const model=process.env.CORTEXOS_AIDER_APPROVED_MODEL;
 const digest=process.env.CORTEXOS_AIDER_APPROVED_DIGEST;
 test.skip(process.env.CORTEXOS_AIDER_REAL_APPROVAL!=='one-adjust-task'||!model||!digest,'Requires fresh approval naming the actual model, digest and one bounded task.');
 test.setTimeout(240000);
 const {fixture,data,scope}=await prepare(info);const app=await launchNative(info,{dataDirectory:data});let id='';
 try{
  await open(app,scope,fixture.completed.id);
  id=(await nativeInvoke<{id:string}>(app.page,'start_harness_aider_session',{workspaceId:scope.workspaceId,input:input(fixture,model!,digest!)})).id;
  const state=()=>nativeInvoke<State>(app.page,'get_harness_aider_session',{...scope,jobId:id});
  await expect.poll(async()=>(await state()).selected?.status,{timeout:215000,intervals:[1000,2000]}).not.toMatch(/^(running|grading)$/);
  const actual=(await state()).selected!;
  await writeFile(info.outputPath('actual-session.json'),JSON.stringify({scope,actual,authorization:'one Aider session; at most 4 requests, context 4096, output 512, request 120s, programming 180s; no retry'},null,2));
  expect(actual.modelCalls).toBeLessThanOrEqual(4);expect(actual.clean).toBe(true);expect(actual.pendingGraders).toEqual([]);
  await app.page.getByRole('button',{name:'更新模型与执行记录',exact:true}).click();await app.page.getByRole('combobox',{name:'Aider任务执行历史',exact:true}).selectOption(id);
  await app.page.getByText('Aider 单题执行',{exact:true}).scrollIntoViewIfNeeded();await app.page.screenshot({path:info.outputPath('actual-session.png')});
 }finally{if(id)try{await nativeInvoke(app.page,'cleanup_harness_aider_session',{workspaceId:scope.workspaceId,jobId:id});}catch{}try{await app.closeFromUi();}catch{}await app.stop();}
});
