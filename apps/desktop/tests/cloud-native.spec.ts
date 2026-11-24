import { expect, test } from "@playwright/test";
import { launchNative, nativeInvoke, createWorkspace } from "./native";

test("domestic cloud presets persist separately across restart", async ({}, info) => {
  test.setTimeout(90_000);
  const options = { endpoint: "http://127.0.0.1:1", dataDirectory: info.outputPath("data") };
  let app = await launchNative(info, options);
  let workspaceId = "";
  type Row = { id: string; label: string; address: string; provider: string; compatibleRerank: boolean; credentialAvailable: boolean };
  const list = () => nativeInvoke<Row[]>(app.page, "list_runtime_cloud_connections", { workspaceId });
  const presets = [
    { name: /DeepSeek/, address: "https://api.deepseek.com/" },
    { name: /百炼/, address: "https://dashscope.aliyuncs.com/compatible-mode/v1/" },
    { name: /硅基流动/, address: "https://api.siliconflow.cn/v1/" },
  ];
  try {
    await createWorkspace(app.page, "云端预设验收");
    workspaceId = await app.page.getByLabel("当前工作区", { exact: true }).inputValue();
    await app.page.evaluate(id => { location.hash = "/settings?section=cloud&workspace=" + id; }, workspaceId);
    for (const { name, address } of presets) {
      await app.page.getByRole("button", { name: "添加连接", exact: true }).click();
      const provider = app.page.getByLabel("服务商", { exact: true });
      const label = await provider.getByRole("option", { name }).innerText();
      await provider.selectOption({ label });
      await app.page.getByLabel("API 密钥", { exact: true }).fill("synthetic-preset-key-for-native-test");
      await app.page.getByRole("button", { name: "保存连接", exact: true }).click();
      await expect.poll(async () => (await list()).some(row => row.address === address)).toBe(true);
    }
    const saved = await list();
    expect(saved).toHaveLength(3);
    for (const { name, address } of presets) {
      const row = saved.find(item => item.address === address);
      expect(row).toMatchObject({ provider: "compatible", credentialAvailable: true, compatibleRerank: address.includes("siliconflow") });
      expect(row?.label).toMatch(name);
    }
    expect(JSON.stringify(saved)).not.toContain("synthetic-preset-key");
    await app.page.screenshot({ path: info.outputPath("presets-native.png") });
    await app.closeWindow();
    await app.stop();
    app = await launchNative(info, options);
    expect(await list()).toEqual(saved);
    const other = await nativeInvoke<{ id: string }>(app.page, "create_workspace", { name: "单独工作区" });
    expect(await nativeInvoke(app.page, "list_runtime_cloud_connections", { workspaceId: other.id })).toEqual([]);
  } finally {
    try {
      if (workspaceId && !app.page.isClosed()) {
        for (const row of await list()) {
          await nativeInvoke(app.page, "remove_runtime_cloud_connection", { workspaceId, connectionId: row.id });
        }
      }
    } finally { await app.stop(); }
  }
});

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
