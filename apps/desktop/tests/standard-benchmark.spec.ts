import { expect,test } from "@playwright/test";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { createWorkspace,launchNative,nativeInvoke } from "./native";
import type { Batch } from "../src/benchmark/personalized";

for (const queueOnly of [false, true]) test(queueOnly ? "standard batch can cancel resource wait after reopening without sending questions" : "standard batches use no cloud key, serialize model groups, cancel and resume without resending",async({},info)=>{
  test.setTimeout(120_000);
  const questions=JSON.parse(await readFile("src-tauri/datasets/cortex-core-60-v2/cases.json","utf8")) as {case_id:string;input:{prompt:string};expected:{kind:string;answer?:number;answers?:string[]}}[];
  const names=["fixture-alpha","fixture-beta","unrelated-loaded-model"];
  const digest=(name:string)=>createHash("sha256").update(name).digest("hex");
  const details={format:"gguf",family:"fixture",parameter_size:"synthetic",quantization_level:"synthetic"};
  const loaded=new Set([names[2]]),wire:{body:Record<string,unknown>;raw:string}[]=[],released:string[]=[];
  let slow=false,concurrent=0,peak=0;
  const server=createServer(async(req,res)=>{
    const json=(value:unknown)=>{res.setHeader("Content-Type","application/json");res.end(JSON.stringify(value));};
    if(req.url==="/api/version")return json({version:"synthetic-standard-fixture"});
    if(req.url==="/api/tags")return json({models:names.map(name=>({name,digest:digest(name),size:100,details}))});
    if(req.url==="/api/ps")return json({models:[...loaded].map(name=>({name,digest:digest(name),size:100,size_vram:100,context_length:4096}))});
    let raw="";for await(const chunk of req)raw+=chunk.toString();const body=JSON.parse(raw||"{}");
    if(req.url==="/api/show")return json({details,capabilities:["completion","tools"],model_info:{"general.architecture":"fixture","fixture.context_length":8192}});
    if(req.url==="/api/generate"){
      expect(body.keep_alive).toBe(0);expect(body.prompt).toBeUndefined();released.push(body.model);loaded.delete(body.model);return json({done:true,done_reason:"unload"});
    }
    if(req.url!=="/api/chat"){res.statusCode=404;return json({error:"unknown fixture endpoint"});}
    wire.push({body,raw});loaded.add(body.model);concurrent++;peak=Math.max(peak,concurrent);res.once("close",()=>{concurrent--;});
    const prompt=body.messages.find((m:{role:string})=>m.role==="user")?.content??"";
    const question=questions.find(q=>prompt.startsWith(q.input.prompt));
    const answer=question?.expected.kind==="integer"?String(question.expected.answer):question?.expected.answers?.[0]??"unsupported fixture question";
    const frame=(content:string,done=false)=>JSON.stringify({message:{role:"assistant",content},done,...(!done?{eval_count:999,prompt_eval_count:999}:{}),...(done?{done_reason:"stop",eval_count:1,prompt_eval_count:20,...(body.model===names[0]?{eval_duration:100_000_000,load_duration:20_000_000,prompt_eval_duration:30_000_000,total_duration:150_000_000,prompt_eval_cached_count:3}:{})}:{})})+"\n";
    res.writeHead(200,{"Content-Type":"application/x-ndjson"});
    if(slow){res.write(frame("9"));return;}
    res.write(frame(" \n"));const textTimer=setTimeout(()=>res.write(frame(answer)),40);textTimer.unref();const timer=setTimeout(()=>res.end(frame("",true)),body.model===names[0]?1150:80);timer.unref();
  });
  await new Promise<void>(resolve=>server.listen(0,"127.0.0.1",resolve));
  const address=server.address();if(!address||typeof address==="string")throw new Error("fixture listener missing");
  const app=await launchNative(info,{endpoint:`http://127.0.0.1:${address.port}`});
  try{
    const {page}=app;page.setDefaultTimeout(10_000);await createWorkspace(page,"标准批量隔离验收");
    const bootstrap=await nativeInvoke<{workspaces:{id:string;name:string}[]}>(page,"bootstrap");const workspace=bootstrap.workspaces.find(w=>w.name==="标准批量隔离验收")!.id;
    const conversations=await nativeInvoke(page,"list_conversations",{workspaceId:workspace});
    expect(await nativeInvoke<{allowed:boolean}>(page,"cloud_connection")).toMatchObject({allowed:false});
    await page.getByRole("navigation",{name:"主导航",exact:true}).getByRole("link",{name:"评测",exact:true}).click();
    await expect(page.getByRole("heading",{name:"本地标准评测",exact:true})).toBeVisible();
    await page.getByRole("checkbox",{name:names[0],exact:true}).check();await page.getByRole("checkbox",{name:names[1],exact:true}).check();
    await page.getByLabel("每题重复次数",{exact:true}).fill("2");
    await page.getByText("模型参数与运行环境",{exact:true}).click();
    await page.getByRole("checkbox",{name:"切换模型时，允许释放上一个已选评测模型",exact:true}).check();
    await page.getByRole("button",{name:"预览标准评测计划",exact:true}).click();
    await expect(page.getByRole("heading",{name:"标准评测进度与结果",exact:true})).toBeVisible();
    const batchId=await page.evaluate(()=>new URLSearchParams(location.hash.split("?")[1]).get("batch")!);
    const plan=await nativeInvoke<Batch>(page,"get_standard_benchmark",{workspaceId:workspace,batchId});
    expect(plan.plan.maxRequests).toBe(8);expect(plan.plan.binding).toBeUndefined();expect(plan.plan.profile).toBeNull();expect(plan.plan.input.repetitions).toBe(2);expect(wire).toHaveLength(0);
    if(queueOnly){
      const holder=await nativeInvoke<Batch>(page,"plan_standard_benchmark",{workspaceId:workspace,input:{...plan.plan.input,models:[plan.plan.input.models[0]],caseIds:["math-001"],repetitions:1,releaseModels:[]}});
      slow=true;await nativeInvoke(page,"start_standard_benchmark",{workspaceId:workspace,batchId:holder.id,approvedPlanHash:holder.planHash});
      await expect.poll(()=>wire.length).toBe(1);
      await page.getByRole("checkbox",{name:/我已检查题库、模型和发送范围/}).check();
      await page.getByRole("button",{name:"确认并运行未发送题目",exact:true}).click();
      const cancel=page.getByRole("button",{name:"取消等待",exact:true});await expect(cancel).toBeVisible();
      await page.reload();await expect(cancel).toBeVisible();await expect(page.getByRole("status").filter({hasText:"固定题库"})).not.toContainText("待确认");await page.screenshot({path:info.outputPath("waiting.png")});await cancel.click();await expect(cancel).toBeHidden();
      const untouched=await nativeInvoke<Batch>(page,"get_standard_benchmark",{workspaceId:workspace,batchId});
      expect(untouched.status).toBe("planned");expect(untouched.items).toEqual(plan.items);expect(wire).toHaveLength(1);
      expect((await nativeInvoke<Batch>(page,"get_standard_benchmark",{workspaceId:workspace,batchId:holder.id})).status).toBe("running");
      await page.getByRole("checkbox",{name:/我已检查题库、模型和发送范围/}).check();
      await page.getByRole("button",{name:"确认并运行未发送题目",exact:true}).click();await expect(cancel).toBeVisible();
      slow=false;await nativeInvoke(page,"stop_standard_benchmark",{workspaceId:workspace,batchId:holder.id});
      await expect.poll(async()=>(await nativeInvoke<Batch>(page,"get_standard_benchmark",{workspaceId:workspace,batchId})).status,{timeout:35_000}).toBe("completed");
      expect(wire).toHaveLength(1+plan.plan.maxRequests);
      return;
    }
    const changed=await nativeInvoke(page,"start_standard_benchmark",{workspaceId:workspace,batchId,approvedPlanHash:"tampered"}).then(()=>false,()=>true);expect(changed).toBe(true);expect(wire).toHaveLength(0);
    await expect(page.getByRole("button",{name:"确认并运行未发送题目",exact:true})).toBeDisabled();
    await page.getByRole("checkbox",{name:/我已检查题库、模型和发送范围/}).check();await page.getByRole("button",{name:"确认并运行未发送题目",exact:true}).click();
    const get=()=>nativeInvoke<Batch>(page,"get_standard_benchmark",{workspaceId:workspace,batchId});
    await expect.poll(async()=>(await get()).status,{timeout:25_000}).toBe("completed");
    const complete=await get();expect(wire).toHaveLength(8);expect(peak).toBe(1);expect(wire.map(w=>w.body.model)).toEqual([...Array(4).fill(names[0]),...Array(4).fill(names[1])]);expect(released).toEqual([names[0]]);expect(loaded.has(names[2])).toBe(true);
    expect(complete.reports?.map(r=>r.statistics.correctness)).toEqual([1,1]);expect(complete.reports?.map(r=>r.statistics.planned)).toEqual([4,4]);expect(complete.selectedCases).toBe(2);expect(complete.datasetCases).toBe(60);
    const overview=await nativeInvoke<Batch>(page,"get_benchmark_overview",{workspaceId:workspace,batchId,scope:"standard"});
    expect(overview.pagedResults).toBe(true);expect(overview.items).toBeUndefined();expect(overview.reports![0].results).toEqual([]);expect(overview.reports![0].statistics).toEqual(complete.reports![0].statistics);expect(overview.reports![0].performance.tokensPerSecond).toBe(complete.reports![0].performance.tokensPerSecond);expect(overview.reports![0].performance.resources?.gpus).toEqual(complete.reports![0].performance.resources?.gpus);expect(overview.reports![0].performance.resources?.systemCpuPercentPeak).toBeCloseTo(complete.reports![0].performance.resources!.systemCpuPercentPeak!,6);
    const alpha=complete.reports![0], beta=complete.reports![1];
    expect(alpha.performance.tokensPerSecond).toBeCloseTo(10);expect(alpha.performance.outputTokensObserved).toBe(4);expect(alpha.performance.inputTokensObserved).toBe(80);expect(alpha.performance.cachedInputTokensObserved).toBe(12);
    expect(alpha.performance.firstTextMs?.n).toBe(4);expect(alpha.performance.firstTextMs?.min).toBeGreaterThanOrEqual(30);expect(alpha.performance.responseMs?.p50).toBeGreaterThanOrEqual(1000);expect(alpha.performance.promptMs?.p50).toBe(30);expect(alpha.performance.byRepetition?.map(r=>r.performance.speedCases)).toEqual([2,2]);
    expect(beta.performance.tokensPerSecond).toBeNull();expect(beta.performance.generationMs?.n).toBe(0);expect(beta.performance.inputTokensObserved).toBe(80);expect(alpha.performance.resources?.measuredCases).toBe(4);expect(alpha.performance.resources?.systemMemoryBytesPeak).toBeGreaterThan(0);
    expect(alpha.results[0].performance?.loadMs).toBe(20);expect(alpha.results[0].performance?.resources.systemCpuSamples).toBeGreaterThan(0);
    const firstRun=complete.reports![0].results[0];const detail=await nativeInvoke<{execution:{snapshot:{requestHash:string};contextManifest:Record<string,unknown>};hardware:Record<string,unknown>;resources:unknown[]}>(page,"get_benchmark_run",{workspaceId:workspace,experimentId:firstRun.runId});expect(detail.hardware.schemaVersion).toBe(1);expect(detail.resources.length).toBeGreaterThanOrEqual(2);await info.attach("standard-measurements",{body:JSON.stringify({alpha:alpha.performance,beta:beta.performance,first:alpha.results[0].performance,resources:detail.resources},null,2),contentType:"application/json"});expect(detail.execution.snapshot.requestHash).toBe(createHash("sha256").update(wire[0].raw).digest("hex"));expect(detail.execution.contextManifest).toMatchObject({historyUsed:false,retrievalUsed:false,benchmarkScope:"standard",batchId});
    expect(await nativeInvoke(page,"list_conversations",{workspaceId:workspace})).toEqual(conversations);expect(await nativeInvoke(page,"bootstrap")).toEqual(bootstrap);
    const graph=await nativeInvoke<{detail:{run:{id:string}}}>(page,"load_execution_graph",{workspaceId:workspace,runId:firstRun.executionRunId});expect(graph.detail.run.id).toBe(firstRun.executionRunId);
    await page.getByLabel("排序",{exact:true}).selectOption("speed");
    await expect(page.locator(".personal-step > .personal-table-wrap > .personal-table > tbody > tr").first()).toContainText(alpha.name);
    await expect(page.locator(".personal-step > .personal-table-wrap > .personal-table > tbody > tr").last()).toContainText(beta.name);
    for(const order of ["correctness","coverage"]){await page.getByLabel("排序",{exact:true}).selectOption(order);await expect(page.locator(".personal-step > .personal-table-wrap > .personal-table > tbody > tr").first()).toContainText(alpha.name);}
    const filters=page.getByRole("group",{name:"逐题筛选",exact:true});
    await filters.getByLabel("查看模型",{exact:true}).selectOption("1");await expect(page.locator(".personal-model-report")).toHaveCount(1);
    await expect(page.locator(".personal-model-report > h3")).toHaveText(beta.name);await expect(page.locator(".personal-step > .personal-table-wrap > .personal-table > tbody > tr")).toHaveCount(2);
    await filters.getByLabel("结果状态",{exact:true}).selectOption("Passed");await expect(page.getByText("匹配 4 / 计划 4 项",{exact:true})).toBeVisible();
    await filters.getByRole("button",{name:"清除筛选",exact:true}).click();await expect(page.locator(".personal-model-report")).toHaveCount(2);
    for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await page.setViewportSize({width,height});await page.getByRole("button",{name:"刷新结果",exact:true}).click();await expect(page.getByRole("table")).toBeVisible();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);await page.screenshot({path:info.outputPath(`standard-report-${width}.png`)});}
    const metricsPanel=page.locator(".personal-model-report").first().locator(".benchmark-measurements").first();await metricsPanel.locator("summary").first().click();
    for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await page.setViewportSize({width,height});await page.evaluate(()=>document.querySelector(".benchmark-measurements")?.scrollIntoView());expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);await page.screenshot({path:info.outputPath(`measurements-${width}.png`)});}
    await metricsPanel.locator("dl").scrollIntoViewIfNeeded();await page.screenshot({path:info.outputPath("resources-1920.png")});
    await page.getByRole("combobox",{name:"外观",exact:true}).selectOption("dark");
    for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await page.setViewportSize({width,height});await metricsPanel.scrollIntoViewIfNeeded();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);await page.screenshot({path:info.outputPath(`measurements-dark-${width}.png`)});}
    const launcher=page.locator(".personal-model-report").first().locator(".benchmark-result-button").first();await launcher.click();const casePanel=page.getByRole("dialog",{name:"单题详情",exact:true});await expect(casePanel.locator(".benchmark-result-meta strong")).toHaveText("通过");await casePanel.locator(".benchmark-measurements > summary").click();await casePanel.getByRole("button",{name:"查看原始采样记录",exact:true}).click();await expect(casePanel.locator(".benchmark-measurements pre").last()).toContainText('"samples"');
    await page.getByRole("combobox",{name:"外观",exact:true}).selectOption("light");
    await casePanel.getByRole("button",{name:"关闭单题详情",exact:true}).click();await expect(launcher).toBeFocused();
    await test.step("actual execution graph returns to the same filtered result",async()=>{
      await filters.getByLabel("查看模型",{exact:true}).selectOption("1");
      await filters.getByLabel("重复轮次",{exact:true}).selectOption("2");
      await expect(page.locator(".benchmark-result-button")).toHaveCount(2);
      await page.locator(".benchmark-result-button").last().click();
      const reportHash=await page.evaluate(()=>location.hash);
      const meta=await casePanel.locator(".benchmark-result-meta").textContent();
      await casePanel.getByRole("button",{name:"查看 3D 执行过程",exact:true}).click();
      await expect(page.getByRole("button",{name:"返回评测报告",exact:true})).toBeVisible();
      const run=await page.evaluate(()=>new URLSearchParams(location.hash.split("?")[1]).get("run"));
      expect(run).toBe(beta.results.at(-1)!.executionRunId);
      await page.reload();await expect(page.getByRole("button",{name:"返回评测报告",exact:true})).toBeVisible();
      await page.getByRole("link",{name:"返回评测结果",exact:true}).click();
      await expect(casePanel.locator(".benchmark-result-meta")).toHaveText(meta!);
      const restored=await page.evaluate(()=>location.hash);
      for(const [key,value] of new URLSearchParams(reportHash.split("?")[1]))expect(new URLSearchParams(restored.split("?")[1]).get(key)).toBe(value);
      expect(new URLSearchParams(restored.split("?")[1]).get("trace")).toBe(run);
      await page.keyboard.press("Escape");
      await page.evaluate(({workspace,run})=>{location.hash=`/graph?workspace=${workspace}&mode=execution&run=${run}`;},{workspace,run});
      await page.getByRole("button",{name:"返回评测报告",exact:true}).click();
      await expect(casePanel.locator(".benchmark-result-meta")).toHaveText(meta!);
      await page.keyboard.press("Escape");expect(wire).toHaveLength(8);
    });
    await test.step("independent regrades and native JSON CSV save cancellation",async()=>{
      const original=await nativeInvoke<Batch>(page,"get_benchmark_overview",{workspaceId:workspace,batchId,scope:"standard"});
      await page.getByRole("button",{name:"重新评分",exact:true}).click();
      const grading=page.getByRole("dialog",{name:"重新评分与历史",exact:true});
      await expect(grading.getByText("原成绩保持不变，新结果作为独立记录保存。",{exact:true})).toBeVisible();
      await expect(grading.getByRole("button",{name:"确认并重新评分",exact:true})).toBeDisabled();
      await grading.getByRole("checkbox",{name:"我确认按上述范围重新评分，保留原成绩。",exact:true}).check();
      await grading.getByRole("button",{name:"确认并重新评分",exact:true}).click();
      await expect(grading.getByRole("button",{name:"查看这轮评分",exact:true})).toBeVisible();expect(wire).toHaveLength(8);
      const history=await nativeInvoke<{sessions:{session_id:string;result_count:number}[]}>(page,"get_benchmark_regrades",{workspaceId:workspace,batchId,scope:"standard",sessionId:null,page:0});expect(history.sessions[0].result_count).toBe(8);
      await grading.getByRole("button",{name:"查看这轮评分",exact:true}).click();await expect(grading.locator(".benchmark-grade-list > li")).toHaveCount(8);await expect(grading.locator(".benchmark-grade-list > li").last()).toContainText("第 2 次");
      for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await page.setViewportSize({width,height});await page.screenshot({path:info.outputPath(`regrade-${width}.png`)});expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);}
      await grading.getByRole("button",{name:"关闭重新评分",exact:true}).click();
      const after=await nativeInvoke<Batch>(page,"get_benchmark_overview",{workspaceId:workspace,batchId,scope:"standard"});expect(after.reports!.map(r=>r.statistics)).toEqual(original.reports!.map(r=>r.statistics));
      const saveDialog=page.getByRole("dialog",{name:"导出评测报告",exact:true});
      const save=async(format:"json"|"csv"|"cancel")=>{
        await page.getByRole("button",{name:"导出报告",exact:true}).click();await saveDialog.getByRole("combobox",{name:"报告文件格式",exact:true}).selectOption(format==="csv"?"csv":"json");
        await saveDialog.getByRole("button",{name:"选择保存位置",exact:true}).click();
        const output=info.outputPath(format==="csv"?"report.csv":"report.json");
        const result=spawnSync("pwsh.exe",["-NoProfile","-ExecutionPolicy","Bypass","-File",resolve("tests/save-report-dialog.ps1"),"-AppProcessId",String(app.processId),...(format==="cancel"?["-Cancel"]:["-OutputPath",output])],{windowsHide:true,timeout:25_000,encoding:"utf8"});
        await info.attach(`native-save-${format}`,{body:JSON.stringify({status:result.status,stdout:result.stdout,stderr:result.stderr}),contentType:"text/plain"});expect(result.status,result.stderr).toBe(0);
        if(format==="cancel"){await expect(saveDialog.getByRole("button",{name:"选择保存位置",exact:true})).toBeEnabled();await saveDialog.getByRole("button",{name:"取消",exact:true}).click();}
        else await expect(saveDialog).toBeHidden();
        return output;
      };
      const jsonPath=await save("json"),jsonBytes=await readFile(jsonPath),report=JSON.parse(jsonBytes.toString("utf8"));expect(report.reportSchema).toBe("cortexos.benchmark-report");expect(report.schemaVersion).toBe(1);expect(report.contentHash).toMatch(/^[a-f0-9]{64}$/);expect(report.payload.items).toHaveLength(8);expect(report.payload.items.every((item:any)=>item.attempts[0].evaluations.length===2)).toBe(true);
      const csvPath=await save("csv"),csv=await readFile(csvPath,"utf8");expect(csv.startsWith("\uFEFF")).toBe(true);expect(csv).toContain("regrade");expect(csv).toContain("original");
      await save("cancel");expect(await readFile(jsonPath)).toEqual(jsonBytes);expect(wire).toHaveLength(8);
    });
    const before=wire.length;
    const interrupted=await nativeInvoke<Batch>(page,"plan_standard_benchmark",{workspaceId:workspace,input:{...plan.plan.input,models:[plan.plan.input.models[0]],caseIds:["math-001","zh-006"],repetitions:2,releaseModels:[]}});
    slow=true;await nativeInvoke(page,"start_standard_benchmark",{workspaceId:workspace,batchId:interrupted.id,approvedPlanHash:interrupted.planHash});
    await expect.poll(()=>wire.length).toBe(before+1);
    await page.evaluate(({workspace,id})=>{location.hash=`/benchmark?workspace=${workspace}&batch=${id}`;},{workspace,id:interrupted.id});await page.reload();
    await page.getByRole("button",{name:"停止评测",exact:true}).click();
    const readInterrupted=()=>nativeInvoke<Batch>(page,"get_standard_benchmark",{workspaceId:workspace,batchId:interrupted.id});
    await expect.poll(async()=>(await readInterrupted()).status).toBe("cancelled");const stopped=await readInterrupted();expect(stopped.remainingRequests).toBe(3);expect(stopped.canResume).toBe(true);const cancelledRun=stopped.reports![0].results[0].runId;const partial=stopped.reports![0].results[0].performance!;expect(partial.responseMs).not.toBeNull();expect(partial.firstTextMs).not.toBeNull();expect(partial.outputTokens).toBeNull();expect(partial.tokensPerSecond).toBeNull();expect(stopped.reports![0].performance.firstTextMs?.n).toBe(0);
    slow=false;await page.getByRole("button",{name:"刷新结果",exact:true}).click();await page.getByRole("checkbox",{name:/我已检查题库、模型和发送范围/}).check();await page.getByRole("button",{name:"确认并运行未发送题目",exact:true}).click();
    await expect.poll(async()=>(await readInterrupted()).status,{timeout:20_000}).toBe("completed");const resumed=await readInterrupted();expect(wire).toHaveLength(before+4);expect(resumed.reports![0].statistics.scoringCoverage).toBe(.75);expect(resumed.reports![0].statistics.correctness).toBe(1);expect(resumed.reports![0].statistics.states.Cancelled).toBe(1);expect(resumed.reports![0].results[0].runId).toBe(cancelledRun);expect(resumed.reports![0].results[0].output).toBe("9");
    await page.evaluate(()=>{location.hash='/chat';});
    for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){await page.setViewportSize({width,height});const controls=await page.getByRole("group",{name:"窗口操作",exact:true}).boundingBox();const panel=await page.getByRole("complementary",{name:"回答依据",exact:true}).boundingBox();expect(controls!.y+controls!.height).toBeLessThan(panel!.y);await page.screenshot({path:info.outputPath(`native-chat-controls-${width}.png`)});}
    expect(app.consoleErrors).toEqual([]);
  }finally{await app.stop();server.closeAllConnections();await new Promise<void>(resolve=>server.close(()=>resolve()));}
});
