import { expect, test } from "@playwright/test";

test.beforeEach(async({page})=>{
  page.setDefaultTimeout(10_000);
  await page.addInitScript(() => {
    const w = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; releaseFirst: () => void; detailsRead: number[]; badCommands: string[] };
    Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async(value:string)=>{(w as any).copiedLink=value;}}});
    w.isTauri = true; w.detailsRead = []; w.badCommands = [];
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    const model = { kind: "local", local: { model: "合成模型（仅用于界面检查）", expectedDigest: "a".repeat(64), parameters: { numCtx: 4096, numPredict: 512, temperature: 0, timeoutSeconds: 30 }, requirements: ["completion"] } };
    const statistics = { planned: 5, attempted: 3, scored: 2, passed: 1, states: { Passed: 1, Failed: 1, "Execution Error": 1, "Not Run": 1, Unsupported: 1 }, correctness: .5, scoringCoverage: .4, executionCoverage: .6, executionSuccessRate: 1 };
    const large=location.hash.includes("navigation-fixture")||location.hash.includes("personal-fixture")||location.hash.includes("late-fixture")||location.hash.includes("presentation-fixture");
    const rows = Array.from({length:large?45:5},(_,i)=>["Passed", "Failed", "Execution Error", "Not Run", "Unsupported"][i]??"Not Run").map((state, ordinal) => ({ ordinal, caseId: `fixture-${ordinal}`, state, status: state === "Not Run" ? "queued" : "finished", repetition: large?Math.floor(ordinal/15)+1:1, question: `合成题目 ${ordinal}`, error: null, runId: state === "Not Run" ? null : `run-${ordinal}`, evaluator: "integer_equal", difficulty: "easy" }));
    if(location.hash.includes("presentation-fixture"))model.local.model="界面检查模型-"+"long-model-name-".repeat(12);
    const batch = { id: large?"navigation-fixture":"drawer-fixture",scope:"standard", status: "completed", planHash: "f".repeat(64), pagedResults: true, canResume: false, plan: { input: { datasetId: "cortex-core-60", datasetVersion: 2, datasetHash: "d".repeat(64), models: [model], price: null, repetitions: large?3:1 }, cases: rows.map(r => r.caseId), binding: null, cloudRequests: [], reservedCost: null, maxRequests: 5, profile: null }, reports: [{ name: model.local.model, modelIndex: 0, kind: "local", statistics, byEvaluator: {}, byDifficulty: {}, performance: { elapsedMs: 1, inputTokensObserved: null, outputTokensObserved: null, tokensPerSecond: null, costEstimate: null, unknownCostRequests: 0, currency: null }, results: [] }] };
    let hold = true, firstError = true;
    let saveStep=0,preview=0,rejectGrade:((reason:unknown)=>void)|null=null;const gradeSessions:any[]=[];
    (w as any).cancelledGrades=0;(w as any).regradeCalls=0;(w as any).exportFormats=[];
    const waiters: (() => void)[] = [];
    w.releaseFirst = () => { hold = false; waiters.splice(0).forEach(done => done()); };
    statistics.planned=rows.length;
    const detail = (ordinal: number) => ({
      itemError: ordinal === 2 ? "合成运行错误：环境不可用，输出未保存。" : null,
      case: { input: { prompt: `合成题目 ${ordinal}：检查详情归属和长文本。`, response_schema: null, tools: [] }, expected: { answer: 4 }, scoring: { id: "integer_equal", version: 2, contract: {} }, fixture: ordinal === 1 ? [{ id: "DOC-1", title: "合成固定资料", lines: ["此资料只供界面检查。", "必须保留来源编号和行号。"+(location.hash.includes("presentation-fixture")?"LONG-SOURCE-".repeat(80):"")] }] : [] },
      output: ordinal === 0 ? "OLD-LATE-ANSWER" : ordinal === 1 ? "## NEW-CURRENT-ANSWER\n\n" + "完整原文，保留空格和换行。\n".repeat(800) + "RAW-END-MARKER" : rows[ordinal].status === "queued" ? null : "",
      execution: rows[ordinal].status === "queued" ? null : { run: { id: `execution-${ordinal}`, status: ordinal === 2 ? "failed" : "completed", errorMessage: null }, snapshot: { request: { model: model.local.model, messages: [] }, requestHash: "b".repeat(64) }, contextManifest: { purpose: "benchmark", historyUsed: false }, events: [{ seq: 1, kind: "model_request_started", payload: { fixture: true } }] },
      summary: rows[ordinal].status === "queued" ? undefined : { id: `run-${ordinal}` },
      evaluations: rows[ordinal].status === "queued" ? [] : [{ details: { state: rows[ordinal].state, reason: "synthetic_ui_only" } }],
      config: rows[ordinal].status === "queued" ? undefined : { datasetVersion: 2, fixtureOnly: true }, model: rows[ordinal].status === "queued" ? undefined : { request: model.local },
      performance: null, hardware: null, resources: [], events: [],
    });
    w.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback: () => {}, invoke: async (command: string, args: Record<string, any>) => {
      if (command === "bootstrap") return { workspaces: [{ id: "fixture", name: "合成界面工作区", createdAt: "2026-10-28" }], settings: { model: model.local.model, numCtx: 4096, numPredict: 512, temperature: 0, timeoutSeconds: 30 }, activeRun: null };
      if (command === "list_conversations" || command === "list_benchmark_datasets" || command === "list_standard_benchmark_cases") return [];
      if (command === "list_benchmark_models") return { models: [], warnings: [] };
      if (command === "check_ollama") return { version: "synthetic_ui_only", models: [] };
      if (command === "cloud_connection") return { state: "verified", allowed: true, provider: "openai", model: "fixture-cloud", checkedAt: "2026-10-28", models: [] };
      if (command === "list_standard_benchmarks") return [{ id: batch.id, status: batch.status, createdAt: "2026-10-28", datasetVersion: 2 }];
      if (command === "get_benchmark_overview") {
        if(args.workspaceId!=="fixture" || ((args.batchId==="personal-fixture")!==(args.scope==="personalized"))) throw "评测不存在或不属于此工作区和评测类型。";
        if(args.batchId==="late-fixture")await new Promise<void>(done=>{(w as any).releaseBatch=done;});
        return {...batch,id:args.batchId,scope:args.scope};
      }
      if(command==="get_benchmark_profile")return null;
      if(command==="list_benchmark_generations"||command==="list_benchmark_repositories")return [];
      if(command==="list_personalized_batches")return [{id:"personal-fixture",status:"completed",createdAt:"2026-10-28",datasetVersion:2}];
      if(command==="personalized_access")return {allowed:true,state:"verified"};
      const filtered=()=>rows.filter(r=>(!args.filter?.state||r.state===args.filter.state)&&(!args.filter?.evaluator||r.evaluator===args.filter.evaluator)&&(!args.filter?.difficulty||r.difficulty===args.filter.difficulty)&&(!args.filter?.repetition||r.repetition===args.filter.repetition));
      if(command==="get_benchmark_result_location"){
        const ordinal=args.ordinal??Number(args.executionRunId?.split("-").at(-1));
        if(!rows[ordinal]||args.workspaceId!=="fixture"||(args.executionRunId&&args.executionRunId!==`execution-${ordinal}`))throw "题目或执行记录不属于当前工作区和评测。";
        const index=filtered().findIndex(r=>r.ordinal===ordinal);
        return {ordinal,modelIndex:0,matchesFilter:index>=0,page:index<0?null:Math.floor(index/args.pageSize)};
      }
      if (command === "get_benchmark_result_page") {if((w as any).holdPage&&args.page===1)await new Promise<void>(done=>{(w as any).releasePage=done;});const matched=filtered();return {page:args.page,pageSize:20,total:matched.length,plannedTotal:rows.length,rows:matched.slice(args.page*20,args.page*20+20)};}
      if (command === "get_benchmark_result_detail") {
        const ordinal = Number(args.ordinal); w.detailsRead.push(ordinal);
        if (ordinal === 0 && hold) await new Promise<void>(done => waiters.push(done));
        if (ordinal === 2 && firstError) { firstError = false; throw "合成读取失败，请重试；没有自动重发模型请求。"; }
        return detail(ordinal);
      }
      if(command==="export_benchmark_report"){
        (w as any).exportFormats.push(args.format);saveStep++;
        if(saveStep===1)return false;if(saveStep===2)throw "文件已存在，请选择新文件名，原文件未改动。";return true;
      }
      if(command==="preview_benchmark_regrade")return {sessionId:`regrade-fixture-${++preview}`,planHash:"p".repeat(64),engineVersion:"synthetic-ui-engine",implementationHash:"f".repeat(64),eligible:3,skippedAttempts:2,plannedItems:5,pythonCases:1};
      if(command==="get_benchmark_regrades")return args.sessionId?{page:0,pageSize:20,total:3,results:Array.from({length:3},(_,i)=>({id:`grade-${i}`,evaluator_version:"synthetic-ui-engine",details_json:{state:i?"Failed":"Passed",regrade:{sessionId:args.sessionId,ordinal:i,modelIndex:0,caseId:`fixture-${i}`,repetition:1}},metrics_json:{syntheticOnly:true}}))}:{page:0,pageSize:20,total:gradeSessions.length,sessions:gradeSessions};
      if(command==="run_benchmark_regrade"){
        (w as any).regradeCalls++;
        if((w as any).holdRegrade)await new Promise<void>((_,reject)=>{rejectGrade=reject;});
        gradeSessions.unshift({session_id:args.sessionId,created_at:"2026-10-29T00:00:00+08:00",engine_version:"synthetic-ui-engine",result_count:3,passed:1,failed:2});
        return {sessionId:args.sessionId};
      }
      if(command==="cancel_benchmark_regrade"){(w as any).cancelledGrades++;(w as any).holdRegrade=false;rejectGrade?.("重新评分已取消，本轮没有保存新记录。");rejectGrade=null;return null;}
      if (command === "plugin:window|is_maximized") return false;
      if (command.startsWith("plugin:")) return 1;
      w.badCommands.push(command); throw new Error(`Unexpected UI command: ${command}`);
    } };
  });
});

