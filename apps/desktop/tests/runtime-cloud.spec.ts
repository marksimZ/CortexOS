import { expect, test } from "@playwright/test";

test.use({ headless: true });

test("Chinese cloud presets keep keys masked and select models per connection", async ({ page }, info) => {
  const presets = [
    { id: "deepseek", label: "DeepSeek 官方", address: "https://api.deepseek.com", rerank: false },
    { id: "bailian", label: "阿里云百炼（北京）", address: "https://dashscope.aliyuncs.com/compatible-mode/v1", rerank: false },
    { id: "siliconflow", label: "硅基流动", address: "https://api.siliconflow.cn/v1", rerank: true },
  ];
  await page.addInitScript(presets => {
    const rows = new Map<string, {
      id: string; label: string; provider: string; address: string; compatibleRerank: boolean;
      catalogState: string; models: object[]; probes: object[]; limits: object[];
    }>();
    const activity: string[] = [];
    const unexpected: string[] = [];
    Object.assign(window, {
      isTauri: true,
      presetAcceptance: { activity, unexpected },
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: { metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback() {},
        invoke: async (command: string, args: { workspaceId?: string; connectionId?: string; input?: Record<string, unknown> }) => {
          if (command === "bootstrap") return {
            workspaces: [{ id: "w", name: "预设验收" }, { id: "other", name: "隔离工作区" }],
            settings: { model: "", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 },
            activeRun: null,
          };
          if (command === "check_ollama") return { version: "fixture", models: [] };
          if (command === "list_conversations") return [];
          if (command === "rag_index_status") return { index: null };
          if (command.startsWith("plugin:")) return 1;
          if (command === "model_cloud_privacy_rules") return { blockEmail: false, blockPhone: false };
          if (command === "list_runtime_cloud_connections") return args.workspaceId === "w" ? [...rows.values()] : [];
          if (args.workspaceId !== "w") throw Error("Unexpected workspace");
          if (command === "add_runtime_cloud_connection") {
            const input = args.input;
            const preset = presets.find(item => item.address === input?.address);
            if (!preset || input?.provider !== "compatible" || input.label !== preset.label
              || input.apiKey !== "synthetic-" + preset.id || input.compatibleRerank !== preset.rerank) {
              unexpected.push("Malformed preset payload");
              throw Error("Malformed preset payload");
            }
            const row = {
              id: preset.id, label: preset.label, provider: "compatible", address: preset.address,
              compatibleRerank: preset.rerank, catalogState: "unconfigured",
              models: [], probes: [], limits: [], credentialAvailable: true,
            };
            rows.set(row.id, row);
            activity.push("save:" + row.id);
            return row;
          }
          const row = rows.get(args.connectionId ?? "");
          if (command === "list_model_cloud_knowledge" && row) return { grants: [], candidates: [] };
          if (command === "refresh_runtime_cloud_catalog" && row) {
            activity.push("catalog:" + row.id);
            row.catalogState = "verified";
            row.models = [{
              id: row.id + "-catalog-fixture", generation: "listed", embedding: "unknown", reranking: "unknown",
              maxInputTokens: null, maxOutputTokens: null,
            }];
            return row;
          }
          unexpected.push(command);
          throw Error("Unexpected mock command: " + command);
        },
      },
    });
  }, presets);

  await page.goto("/#/settings?workspace=w");
  await page.getByRole("button", { name: "云端模型", exact: true }).click();
  const cloud = page.getByRole("region", { name: "云端连接", exact: true });
  const selected = cloud.getByRole("combobox", { name: "当前工作区的连接", exact: true });
  const activity = () => page.evaluate(() => (window as unknown as {
    presetAcceptance: { activity: string[]; unexpected: string[] };
  }).presetAcceptance);
  const expected: string[] = [];
  for (const preset of presets) {
    await cloud.getByRole("button", { name: "添加连接", exact: true }).click();
    const key = cloud.getByLabel("API 密钥", { exact: true });
    await key.fill("synthetic-previous-provider");
    await cloud.getByLabel("服务商", { exact: true }).selectOption(preset.id);
    await expect(cloud.getByLabel("服务商", { exact: true }).locator("option:checked")).toHaveText(preset.label);
    const address = cloud.getByLabel("HTTPS 服务地址", { exact: true });
    await expect(address).toHaveValue(preset.address);
    await expect(address).not.toBeEditable();
    await expect(cloud.getByRole("link", { name: "获取 API 密钥", exact: true })).toHaveAttribute("href", /^https:\/\//);
    await page.screenshot({ path: info.outputPath(preset.id + "-setup.png") });
    expect(await activity()).toEqual({ activity: expected, unexpected: [] });
    await expect(key).toHaveAttribute("type", "password");
    await expect(key).toHaveValue("");
    await key.fill("synthetic-" + preset.id);
    await cloud.getByRole("button", { name: "保存连接", exact: true }).click();
    await expect(cloud.getByText("已保存连接。", { exact: true })).toBeVisible();
    await expect(key).toHaveCount(0);
    await expect(selected).toHaveValue(preset.id);
    await expect(cloud.getByText(preset.address, { exact: false })).toBeVisible();
    expected.push("save:" + preset.id);
    expect(await activity()).toEqual({ activity: expected, unexpected: [] });
    await cloud.getByLabel("模型标识", { exact: true }).fill(preset.id + "-manual-fixture");
    await cloud.getByRole("button", { name: "刷新模型列表", exact: true }).click();
    await cloud.getByLabel("模型", { exact: true }).selectOption(preset.id + "-catalog-fixture");
    await expect(cloud.getByRole("button", { name: "验证生成回答", exact: true })).toBeVisible();
    await expect(cloud.getByRole("button", { name: "验证重排结果", exact: true })).toHaveCount(preset.rerank ? 1 : 0);
    expected.push("catalog:" + preset.id);
    expect(await activity()).toEqual({ activity: expected, unexpected: [] });
    await page.screenshot({ path: info.outputPath(preset.id + "-catalog.png") });
  }
  await selected.selectOption("deepseek");
  await expect(cloud.getByLabel("模型", { exact: true })).toHaveValue("");
  await cloud.getByLabel("模型", { exact: true }).selectOption("deepseek-catalog-fixture");
  await page.evaluate(() => { location.hash = "/settings?workspace=other"; });
  await expect(page.getByRole("banner").getByText("隔离工作区", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "云端模型", exact: true }).click();
  await expect(cloud.getByText("当前工作区还没有云端连接。", { exact: true })).toBeVisible();
  await page.evaluate(() => { location.hash = "/settings?workspace=w"; });
  await expect(page.getByRole("banner").getByText("预设验收", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "云端模型", exact: true }).click();
  await expect(selected).toHaveValue("deepseek");
  const persisted = await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }));
  for (const { id } of presets) {
    expect(persisted).not.toContain("synthetic-" + id);
    await expect(page.locator("body")).not.toContainText("synthetic-" + id);
  }
});

