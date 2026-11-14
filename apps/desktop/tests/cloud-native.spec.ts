import { expect, test } from "@playwright/test";
import { launchNative, nativeInvoke, createWorkspace } from "./native";

test("native personalized guard denies direct calls and leaves local datasets and navigation available", async ({}, info) => {
  test.setTimeout(90_000);
  const app = await launchNative(info);
  try {
    const { page } = app;
    await createWorkspace(page, "C1 隔离验收");
    const before = await nativeInvoke<{workspaces: {id: string; name: string}[]}>(page, "bootstrap");
    const workspace = before.workspaces.find(item => item.name === "C1 隔离验收")!;
    const status = await nativeInvoke<{state: string; allowed: boolean}>(page, "cloud_connection");
    expect(status.state).toBe("unconfigured"); expect(status.allowed).toBe(false);
    const deny = async () => {
      try { await nativeInvoke(page, "personalized_access", { allowed: true }); return false; } catch { return true; }
    };
    expect(await deny()).toBe(true);
    const price={currency:"USD",inputPerMillion:1,outputPerMillion:2,budget:10,reference:"synthetic fixture price"};
    const protectedCalls:[string,Record<string,unknown>][]=[
      ["import_benchmark_repository",{kind:"folder"}],
      ["list_benchmark_repositories",{}],["get_benchmark_repository",{snapshotId:"missing"}],
      ["plan_benchmark_generation",{input:{profileVersion:1,repositoryId:null,sharing:"summary",snippets:[],count:1,difficulty:"easy",categories:["integer_equal"],maxOutputTokens:512,price}}],
      ["get_benchmark_generation",{jobId:"missing"}],["list_benchmark_generations",{}],
      ["start_benchmark_generation",{jobId:"missing",approvedPlanHash:"x"}],
      ["publish_benchmark_dataset",{jobId:"missing",selection:[],reviewer:"tester"}],
      ["plan_personalized_batch",{input:{datasetId:"personal-missing",datasetVersion:1,datasetHash:"a".repeat(64),models:[{kind:"cloud",local:null}],maxOutputTokens:512,price,pythonImage:null}}],
      ["get_personalized_batch",{batchId:"missing"}],["list_personalized_batches",{}],
      ["start_personalized_batch",{batchId:"missing",approvedPlanHash:"x"}],
      ["export_personalized_report",{batchId:"missing"}],
    ];
    for(const [command,args] of protectedCalls){
      const rejection=await nativeInvoke(page,command,{workspaceId:workspace.id,...args}).then(()=>null,e=>String(e));
      expect(rejection,command).toMatch(/验证|连接/);
    }
    for(const command of ["get_benchmark_profile","save_benchmark_profile"]) {
      const denied=await nativeInvoke(page,command,{workspaceId:workspace.id,input:{expectedVersion:0,action:"reset",profile:{domains:[],otherDomain:"",level:null,tasks:[],taskNotes:"",tools:[],weights:{quality:3,speed:3,cost:3,resources:3,privacy:3,coding:3,context:3}}}}).then(()=>false,()=>true);
      expect(denied).toBe(true);
    }
    await page.evaluate(() => { location.hash = "/benchmark?view=personalized&allowed=true"; });
    await expect(page.getByRole("heading", { name: "为你的任务设计评测" })).toBeVisible();
    await expect(page.getByRole("button", { name: "继续" })).toHaveCount(0);
    await page.reload();
    await expect(page.getByText("尚未连接", { exact: true })).toBeVisible();
    expect(await deny()).toBe(true);
    const datasets = await nativeInvoke<{version: number; manifest: {case_count: number}}[]>(page, "list_benchmark_datasets", { workspaceId: workspace.id });
    expect(datasets.map(item => item.version)).toEqual([1,2]);
    expect(datasets.every(item => item.manifest.case_count === 60)).toBe(true);
    expect(await nativeInvoke(page,"bootstrap")).toEqual(before);
    for (const [width,height] of [[1366,768],[1600,900],[1920,1080]]) {
      await page.setViewportSize({width,height});
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({path:info.outputPath(`native-cloud-${width}.png`)});
    }
    for (const name of ["对话","知识","图谱","设置"]) {
      await page.getByRole("navigation",{name:"主导航"}).getByRole("link",{name,exact:true}).click();
      await expect(page.locator(".topbar-title strong")).toBeVisible();
      if(name!=="对话")await expect(page.locator(".topbar-title strong")).toHaveText(name);
    }
    expect(app.consoleErrors).toEqual([]);
  } finally { await app.stop(); }
});
