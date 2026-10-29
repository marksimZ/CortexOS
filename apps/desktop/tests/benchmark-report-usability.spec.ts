import { expect,test } from "@playwright/test";
import { createServer } from "node:http";
import { writeFile } from "node:fs/promises";
import { createWorkspace,launchNative,nativeInvoke } from "./native";
import type { Batch } from "../src/benchmark/personalized";

test("native large report measures reads and keeps keyboard and running controls visible",async({},info)=>{
 test.setTimeout(90_000);
 const names=["a","b","c"].map(s=>`ui-${"long-model-name-".repeat(7)}${s}`),digest="a".repeat(64);
 let requests=0;
 const server=createServer(async(req,res)=>{
  const json=(v:unknown)=>{res.setHeader("Content-Type","application/json");res.end(JSON.stringify(v));};
  if(req.url==="/api/version")return json({version:"controlled-ui-only"});
  if(req.url==="/api/tags")return json({models:names.map(name=>({name,digest,size:1,details:{format:"gguf"}}))});
  if(req.url==="/api/ps")return json({models:[]});
  if(req.url==="/api/show")return json({capabilities:["completion"],details:{format:"gguf"},model_info:{"general.architecture":"fixture","fixture.context_length":8192}});
  if(req.url==="/api/chat"){
   for await(const _chunk of req){ /* Drain this controlled request before returning an unfinished stream. */ }
   requests++;res.writeHead(200,{"Content-Type":"application/x-ndjson"});res.write(JSON.stringify({message:{role:"assistant",content:"受控界面检查：等待停止。"},done:false})+"\n");return;
  }
  res.statusCode=400;return json({error:"Unexpected controlled endpoint"});
 });
 await new Promise<void>(r=>server.listen(0,"127.0.0.1",r));const address=server.address();if(!address||typeof address==="string")throw Error("Missing fixture listener");
 const app=await launchNative(info,{endpoint:`http://127.0.0.1:${address.port}`});
 try{
  const {page}=app;page.setDefaultTimeout(10_000);await page.emulateMedia({reducedMotion:"reduce"});
  await createWorkspace(page,"900 项界面与读取检查（受控数据）");
  const bootstrap=await nativeInvoke<{workspaces:{id:string;name:string}[]}>(page,"bootstrap"),workspaceId=bootstrap.workspaces.find(w=>w.name==="900 项界面与读取检查（受控数据）")!.id;
  const datasets=await nativeInvoke<{id:string;version:number;contentHash:string}[]>(page,"list_benchmark_datasets",{workspaceId});const dataset=datasets.find(d=>d.id==="cortex-core-60"&&d.version===2)!;
  const plan=await nativeInvoke<Batch>(page,"plan_standard_benchmark",{workspaceId,input:{datasetId:dataset.id,datasetVersion:2,datasetHash:dataset.contentHash,models:names.map(model=>({kind:"local",local:{model,expectedDigest:digest,parameters:{numCtx:4096,numPredict:512,temperature:0,timeoutSeconds:30},requirements:["completion"]}})),maxOutputTokens:512,repetitions:5,caseIds:[],releaseModels:[]}});
  const args={workspaceId,batchId:plan.id,scope:"standard"};
  const reads:Record<string,{ms:number;bytes:number}[]>={};
  for(const [label,command,extra] of [["overview","get_benchmark_overview",{}],["page","get_benchmark_result_page",{modelIndex:0,page:7,pageSize:20}],["detail","get_benchmark_result_detail",{ordinal:1}]] as const){
   reads[label]=[];
   for(let i=0;i<5;i++)reads[label].push(await page.evaluate(async({command,args})=>{
    const start=performance.now();const data=await (window as any).__TAURI_INTERNALS__.invoke(command,args);const ms=performance.now()-start;
    return {ms,bytes:new TextEncoder().encode(JSON.stringify(data)).length};
   },{command,args:{...args,...extra}}));
  }
  expect(Math.max(...reads.overview.map(v=>v.bytes))).toBeLessThan(120_000);expect(Math.max(...reads.page.map(v=>v.bytes))).toBeLessThan(60_000);
  const overview=await nativeInvoke<Batch>(page,"get_benchmark_overview",args);expect(overview.reports!.map(r=>r.statistics.planned)).toEqual([300,300,300]);expect(overview.reports!.every(r=>r.results.length===0)).toBe(true);
  await page.evaluate(({workspaceId,id})=>{location.hash=`/benchmark?workspace=${workspaceId}&batch=${id}`;},{workspaceId,id:plan.id});
  const content=page.locator(".benchmark-content"),models=page.locator(".personal-model-report"),first=models.first();
  await expect(page.locator(".benchmark-result-button")).toHaveCount(60);expect(await content.evaluate(e=>e.scrollTop)).toBe(0);
  const flipMs:number[]=[];
  for(let i=0;i<5;i++){
   const next=i%2===0;const start=performance.now();await first.getByRole("button",{name:next?"下一页":"上一页",exact:true}).click();
   await expect(first.locator(".benchmark-result-button").first()).toContainText(next?"21.":"1.");flipMs.push(performance.now()-start);
  }
  await first.getByRole("button",{name:"上一页",exact:true}).click();
  await expect(first.locator(".benchmark-result-button").first()).toContainText("1.");
  for(const theme of ["light","dark"]){
   await page.getByRole("combobox",{name:"外观",exact:true}).selectOption(theme);
   for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
    await page.setViewportSize({width,height});await content.evaluate(e=>e.scrollTop=0);
    expect(await content.evaluate(e=>e.scrollWidth<=e.clientWidth)).toBe(true);
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.screenshot({path:info.outputPath(`large-report-${theme}-${width}.png`)});
    const table=page.getByRole("region",{name:"全计划模型比较表",exact:true});await table.focus();await page.keyboard.press("ArrowRight");await expect(table).toBeFocused();
    await first.locator(".benchmark-result-button").last().scrollIntoViewIfNeeded();
    const actions=await page.getByRole("group",{name:"评测运行操作",exact:true}).boundingBox(),pagination=await first.locator(".benchmark-pagination").boundingBox();
    expect(actions!.y).toBeGreaterThanOrEqual(76);expect(pagination!.y).toBeGreaterThanOrEqual(actions!.y+actions!.height-1);expect(pagination!.y+pagination!.height).toBeLessThan(height);
    await page.screenshot({path:info.outputPath(`sticky-report-${theme}-${width}.png`)});
    await first.locator(".benchmark-result-button").nth(3).focus();
    const focus=await first.locator(".benchmark-result-button").nth(3).boundingBox(),toolbar=await first.locator(".profile-header").boundingBox();expect(focus!.y).toBeGreaterThanOrEqual(toolbar!.y+toolbar!.height);
    await page.keyboard.press("Enter");const drawer=page.getByRole("dialog",{name:"单题详情",exact:true});await expect(drawer.getByText("尚未发送，没有模型输出。",{exact:true})).toBeVisible();
    const close=await drawer.getByRole("button",{name:"关闭单题详情",exact:true}).boundingBox();expect(close!.y).toBeGreaterThanOrEqual(76);await page.keyboard.press("Escape");await expect(first.locator(".benchmark-result-button").nth(3)).toBeFocused();
   }
  }
  const unchanged=await nativeInvoke<Batch>(page,"get_benchmark_overview",args);expect(unchanged.reports!.map(r=>r.statistics)).toEqual(overview.reports!.map(r=>r.statistics));expect(requests).toBe(0);
  // Send one unfinished controlled response to verify the existing stop operation stays available while scrolling.
  await nativeInvoke(page,"start_standard_benchmark",{workspaceId,batchId:plan.id,approvedPlanHash:plan.planHash});
  await expect.poll(()=>requests).toBe(1);await page.getByRole("button",{name:"刷新结果",exact:true}).click();
  await expect(page.getByRole("button",{name:"停止评测",exact:true})).toBeVisible();
  await models.last().locator(".benchmark-result-button").last().scrollIntoViewIfNeeded();
  const stop=page.getByRole("button",{name:"停止评测",exact:true}),box=await stop.boundingBox();expect(box!.y).toBeGreaterThanOrEqual(76);expect(box!.y+box!.height).toBeLessThan(1080);
  await page.screenshot({path:info.outputPath("running-stop-after-scroll.png")});await stop.click();
  await expect.poll(async()=>(await nativeInvoke<Batch>(page,"get_benchmark_overview",args)).status).toBe("cancelled");expect(requests).toBe(1);
  const summary=(v:number[])=>{const s=[...v].sort((a,b)=>a-b);return{n:s.length,min:s[0],median:s[Math.floor(s.length/2)],max:s.at(-1)};};
  const evidence={planned:900,models:3,pageSize:20,renderedItems:60,reads:Object.fromEntries(Object.entries(reads).map(([key,v])=>[key,{ms:summary(v.map(x=>x.ms)),jsonUtf8Bytes:summary(v.map(x=>x.bytes)),samples:v}])),pageFlipMsIncludingAutomation:summary(flipMs),generationRequestsDuringReads:0,controlledUnfinishedRequestForStop:requests,scope:"synthetic model catalog; actual SQLite/WebView2; queued plan, not a model capability score"};
  await writeFile(info.outputPath("performance.json"),JSON.stringify(evidence,null,2));
  await info.attach("report-performance-evidence",{body:JSON.stringify(evidence,null,2),contentType:"application/json"});
  expect(app.consoleErrors).toEqual([]);
 }finally{await app.stop();server.closeAllConnections();await new Promise<void>(r=>server.close(()=>r()));}
});
