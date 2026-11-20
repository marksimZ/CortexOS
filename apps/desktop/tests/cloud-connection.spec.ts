import { expect, test } from "@playwright/test";
import type { Profile } from "../src/benchmark/ProfileEditor";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; mockFailure: string | null; mockDelay: number; guardCalls: number };
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    w.isTauri = true; w.mockFailure = null; w.mockDelay = 100; w.guardCalls = 0;
    let connection = { state: "unconfigured", provider: null as string | null, model: null as string | null, allowed: false, checkedAt: null as string | null, models: [] as { id: string; generation: string }[] };
    const empty = () => ({ state: "unconfigured", provider: null, model: null, allowed: false, checkedAt: null, models: [] });
    let profile: {workspaceId:string;version:number;action:string;profile:Profile;createdAt:string;contentHash:string}|null=null;
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback: () => {},
      invoke: async (command: string, args: { input?: { provider: string; model: string | null; apiKey: string | null; expectedVersion:number;action:string;profile:Profile };workspaceId:string }) => {
        if (command === "bootstrap") return { workspaces: [{ id: "fixture", name: "测试工作区", createdAt: "2026-10-25" }], settings: { model: "", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 }, activeRun: null, dataPath: "isolated mock" };
        if (["list_conversations","list_benchmark_generations","list_personalized_batches","list_standard_benchmarks","list_standard_benchmark_cases"].includes(command)) return [];
        if (command === "check_ollama") return { version: "mock", models: [] };
        if (command === "list_benchmark_models") return {models:[]};
        if (command === "list_benchmark_datasets") return [{ id: "cortex-core-60", version: 2, manifest: { name: "Core 60 · 测试替身", case_count: 60 } }];
        if (command === "cloud_connection") return connection;
        if (command === "get_benchmark_profile") {if(!connection.allowed)throw "请先验证云端连接";return profile;}
        if (command === "save_benchmark_profile") {
          if(!connection.allowed)throw "请先验证云端连接";
          const input=args.input!;
          if(input.expectedVersion!==(profile?.version??0))throw "版本冲突";
          const value=input.action==="save"?input.profile:{domains:[],otherDomain:"",level:null,tasks:[],taskNotes:"",tools:[],weights:Object.fromEntries(["quality","speed","cost","resources","privacy","coding","context"].map(k=>[k,3]))};
          profile={workspaceId:args.workspaceId,version:(profile?.version??0)+1,action:input.action,profile:structuredClone(value),createdAt:new Date().toISOString(),contentHash:"mock"};return profile;
        }
        if (command === "verify_cloud_connection") {
          const input = args.input!;
          await new Promise(resolve => setTimeout(resolve, w.mockDelay));
          connection = { state: w.mockFailure ?? (input.model ? "verified" : "model_required"), provider: input.provider, model: input.model, allowed: !w.mockFailure && !!input.model, checkedAt: "2026-10-25T00:00:00Z", models: w.mockFailure ? [] : [{ id: "fixture-model", generation: "unknown" }] };
          return connection;
        }
        if (command === "personalized_access") { w.guardCalls++; if (!connection.allowed) throw new Error("guard denied"); return connection; }
        if (command === "remove_cloud_connection") { connection = empty(); return connection; }
        if (command === "cancel_cloud_verification") return { ...connection, state: "cancelled", allowed: false };
        if (command.startsWith("plugin:")) return 1;
        throw new Error(`Unexpected mock command: ${command}`);
      },
    };
  });
});

test("chat window controls stay in the app toolbar above the evidence panel",async({page},info)=>{
  await page.goto("/#/chat");
  const controls=page.getByRole("group",{name:"窗口操作",exact:true});
  const panel=page.getByRole("complementary",{name:"回答依据",exact:true});
  for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]){
    await page.setViewportSize({width,height});
    const toolbar=await page.locator(".prototype-topbar").boundingBox();
    const box=await controls.boundingBox();const evidence=await panel.boundingBox();
    expect(box!.y).toBeGreaterThanOrEqual(toolbar!.y);
    expect(box!.y+box!.height).toBeLessThanOrEqual(toolbar!.y+toolbar!.height);
    expect(box!.y+box!.height).toBeLessThan(evidence!.y);
    await page.screenshot({path:info.outputPath(`chat-controls-${width}.png`)});
    await page.getByRole("button",{name:"收起回答依据",exact:true}).click();
    const closed=await controls.boundingBox();expect(closed!.y).toBe(box!.y);expect(closed!.x).toBe(box!.x);
    await page.getByRole("button",{name:"打开回答依据",exact:true}).click();
  }
});