test("report links restore pages filters selection and browser history", async ({page})=>{
  await page.goto("/#/benchmark?workspace=fixture&batch=navigation-fixture");
  const buttons=page.locator(".benchmark-result-button"), drawer=page.getByRole("dialog",{name:"单题详情",exact:true});
  await expect(buttons).toHaveCount(20);
  await page.getByRole("button",{name:"下一页",exact:true}).click();
  await expect(buttons.first()).toContainText("合成题目 20");
  await page.goBack();await expect(buttons.first()).toContainText("合成题目 0");
  await page.goForward();await expect(buttons.first()).toContainText("合成题目 20");
  await page.reload();await expect(buttons.first()).toContainText("合成题目 20");
  await page.getByLabel("重复轮次",{exact:true}).selectOption("3");
  await page.getByLabel("排序",{exact:true}).selectOption("speed");
  await expect(buttons).toHaveCount(15);await buttons.nth(5).click();
  await expect(drawer).toContainText("合成题目 35");
  await drawer.getByRole("button",{name:"复制定位链接",exact:true}).click();
  const link=await page.evaluate(()=>(window as any).copiedLink as string);
  expect(link).toContain("item=35");expect(link).toContain("repeat=3");expect(link).toContain("order=speed");
  await page.goBack();await expect(drawer).toHaveCount(0);
  await page.goForward();await expect(drawer).toContainText("合成题目 35");
  await page.reload();await expect(drawer).toContainText("合成题目 35");
  await drawer.getByRole("button",{name:"返回结果列表",exact:true}).click();
  await expect(drawer).toHaveCount(0);await expect(buttons.nth(5)).toBeFocused();
  await page.getByRole("button",{name:"打开定位链接",exact:true}).click();
  const linkDialog=page.getByRole("dialog",{name:"打开评测定位链接",exact:true});
  await linkDialog.getByLabel("评测定位链接",{exact:true}).fill("https://example.com/#/benchmark?workspace=fixture&batch=navigation-fixture");
  await linkDialog.getByRole("button",{name:"打开",exact:true}).click();await expect(linkDialog.getByRole("alert")).toBeVisible();
  await linkDialog.getByLabel("评测定位链接",{exact:true}).fill(link);await linkDialog.getByRole("button",{name:"打开",exact:true}).click();
  await expect(drawer).toContainText("合成题目 35");await page.keyboard.press("Escape");
  const reads=await page.evaluate(()=>(window as any).detailsRead.length);
  await page.evaluate(()=>{location.hash="/benchmark?workspace=fixture&batch=navigation-fixture&item=1&trace=execution-0";});
  await expect(page.getByRole("alert")).toContainText("题目或执行记录不属于");await expect(drawer).toHaveCount(0);
  expect(await page.evaluate(()=>(window as any).detailsRead.length)).toBe(reads);
  await page.evaluate(()=>{location.hash="/benchmark?workspace=fixture&batch=navigation-fixture&model=999";});
  await expect(page.getByRole("alert")).toContainText("模型");
  await page.getByRole("button",{name:"重置查看条件",exact:true}).click();await expect(buttons).toHaveCount(20);
  expect(await page.evaluate(()=>(window as any).badCommands)).toEqual([]);
});