test("cloud capabilities require an explicit probe and keep workspace scope", async ({ page }, info) => {
  await page.addInitScript(() => {
    const settings = { model: "fixture", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 };
    const model = { id: "models/test-embedding", generation: "unsupported", embedding: "listed", reranking: "unsupported", maxInputTokens: 2048, maxOutputTokens: null };
    let record = {
      id: "connection", provider: "gemini", label: "测试账号", address: null, compatibleRerank: false,
      catalogState: "unconfigured", catalogCheckedAt: "", models: [] as (typeof model)[],
      limits: [] as unknown[], probes: [] as unknown[],
    };
    let saved = false;
    let probes = 0;
    let builds = 0;
    let privacy = { blockEmail: false, blockPhone: false };
    Object.assign(window, {
      isTauri: true,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: { metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback() {},
        invoke: async (command: string, args: { workspaceId?: string; model?: string; capability?: string; connectionId?: string; input?: Record<string, unknown> }) => {
          if (command === "bootstrap") return { workspaces: [{ id: "w", name: "甲工作区" }, { id: "other", name: "乙工作区" }], settings, activeRun: null };
          if (command === "check_ollama") return { version: "fixture", models: [] };
          if (command === "list_conversations") return [];
          if (command.startsWith("plugin:")) return 1;
          if (command === "list_runtime_cloud_connections") return args.workspaceId === "w" && saved ? [record] : [];
          if (args.workspaceId !== "w") throw Error("Unscoped command");
          if (command === "model_cloud_privacy_rules") {
            if (args.input) {
              if (args.input.blockEmail !== true || args.input.blockPhone !== false) throw Error("Unexpected rules");
              privacy = { blockEmail: true, blockPhone: false };
            }
            return privacy;
          }
          if (command === "add_runtime_cloud_connection") {
            if (args.input?.apiKey !== "fake-key-for-fixture" || args.input.provider !== "gemini") throw Error("Malformed new connection");
            saved = true;
          } else {
            if (!saved || (args.connectionId ?? args.input?.connectionId) !== "connection") throw Error("Unscoped connection");
            if (command === "refresh_runtime_cloud_catalog") record = { ...record, catalogState: "verified", catalogCheckedAt: "2026-11-17T07:10:00Z", models: [model] };
            else if (command === "probe_runtime_cloud_model") {
              if (args.capability !== "embedding" || args.model !== model.id || ++probes !== 1) throw Error("Unexpected inference");
              record = { ...record, probes: [{ model: model.id, capability: "embedding", state: "verified", checkedAt: "2026-11-17T07:11:00Z", dimensions: 768 }] };
            } else if (command === "set_runtime_cloud_model_limits") {
              if (args.input?.inputMicroUsdPerMillion !== 250_000 || args.input.maxInputTokens !== 2000) throw Error("Unexpected price or limit");
              record = { ...record, limits: [args.input] };
            } else if (command === "get_cloud_rag_index") return { index: null, readyVersions: 0 };
            else if (command === "list_model_cloud_knowledge") {
              return { grants: [{ nodeId: "doc", version: 2, title: "已授权知识", available: true, grantedAt: "2026-11-17T07:10:00Z" }], candidates: [] };
            } else if (command === "build_cloud_rag_index") {
              if (++builds !== 1 || args.input?.model !== model.id
                || JSON.stringify(args.input.versions) !== JSON.stringify([{ nodeId: "doc", version: 2 }])) throw Error("Wrong build scope");
              return { index: { dimensions: 768 }, readyVersions: 1, build: { vectorCount: 2, requests: 1, inputTokens: 7 } };
            } else if (command === "remove_runtime_cloud_connection") {
              saved = false;
              return;
            } else throw Error(command);
          }
          return record;
        },
      },
    });
  });

  await page.goto("/#/settings?workspace=w");
  await page.getByRole("button", { name: "云端模型", exact: true }).click();
  await page.getByText("发送隐私检查", { exact: true }).click();
  await page.getByRole("checkbox", { name: "拦截常见邮箱地址", exact: true }).check();
  await page.getByRole("button", { name: "保存隐私检查", exact: true }).click();
  await expect(page.getByText("已保存隐私检查。", { exact: true })).toBeVisible();
  await page.screenshot({ path: info.outputPath("cloud-privacy-settings.png") });
  await page.getByText("发送隐私检查", { exact: true }).click();
  await page.getByRole("button", { name: "添加连接", exact: true }).click();
  await page.getByLabel("服务商", { exact: true }).selectOption("gemini");
  await page.getByLabel("连接名称", { exact: true }).fill("测试账号");
  await page.getByLabel("API 密钥", { exact: true }).fill("fake-key-for-fixture");
  await page.getByRole("button", { name: "保存连接", exact: true }).click();
  await expect(page.getByRole("status", { name: "" }).filter({ hasText: "已保存连接。" })).toBeVisible();
  await expect(page.getByLabel("API 密钥", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "刷新模型列表", exact: true }).click();
  const cloud = page.getByRole("region", { name: "云端连接", exact: true });
  await cloud.getByLabel("模型", { exact: true }).selectOption("models/test-embedding");
  await expect(page.getByRole("button", { name: "验证生成回答", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "验证生成向量", exact: true }).click();
  await expect(page.getByText(/服务商可能收费/)).toBeVisible();
  await page.getByRole("button", { name: "取消", exact: true }).click();
  await expect(page.getByText(/768 维/)).toHaveCount(0);
  await page.getByRole("button", { name: "验证生成向量", exact: true }).click();
  await page.getByRole("button", { name: "开始验证", exact: true }).click();
  await expect(page.getByText(/768 维/)).toBeVisible();
  await page.getByText("云端向量索引", { exact: true }).click();
  const buildButton = page.getByRole("button", { name: "构建云端索引…", exact: true });
  await buildButton.click();
  const confirm = page.getByRole("dialog", { name: "确认构建云端向量索引" });
  await expect(confirm.getByText(/选中的 1 个授权知识版本/)).toBeVisible();
  await page.screenshot({ path: info.outputPath("cloud-index-consent.png") });
  await confirm.getByRole("button", { name: "取消", exact: true }).click();
  await expect(page.getByText("尚未建立索引", { exact: true })).toBeVisible();
  await buildButton.click();
  await confirm.getByRole("button", { name: "确认发送并构建", exact: true }).click();
  await expect(page.getByText("1 个知识版本可用于向量查找", { exact: true })).toBeVisible();
  await page.getByText("云端向量索引", { exact: true }).click();
  await page.getByText("模型上限与费率", { exact: true }).click();
  await page.getByLabel("输入上限（Token）", { exact: true }).fill("2000");
  await page.getByLabel("每百万输入 Token（美元）", { exact: true }).fill("0.25");
  await page.getByRole("button", { name: "保存模型参数", exact: true }).click();
  await expect(page.getByText("已保存模型上限和费率。", { exact: true })).toBeVisible();
  await page.getByText("模型上限与费率", { exact: true }).click();
  for (const theme of ["light", "dark"]) {
    await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
    await expect(page.locator("html")).toHaveAttribute("data-color-scheme", theme);
    for (const [width, height] of [[1366, 768], [1920, 1080]]) {
      await page.setViewportSize({ width, height });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: info.outputPath("cloud-" + theme + "-" + width + ".png") });
    }
  }
  await page.evaluate(() => { location.hash = "/settings?workspace=other"; });
  await expect(page.getByRole("banner").getByText("乙工作区", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "云端模型", exact: true }).click();
  await expect(page.getByText("当前工作区还没有云端连接。", { exact: true })).toBeVisible();
  await page.evaluate(() => { location.hash = "/settings?workspace=w"; });
  await expect(page.getByRole("banner").getByText("甲工作区", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "云端模型", exact: true }).click();
  await expect(cloud.getByRole("combobox", { name: "当前工作区的连接" })).toHaveValue("connection");
  await page.getByRole("button", { name: "移除连接", exact: true }).click();
  await page.getByRole("button", { name: "确认移除", exact: true }).click();
  await expect(page.getByText("当前工作区还没有云端连接。", { exact: true })).toBeVisible();
});