test("cloud onboarding guards deep links, clears input, uses actual catalog and keeps standard access", async ({ page }) => {
  await page.goto("/#/benchmark?view=personalized&allowed=true");
  await expect(page.getByRole("heading", { name: "为你的任务设计评测" })).toBeVisible();
  await expect(page.getByRole("button", { name: "继续" })).toHaveCount(0);
  await page.getByLabel("API Key", { exact: true }).fill("synthetic-ui-key");
  await page.getByRole("button", { name: "验证密钥并读取模型" }).click();
  await expect(page.getByLabel("API Key", { exact: true })).toHaveValue("");
  await expect(page.getByText("密钥验证通过，请选择模型", { exact: true })).toBeVisible();
  await page.getByRole("combobox", { name: "题目生成模型", exact: true }).selectOption("fixture-model");
  await page.getByRole("button", { name: "验证模型并连接" }).click();
  await expect(page.getByText("连接已验证", { exact: true }).first()).toBeVisible();
  await page.getByRole("button", { name: "继续" }).click();
  await expect(page.getByRole("heading", { name: "使用偏好",exact:true })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { guardCalls: number }).guardCalls)).toBe(1);
  expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain("synthetic-ui-key");
  await page.getByRole("link", { name: "标准评测",exact:true }).click();
  await expect(page.getByRole("heading",{name:"Core 60 · 测试替身 · v2",exact:true})).toBeVisible();
  await page.getByRole("link", { name: "个性化评测", exact: true }).click();
  await page.getByRole("button", { name: "连接设置",exact:true }).click();
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "解除绑定" }).click();
  await expect(page.getByText("尚未连接", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "继续" })).toHaveCount(0);
});

test("cloud errors remain visible and connection layout fits light and dark window sizes", async ({ page }, testInfo) => {
  await page.goto("/#/benchmark?view=personalized");
  await page.evaluate(() => { (window as unknown as { mockFailure: string }).mockFailure = "network_unavailable"; });
  await page.getByLabel("API Key", { exact: true }).fill("synthetic-only");
  await page.getByRole("button", { name: "验证密钥并读取模型" }).click();
  await expect(page.getByText("暂时无法连接服务商，请检查网络后重试；密钥已保留")).toBeVisible();
  await expect(page.getByRole("button", { name: "继续" })).toHaveCount(0);
  for (const [width, height] of [[1366,768],[1600,900],[1920,1080],[800,600]]) {
    await page.setViewportSize({ width, height });
    for (const theme of ["light", "dark"]) {
      await page.evaluate(theme => document.documentElement.dataset.colorScheme = theme, theme);
      await expect(page.getByRole("heading", { name: "为你的任务设计评测" })).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.getByRole("button", { name: "重新验证 / 读取模型" }).scrollIntoViewIfNeeded();
      await expect(page.getByRole("button", { name: "重新验证 / 读取模型" })).toBeInViewport();
      await page.screenshot({ path: testInfo.outputPath(`cloud-${width}-${theme}.png`) });
    }
  }
});

test("cloud cancel ignores a late mock response", async ({ page }) => {
  await page.goto("/#/benchmark?view=personalized");
  await page.evaluate(() => { (window as unknown as {mockDelay: number}).mockDelay = 800; });
  await page.getByLabel("API Key", {exact:true}).fill("synthetic-ui-key");
  await page.getByRole("button",{name:"验证密钥并读取模型"}).click();
  await page.getByRole("button",{name:"取消验证"}).click();
  await expect(page.getByText("验证已取消，可重新验证或解除绑定")).toBeVisible();
  await expect(page.getByRole("button",{name:"取消验证"})).toHaveCount(0);
  await expect(page.getByText("验证已取消，可重新验证或解除绑定")).toBeVisible();
  await expect(page.getByRole("button",{name:"继续",exact:true})).toHaveCount(0);
});