test("personal report refresh restores selection without running tasks and late batches stay isolated",async({page})=>{
  await page.goto("/#/benchmark?workspace=fixture&view=personalized&batch=personal-fixture&repeat=3&item=35");
  const drawer=page.getByRole("dialog",{name:"单题详情",exact:true});
  await expect(drawer).toContainText("合成题目 35");
  await page.reload();await expect(drawer).toContainText("合成题目 35");await page.keyboard.press("Escape");
  await page.evaluate(()=>{location.hash="/benchmark?workspace=fixture&batch=late-fixture";});
  await expect(page.getByText("正在读取评测报告…",{exact:true})).toBeVisible();
  await page.evaluate(()=>{location.hash="/benchmark?workspace=fixture&batch=navigation-fixture";});
  await expect(page.locator(".benchmark-result-button")).toHaveCount(20);
  await page.evaluate(async()=>{(window as any).releaseBatch();await new Promise(requestAnimationFrame);});
  await expect(page.locator(".benchmark-result-button")).toHaveCount(20);
  expect(await page.evaluate(()=>new URLSearchParams(location.hash.split("?")[1]).get("batch"))).toBe("navigation-fixture");
  await page.evaluate(()=>{location.hash="/benchmark?workspace=fixture&view=personalized&batch=navigation-fixture&item=1";});
  await expect(page.getByRole("alert")).toContainText("评测不存在或不属于");await expect(drawer).toHaveCount(0);
  expect(await page.evaluate(()=>(window as any).badCommands)).toEqual([]);
});