test("resource panel distinguishes measured usage, unknown costs and missing GPU data", async ({ page }, info) => {
  await page.addInitScript(() => {
    const settings = { model: "fixture", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 };
    Object.assign(window, {
      isTauri: true,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: { metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback() {},
        invoke: async (command: string, args: { hours?: number; workspaceId?: string }) => {
          if (command === "bootstrap") return { workspaces: [{ id: "w", name: "项目甲" }, { id: "other", name: "项目乙" }], settings, activeRun: null };
          if (command === "check_ollama") return { version: "fixture", models: [] };
          if (command === "list_conversations") return [];
          if (command.startsWith("plugin:")) return 1;
          if (command === "model_runtime_resources") return { sampledAt: "2026-11-17T07:11:00Z", cpuPercent: 19, systemMemory: { usedBytes: 8 * 1024 ** 3, totalBytes: 16 * 1024 ** 3 }, gpus: null };
          if (command === "model_runtime_usage") return {
            rows: args.workspaceId === "w" && args.hours === 24 ? [{
              model: "fixture-embedding", provider: "gemini", capability: "embedding", requests: 3, completed: 2,
              inputTokens: 16, inputReported: 2, outputTokens: null, outputReported: 0, vectors: 5,
              averageMs: 120, totalMs: 360, settledMicros: 850, reservedMicros: null, unpriced: 1, systemCpuPeak: null,
            }] : [],
            retrieval: { count: 2, averageMs: 8 },
            budget: { accountedMicros: 850, singleRunMicros: null, rollingDayMicros: 50000, unpriced: 1 },
          };
          throw Error("Unexpected " + command);
        },
      },
    });
  });
  await page.goto("/#/settings?workspace=w");
  await page.getByRole("button", { name: "资源与用量", exact: true }).click();
  await expect(page.getByText("fixture-embedding", { exact: true })).toBeVisible();
  await expect(page.getByText("1 次费用未知", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "读取当前占用", exact: true }).click();
  await expect(page.getByText("19%", { exact: true })).toBeVisible();
  await expect(page.getByText("暂无驱动读数", { exact: true })).toBeVisible();
  await page.setViewportSize({ width: 1366, height: 768 });
  await page.screenshot({ path: info.outputPath("runtime-resources.png") });
  await page.getByLabel("记录范围", { exact: true }).selectOption("168");
  await expect(page.getByText("当前范围暂无模型用量。", { exact: true })).toBeVisible();
  await page.evaluate(() => { location.hash = "/settings?workspace=other"; });
  await expect(page.getByRole("banner").getByText("项目乙", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "资源与用量", exact: true }).click();
  await expect(page.getByText("当前范围暂无模型用量。", { exact: true })).toBeVisible();
  await expect(page.getByText("fixture-embedding", { exact: true })).toHaveCount(0);
});