test("profile questionnaire saves multiple domains, editable weights, skip and versioned reset", async ({page},info)=>{
  await page.goto("/#/benchmark?view=personalized");
  await page.getByLabel("API Key",{exact:true}).fill("synthetic-profile-key");
  await page.getByRole("button",{name:"验证密钥并读取模型"}).click();
  await page.getByRole("combobox",{name:"题目生成模型",exact:true}).selectOption("fixture-model");
  await page.getByRole("button",{name:"验证模型并连接"}).click();
  await page.getByRole("button",{name:"继续",exact:true}).click();
  await page.getByRole("checkbox",{name:"软件开发",exact:true}).check();
  await page.getByRole("checkbox",{name:"科学研究",exact:true}).check();
  await page.getByRole("combobox",{name:"专业水平",exact:true}).selectOption("advanced");
  for(const [width,height] of [[1366,768],[1600,900],[1920,1080]]) {
    await page.setViewportSize({width,height});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.screenshot({path:info.outputPath(`profile-${width}.png`)});
  }
  await page.getByRole("button",{name:"下一步",exact:true}).click();
  await page.setViewportSize({width:1366,height:768});
  await page.screenshot({path:info.outputPath("profile-tasks.png")});
  await page.getByRole("checkbox",{name:"代码审查",exact:true}).check();
  await page.getByLabel("补充你的任务",{exact:true}).fill("分析边界条件，保留中文草稿。");
  await page.getByLabel("工具清单（用逗号分隔，最多 20 个）",{exact:true}).fill("Rust, SQL, 自定义工具");
  await page.getByRole("button",{name:"下一步",exact:true}).click();
  await page.getByRole("slider",{name:"隐私保护",exact:true}).fill("5");
  await page.evaluate(()=>{document.documentElement.dataset.colorScheme="dark";});
  const lastWeight = page.getByRole("slider", { name: "长上下文能力", exact: true });
  const lastWeightBottom = await lastWeight.evaluate(element => element.getBoundingClientRect().bottom);
  const footerTop = await page.locator(".profile-footer").evaluate(element => element.getBoundingClientRect().top);
  expect(lastWeightBottom).toBeLessThan(footerTop);
  await page.screenshot({path:info.outputPath("profile-weights-dark.png")});
  await page.setViewportSize({ width: 800, height: 600 });
  const previousWeight = await lastWeight.inputValue();
  await lastWeight.press("ArrowRight");
  await expect(lastWeight).toHaveValue(String(Number(previousWeight) + 1));
  await lastWeight.press("ArrowLeft");
  await lastWeight.scrollIntoViewIfNeeded();
  await expect(lastWeight).toBeInViewport();
  const submit = page.getByRole("button", { name: "保存使用偏好", exact: true });
  await submit.scrollIntoViewIfNeeded();
  await expect(submit).toBeInViewport();
  await page.screenshot({ path: info.outputPath("profile-weights-dark-800.png") });
  await submit.press("Enter");
  await page.setViewportSize({ width: 1366, height: 768 });
  await expect(page.getByText("软件开发、科学研究",{exact:true})).toBeVisible();
  await expect(page.getByText("Rust、SQL、自定义工具",{exact:true})).toBeVisible();
  await expect(page.getByText("隐私保护：5 / 5",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"修改使用偏好",exact:true}).click();
  await page.getByLabel("其他领域",{exact:true}).fill("保留草稿");
  await page.getByRole("link",{name:"标准评测",exact:true}).click();
  await page.getByRole("link",{name:"个性化评测",exact:true}).click();
  await expect(page.getByLabel("其他领域",{exact:true})).toHaveValue("保留草稿");
  await page.getByRole("button",{name:"3. 选择模型的偏好",exact:true}).click();
  await page.getByRole("button",{name:"保存使用偏好",exact:true}).click();
  page.once("dialog",d=>d.accept());
  await page.getByRole("button",{name:"重置使用偏好",exact:true}).click();
  await expect(page.getByRole("checkbox",{name:"软件开发",exact:true})).not.toBeChecked();
  page.once("dialog",d=>d.accept());
  await page.getByRole("button",{name:"暂时跳过",exact:true}).click();
  await expect(page.getByText("已跳过问卷，当前使用默认偏好。",{exact:true})).toBeVisible();
  await expect(page.getByText(/版本 4 ·/)).toBeVisible();
});