test("drawer ignores late answers and retains long text, errors and focus", async ({ page }, info) => {
  const errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
  await page.goto("/#/benchmark?workspace=fixture&batch=drawer-fixture");
  const launchers = page.locator(".benchmark-result-button"), drawer = page.getByRole("dialog", { name: "单题详情", exact: true });
  await expect(launchers).toHaveCount(5);
  expect(await page.evaluate(() => (window as any).detailsRead)).toEqual([]);
  await launchers.first().click();
  expect(await page.locator("#main-content").evaluate(e => (e as HTMLElement).inert)).toBe(true);
  await expect(drawer.getByText("正在读取本题…", { exact: true })).toBeVisible();
  await drawer.getByRole("button", { name: "下一题", exact: true }).click();
  await expect(drawer.getByRole("heading", { name: "NEW-CURRENT-ANSWER", exact: true })).toBeVisible();
  await page.evaluate(async () => { (window as any).releaseFirst(); await new Promise(requestAnimationFrame); });
  await expect(drawer).not.toContainText("OLD-LATE-ANSWER");
  await drawer.getByRole("button", { name: "原文", exact: true }).click();
  expect((await drawer.locator(".benchmark-answer-raw").textContent())!.endsWith("RAW-END-MARKER")).toBe(true);
  await drawer.locator(".benchmark-case-sources > summary").click();
  await expect(drawer.locator(".benchmark-case-sources ol li")).toHaveCount(2);
  const header = await drawer.locator(".panel-header").boundingBox(), footer = await drawer.locator(".panel-footer").boundingBox();
  const scroll = await drawer.locator(".panel-body").evaluate(e => { e.scrollTop = e.scrollHeight; return e.scrollTop; });
  expect(scroll).toBeGreaterThan(0);
  expect((await drawer.locator(".panel-header").boundingBox())!.y).toBe(header!.y);
  expect((await drawer.locator(".panel-footer").boundingBox())!.y).toBe(footer!.y);
  await page.screenshot({ path: info.outputPath("long-original-fixed-controls.png") });
  await drawer.getByRole("button", { name: "下一题", exact: true }).click();
  await expect(drawer.getByText(/合成读取失败/)).toBeVisible();
  await expect(drawer).not.toContainText("NEW-CURRENT-ANSWER");
  await drawer.getByRole("button", { name: "重新读取本题", exact: true }).click();
  await expect(drawer.getByText(/合成运行错误/)).toBeVisible();
  await expect(drawer.getByText("本次运行没有保存文字输出。", { exact: true })).toBeVisible();
  await drawer.getByRole("button", { name: "下一题", exact: true }).click();
  await expect(drawer.getByText("尚未发送，没有模型输出。", { exact: true })).toBeVisible();
  await expect(drawer.getByRole("button", { name: "查看 3D 执行过程", exact: true })).toHaveCount(0);
  await drawer.getByRole("button", { name: "下一题", exact: true }).click();
  await expect(drawer.locator(".benchmark-result-meta strong")).toHaveText("不支持");
  await expect(drawer.getByRole("button", { name: "下一题", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(drawer).toHaveCount(0);
  await expect(launchers.first()).toBeFocused();
  expect(await page.evaluate(() => (window as any).badCommands)).toEqual([]);
  expect(errors).toEqual([]);
});


test("report loading keyboard and long content keep controls visible",async({page},info)=>{
  await page.goto("/#/benchmark?workspace=fixture&batch=presentation-fixture");
  const content=page.locator(".benchmark-content"),buttons=page.locator(".benchmark-result-button"),drawer=page.getByRole("dialog",{name:"单题详情",exact:true});
  await expect(buttons).toHaveCount(20);
  expect(await content.evaluate(e=>e.scrollTop)).toBe(0);
  await page.evaluate(()=>{(window as any).holdPage=true;});
  await page.getByRole("button",{name:"下一页",exact:true}).click();
  await expect(page.locator(".benchmark-page-count")).toHaveText("第 2 页 · 读取中…");
  await expect(page.locator(".benchmark-page-count")).not.toContainText("/ 1 页");
  await page.evaluate(()=>{(window as any).holdPage=false;(window as any).releasePage();});
  await expect(buttons.first()).toContainText("合成题目 20");
  await page.getByRole("button",{name:"上一页",exact:true}).click();await expect(buttons.first()).toContainText("合成题目 0");
  const appearance=page.getByRole("combobox",{name:"外观",exact:true});
  for(const theme of ["light","dark"]){
    await appearance.selectOption(theme);
    for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
      await page.setViewportSize({width,height});await content.evaluate(e=>e.scrollTop=0);
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
      await page.screenshot({path:info.outputPath(`report-long-name-${theme}-${width}.png`)});
      const table=page.getByRole("region",{name:"全计划模型比较表",exact:true});
      await table.focus();await page.keyboard.press("ArrowRight");await expect(table).toBeFocused();
      await buttons.last().scrollIntoViewIfNeeded();
      const actions=await page.getByRole("group",{name:"评测运行操作",exact:true}).boundingBox(),pagination=await page.locator(".benchmark-pagination").boundingBox();
      expect(actions!.y).toBeGreaterThanOrEqual(76);expect(pagination!.y).toBeGreaterThanOrEqual(actions!.y+actions!.height-1);expect(pagination!.y+pagination!.height).toBeLessThan(height);
      await page.screenshot({path:info.outputPath(`report-sticky-controls-${theme}-${width}.png`)});
      await buttons.nth(1).focus();await page.keyboard.press("Enter");await expect(drawer.getByRole("heading",{name:"NEW-CURRENT-ANSWER",exact:true})).toBeVisible();
      await drawer.getByRole("button",{name:"原文",exact:true}).click();
      expect((await drawer.locator(".benchmark-answer-raw").textContent())!.endsWith("RAW-END-MARKER")).toBe(true);
      await drawer.locator(".benchmark-case-sources > summary").click();
      const body=drawer.locator(".panel-body");await body.evaluate(e=>e.scrollTop=e.scrollHeight);
      expect(await body.evaluate(e=>e.scrollWidth<=e.clientWidth)).toBe(true);
      await page.screenshot({path:info.outputPath(`long-content-${theme}-${width}.png`)});
      await appearance.focus();await appearance.press("Enter");await expect(page.getByRole("listbox",{name:"外观选项",exact:true})).toBeVisible();
      await appearance.press("Escape");await expect(drawer).toBeVisible();await expect(appearance).toBeFocused();
      await page.keyboard.press("Escape");await expect(drawer).toHaveCount(0);await expect(buttons.nth(1)).toBeFocused();
    }
  }
  expect(await page.evaluate(()=>(window as any).badCommands)).toEqual([]);
});


test("report export cancel errors and independent regrade confirmation remain recoverable",async({page},info)=>{
 await page.goto("/#/benchmark?workspace=fixture&batch=drawer-fixture");await expect(page.locator(".benchmark-result-button")).toHaveCount(5);
 const original=await page.locator(".benchmark-result-button").allTextContents();
 await page.getByRole("button",{name:"导出报告",exact:true}).click();const exporting=page.getByRole("dialog",{name:"导出评测报告",exact:true});
 await exporting.getByRole("button",{name:"选择保存位置",exact:true}).click();await expect(page.getByText("已取消保存",{exact:true})).toBeVisible();await expect(exporting).toBeVisible();
 await exporting.getByRole("combobox",{name:"报告文件格式",exact:true}).selectOption("csv");
 await exporting.getByRole("button",{name:"选择保存位置",exact:true}).click();await expect(exporting.getByRole("alert")).toContainText("文件已存在");
 await exporting.getByRole("button",{name:"选择保存位置",exact:true}).click();await expect(exporting).toBeHidden();expect(await page.evaluate(()=>(window as any).exportFormats)).toEqual(["json","csv","csv"]);
 await page.evaluate(()=>{(window as any).holdRegrade=true;});await page.getByRole("button",{name:"重新评分",exact:true}).click();
 const grading=page.getByRole("dialog",{name:"重新评分与历史",exact:true}),confirm=grading.getByRole("button",{name:"确认并重新评分",exact:true});await expect(confirm).toBeDisabled();
 await grading.getByRole("checkbox",{name:"我确认按上述范围重新评分，保留原成绩。",exact:true}).check();await confirm.click();await expect(grading.getByText("正在对已保存回答评分，不会请求模型…",{exact:true})).toBeVisible();
 await page.keyboard.press("Escape");await expect(grading.getByRole("alert")).toContainText("已取消");await expect(grading).toBeVisible();await expect(grading.getByRole("button",{name:"查看这轮评分",exact:true})).toHaveCount(0);
 await confirm.click();await expect(grading.getByRole("button",{name:"查看这轮评分",exact:true})).toBeVisible();await expect(confirm).toBeDisabled();
 await grading.getByRole("button",{name:"查看这轮评分",exact:true}).click();await expect(grading.locator(".benchmark-grade-list > li")).toHaveCount(3);
 const appearance=page.getByRole("combobox",{name:"外观",exact:true});
 for(const theme of ["light","dark"]){await appearance.selectOption(theme);for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
  await page.setViewportSize({width,height});const header=await grading.locator(".panel-header").boundingBox();await grading.locator(".panel-body").evaluate(e=>e.scrollTop=e.scrollHeight);expect((await grading.locator(".panel-header").boundingBox())!.y).toBe(header!.y);expect(header!.y).toBeGreaterThanOrEqual(76);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);await page.screenshot({path:info.outputPath(`regrade-history-${theme}-${width}.png`)});
 }}
 await page.keyboard.press("Escape");await expect(grading).toBeHidden();expect(await page.locator(".benchmark-result-button").allTextContents()).toEqual(original);
 expect(await page.evaluate(()=>(window as any).regradeCalls)).toBe(2);
 await page.evaluate(()=>{(window as any).holdRegrade=true;});await page.getByRole("button",{name:"重新评分",exact:true}).click();await grading.getByRole("checkbox",{name:"我确认按上述范围重新评分，保留原成绩。",exact:true}).check();await confirm.click();await expect(grading.getByText("正在对已保存回答评分，不会请求模型…",{exact:true})).toBeVisible();
 await page.evaluate(()=>{location.hash="/benchmark?workspace=fixture&batch=other-fixture";});
 await expect.poll(async()=>page.evaluate(()=>(window as any).cancelledGrades)).toBe(2);await expect(grading).toHaveCount(0);
 expect(await page.evaluate(()=>(window as any).badCommands)).toEqual([]);
});
