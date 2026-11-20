import { expect,test } from "@playwright/test";
test.beforeEach(async({page})=>{
 await page.addInitScript(()=>{
  const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__TAURI_EVENT_PLUGIN_INTERNALS__:unknown;sent:number;published:number};w.isTauri=true;w.sent=0;w.published=0;w.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
  const weights={quality:5,speed:3,cost:3,resources:2,privacy:4,coding:5,context:3};const profile={version:1,action:"save",createdAt:"2026-10-27T08:00:00Z",contentHash:"fixture",profile:{domains:["software"],otherDomain:"",level:"advanced",tasks:["coding","math"],taskNotes:"核对边界条件",tools:["Python"],weights}};
  const connection={state:"verified",provider:"openai",model:"fixture-cloud",checkedAt:"2026-10-27T08:00:00Z",allowed:true,models:[]};
  const snapshot={id:"repository",name:"合成项目，仅用于界面验证",createdAt:"2026-10-27T08:00:00Z",contentHash:"a".repeat(64),profile:{languages:{Python:1},modules:["src"],dependencies:[],testFiles:[],fileCount:1,textBytes:42,skipped:{"疑似凭据":1},candidateAreas:[]},files:[{path:"src/"+"模型评测与项目知识边界回归-".repeat(10)+".py",language:"Python",content:"def add(a, b):\n    return a + b",contentHash:"b".repeat(64),symbols:[{name:"add",line:1}]}]};
  const candidate={id:"custom-fixture",revision:1,candidate:{case:{case_id:"custom-fixture",dataset_version:1,category:"math_logic",difficulty:"easy",input:{prompt:"计算 2 + 2，只输出整数。",response_schema:null,tools:[]},expected:{kind:"integer",answer:4},scoring:{id:"integer_equal",version:2,contract:{normalization:"trim_unicode_whitespace",syntax:"ascii_minus_optional_digits",integer_range:"i64"}},requirements:["completion"],fixture:[],output_format:"integer",content_hash:"f".repeat(64)},rationale:"合成题目，用于验证界面流程。",sources:[]},validation:{valid:true,supported:true,errors:[],warnings:["题意、答案和来源需人工审核。"]}};
  let job: any=null,batch:any=null;const dataset={id:"personal-fixture",version:1,contentHash:"d".repeat(64),manifest:{name:"我的专属测试集",level:"personalized",case_count:1}};
  w.__TAURI_INTERNALS__={metadata:{currentWindow:{label:"main"}},transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command:string,args:any)=>{
   if(command==="bootstrap")return{workspaces:[{id:"fixture",name:"界面验证工作区",createdAt:"2026-10-27"}],settings:{model:"fixture-local",numCtx:4096,numPredict:512,temperature:.3,timeoutSeconds:180},activeRun:null,dataPath:"isolated UI mock"};
   if(command==="list_conversations")return[];if(command==="check_ollama")return{version:"mock",models:[]};if(command==="cloud_connection"||command==="personalized_access")return connection;
   if(command==="get_benchmark_profile")return profile;if(command==="list_benchmark_repositories")return[];if(command==="import_benchmark_repository")return snapshot;
   if(command==="list_benchmark_generations")return job?[{id:job.id,status:job.status,createdAt:"2026-10-27T08:00:00Z"}]:[];
   if(command==="list_personalized_batches")return batch?[{id:batch.id,status:batch.status,createdAt:"2026-10-27T08:00:00Z",datasetVersion:1}]:[];
   if(command==="plan_benchmark_generation"){job={id:"job",status:"planned",planHash:"approved-hash",error:null,usage:null,response:null,candidates:[],plan:{binding:{provider:"openai",model:"fixture-cloud"},input:args.input,request:{system:"界面测试的合成发送计划，不发送真实请求。",prompt:JSON.stringify({profile,snippets:args.input.snippets})},inputTokenReserve:4000,estimatedCost:.02,maxRequests:1}};return job;}
   if(command==="start_benchmark_generation"){if(args.approvedPlanHash!==job.planHash)throw"计划已变化";w.sent++;job={...job,status:"review",candidates:[candidate],response:{text:"合成响应"},usage:{inputTokens:400,outputTokens:300}};return job;}
   if(command==="get_benchmark_generation")return job;
   if(command==="publish_benchmark_dataset"){if(!args.selection.length||!args.reviewer)throw"未经审核";w.published++;return dataset;}
   if(command==="list_benchmark_datasets")return w.published?[dataset]:[];
   if(command==="list_benchmark_models")return{models:[{name:"fixture-local",digest:"a".repeat(64)}]};
   if(command==="plan_personalized_batch"){batch={id:"batch",status:"planned",planHash:"batch-hash",plan:{input:args.input,binding:{provider:"openai",model:"fixture-cloud"},profile,cases:[candidate.id],cloudRequests:[],reservedCost:null,maxRequests:1},items:[{ordinal:0,modelIndex:0,caseId:candidate.id,status:"queued",runId:null,error:null}]};return batch;}
   if(command==="start_personalized_batch"){batch={...batch,status:"completed",items:[{...batch.items[0],status:"finished",runId:"synthetic-run"}],reports:[{modelIndex:0,name:"fixture-local",kind:"local",statistics:{planned:1,attempted:1,scored:1,passed:1,correctness:1,scoringCoverage:1,executionCoverage:1,executionSuccessRate:1,states:{Passed:1}},byEvaluator:{integer_equal:{planned:1,scored:1,correctness:1}},byDifficulty:{easy:{planned:1,scored:1,correctness:1}},performance:{elapsedMs:500,inputTokensObserved:100,outputTokensObserved:1,tokensPerSecond:2,costEstimate:null,unknownCostRequests:0,currency:null,memory:"unavailable"},results:[{ordinal:0,caseId:candidate.id,question:"计算 2 + 2，只输出整数。",output:"4",runId:"synthetic-run",executionRunId:"synthetic-execution",error:null,score:{state:"Passed",metrics:{correctness:1},details:{reason:"synthetic UI fixture"}}}]}],comparisonPolicy:{weights,recommendation:"仅供本测试界面验证，不是模型能力成绩。"}};return batch;}
   if(command==="get_personalized_batch"||command==="get_benchmark_overview")return batch;if(command==="export_personalized_report")return false;if(command.startsWith("plugin:"))return 1;throw new Error(`Unhandled UI fixture command: ${command}`);
  }};
 });
});
for(const [width,height] of [[1366,768],[1600,900],[1920,1080]])test(`personalized guided consent review and report at ${width}x${height}`,async({page},info)=>{
 await page.setViewportSize({width,height});await page.goto("/#/benchmark?view=personalized");await page.getByRole("button",{name:"继续",exact:true}).click();await page.getByRole("button",{name:"继续选择项目",exact:true}).click();
 await page.getByRole("button",{name:"选择文件夹",exact:true}).click();await expect(page.getByText("合成项目，仅用于界面验证",{exact:true}).first()).toBeVisible();await page.getByRole("button",{name:"加入待发送清单",exact:true}).click();
 const filename=page.locator(".personal-list .file-name");await expect(filename.locator(".file-name-extension")).toHaveText(".py");
 expect(await filename.locator(".file-name-stem").evaluate(e=>e.scrollWidth>e.clientWidth)).toBe(true);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 await page.screenshot({path:info.outputPath(`project-${width}.png`)});
 await page.getByRole("button",{name:"下一步",exact:true}).click();await page.getByLabel("每百万输入 Token 单价",{exact:true}).fill("1");await page.getByLabel("每百万输出 Token 单价",{exact:true}).fill("2");await page.getByLabel("价格依据（服务商页面或账单日期）",{exact:true}).fill("Synthetic UI fixture prices");await page.getByRole("button",{name:"预览发送计划",exact:true}).click();
 await expect(page.getByRole("button",{name:"确认并生成",exact:true})).toBeDisabled();expect(await page.evaluate(()=>(window as any).sent)).toBe(0);await page.getByRole("checkbox",{name:"我已检查资料与价格",exact:false}).check();await page.screenshot({path:info.outputPath(`consent-${width}.png`)});await page.getByRole("button",{name:"确认并生成",exact:true}).click();expect(await page.evaluate(()=>(window as any).sent)).toBe(1);
 await page.getByRole("button",{name:"查看审核",exact:true}).click();
 if(width===1366){
   await page.getByRole("button",{name:"修订题目",exact:true}).click();
   const json=page.getByLabel("修改候选 JSON",{exact:true});
   for(const invalid of ["{", "[]", '{"case":{}}']){
     await json.fill(invalid);
     await page.getByRole("button",{name:"保存新修订",exact:true}).click();
     await expect(json).toHaveAttribute("aria-invalid","true");
     await expect(page.getByText(/包含 case 对象/)).toBeVisible();
     await expect(json).toHaveValue(invalid);
   }
   await page.getByRole("button",{name:"取消编辑",exact:true}).click();
 }
 await expect(page.getByRole("button",{name:"发布已批准的 0 题",exact:true})).toBeDisabled();await page.getByRole("checkbox",{name:"我已核对本题",exact:false}).check();await page.getByLabel("审核人",{exact:true}).fill("UI fixture reviewer");await page.screenshot({path:info.outputPath(`review-${width}.png`)});await page.getByRole("button",{name:"发布已批准的 1 题",exact:true}).click();
 await page.getByRole("checkbox",{name:"fixture-local",exact:true}).check();await page.getByRole("button",{name:"预览评测计划",exact:true}).click();await expect(page.getByRole("button",{name:"确认并运行未发送题目",exact:true})).toBeDisabled();await page.getByRole("checkbox",{name:"我已检查题库、模型和发送范围",exact:false}).check();await page.getByRole("button",{name:"确认并运行未发送题目",exact:true}).click();await expect(page.getByRole("table")).toBeVisible();
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);await page.screenshot({path:info.outputPath(`report-${width}.png`)});
});
