import { expect, test } from "@playwright/test";

test.use({ headless: true });

test("fixed retrieval comparison keeps previous reports and opens evidence by case", async ({ page }, info) => {
  await page.addInitScript(() => {
    const createdAt = "2026-11-17T01:00:00Z";
    const dataset = { version: 1, caseCount: 9, documentCount: 11, snapshotDate: "2026-06-01",
      datasetHash: "a".repeat(64), snapshotHash: "b".repeat(64), scorerVersion: "cortex-rag-rubric-1", policyVersion: "cortex-rag-policy-1" };
    const history: { id: string; createdAt: string; strategies: string[]; datasetVersion: number }[] = [];
    const reports = new Map<string, unknown>();
    const sample = (policy: string) => ({ policy, summary: { recallAt5: { value: policy === "graph" ? 1 : .33, cases: 7 },
      citationPrecision: { value: .5, cases: 7 }, retrievalLatencyMs: { value: 3.23, cases: 9 } }, cases: [{
      id: "RAG-09", kind: "跨文件关系", question: "接入端点用什么资源？", expected: [["entrance",1], ["fiber",1]],
      usedReferences: [{nodeId:"entrance"}], candidates: [{ nodeId: "entrance", version: 1, title: "接入端点", included: true, reason: "included",
        sourceLocation: { name: "entrance.md", version: 1, startLine: 1, endLine: 2, match: { startLine: 2, endLine: 2 } } }],
      replay: { text: "先检查专线 [资料1]。", checks: [{ marker: "[资料1]", reference: ["entrance",1], grounded: true, supportsExpected: true }] },
    }] });
    Object.assign(window, {
      isTauri: true,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: {
        metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback() {},
        invoke: async (command: string, args: { workspaceId?: string; policyA?: string; policyB?: string; evaluationId?: string }) => {
          switch (command) {
            case "bootstrap": return { workspaces: [{ id: "w", name: "固定检索验收", createdAt }],
              settings: { model: "fixture", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 }, activeRun: null };
            case "check_ollama": return { version: "fixture", models: [] };
            case "list_conversations": return [];
            case "get_rag_evaluation":
              if (args.workspaceId !== "w") throw Error("Wrong workspace");
              return { dataset, history: [...history], report: reports.get(args.evaluationId ?? history[0]?.id) ?? null };
            case "run_rag_evaluation": {
              if (args.workspaceId !== "w" || !args.policyA || !args.policyB || args.policyA === args.policyB) throw Error("Wrong policies");
              const entry = { id: `run-${history.length + 1}`, strategies: [args.policyA,args.policyB], datasetVersion: 1, createdAt };
              const report = { ...entry, dataset, results: [sample(args.policyA), sample(args.policyB)] };
              history.unshift(entry);
              reports.set(entry.id, report);
              return report;
            }
            default: if (command.startsWith("plugin:")) return 1; throw Error(`Unexpected ${command}`);
          }
        },
      },
    });
  });
  await page.goto("/#/benchmark?workspace=w&view=rag");
  await expect(page.getByRole("link", { name: "检索评测", exact: true })).toHaveAttribute("aria-current", "page");
  await expect(page.getByText(/真实模型与模型判分尚未运行/)).toBeVisible();
  const run = page.getByRole("button", { name: "运行对照", exact: true });
  const strategyB = page.getByLabel("策略 B", { exact: true });
  await strategyB.selectOption("fts5");
  await expect(run).toBeDisabled();
  await strategyB.selectOption("graph");
  await run.click();
  const header = page.getByRole("columnheader", { name: "B 词语与知识关系", exact: true });
  await expect(header).toBeVisible();
  await strategyB.selectOption("hybrid");
  await run.click();
  const history = page.getByLabel("历史对照", { exact: true });
  await expect(history).toHaveValue("run-2");
  await history.selectOption("run-1");
  await expect(header).toBeVisible();
  await expect(strategyB).toHaveValue("graph");
  await page.getByText("逐题检查（9）", { exact: true }).click();
  await page.getByText(/跨文件关系.*接入端点用什么资源/).click();
  await expect(page.getByText("entrance.md v1，第 2–2 行", { exact: true })).toHaveCount(2);
  await page.getByText("固定回答与引用核对", { exact: true }).first().click();
  await expect(page.getByRole("listitem").filter({ hasText: "支持标准结论" })).toBeVisible();
  for (const theme of ["light", "dark"]) {
    await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
    await expect(page.locator("html")).toHaveAttribute("data-color-scheme", theme);
    await expect(header).toHaveCSS("color", theme === "dark" ? "rgb(232, 236, 245)" : "rgb(27, 34, 48)");
    for (const [width, height] of [[1366,768], [1600,900], [1920,1080]]) {
      await page.setViewportSize({ width,height });
      await page.getByRole("heading", { name: "检索结果", exact: true }).scrollIntoViewIfNeeded();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: info.outputPath(`rag-evaluation-${theme}-${width}.png`) });
    }
  }
});

test("model comparison confirms cloud scope, stops, opens recorded graph and exports", async ({ page }, info) => {
  await page.addInitScript(() => {
    const createdAt = "2026-11-17T01:00:00Z";
    const settings = { model: "fixture", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 };
    const local = { id: "local", name: "本地方案", mode: "local", generation: { kind: "local", model: "fixture" },
      embedding: { kind: "off" }, reranking: { kind: "off" }, updatedAt: createdAt };
    const remote = { ...local, id: "remote", name: "远端方案", mode: "cloud", generation: { kind: "cloud", connectionId: "c", model: "fixed-cloud" } };
    const item = { id: "RAG-01", question: "日志保留期限是多少天？" };
    const run = { id:"scratch-run", workspaceId:"fixture", conversationId:"scratch-chat", assistantMessageId:"answer",
      modelId:"fixture", status:"completed", startedAt:createdAt, finishedAt:createdAt };
    const execution = { run, settings, contextManifest: { knowledgeRefs: [], messageRefs: [] }, snapshot: null,
      events: [{ seq:1, kind:"request_received", occurredAt:createdAt, payload:{} },
        { seq:2, kind:"model_request_started", occurredAt:createdAt, payload:{} },
        { seq:3, kind:"response_completed", occurredAt:createdAt, payload:{} }] };
    let plan: unknown;
    let report: { id: string; createdAt: string; status: string; plan: unknown; results: unknown[] } | null = null;
    let stop: (() => void) | undefined;
    Object.assign(window, {
      isTauri: true,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: {
        metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback() {},
        invoke: async (command: string, args: { workspaceId?: string; id?: string; planHash?: string; input?: { arms: { profileId: string | null; policy: string }[]; caseId: string; maxCostMicros: number } }) => {
          switch (command) {
            case "bootstrap": return { workspaces: [{ id:"w", name:"模型组合验收", createdAt }], settings, activeRun:null };
            case "check_ollama": return { version:"fixture", models:[] };
            case "list_conversations": return [];
            case "get_rag_evaluation": return { dataset:{}, history:[], report:null };
            case "list_model_runtime_profiles": return { activeId:null, profiles:[local,remote] };
            case "get_rag_runtime_experiment": return { history:report ? [report] : [], report, cases:[item] };
            case "preview_rag_runtime_experiment": {
              if (args.input?.maxCostMicros !== 50000 || args.input.caseId !== item.id) throw Error("预算或题目缺失");
              const arms = args.input.arms.map(arm => ({ profile:arm.profileId === "remote" ? remote : local, strategy:arm.policy, settings,
                cloud:arm.profileId === "remote" ? [{ capability:"generation", provider:"compatible", model:"fixed-cloud", destination:"https://fixture.invalid/v1" }] : [] }));
              plan = { arms, caseIds:[item.id], containsCloudCalls:true, costCapMicros:50000, usesLocalModels:true, dataset:{version:1}, scorer:"fixed" };
              return { id:"experiment", planHash:"fixture-hash", plan };
            }
            case "run_rag_runtime_experiment": {
              if (args.planHash !== "fixture-hash" || args.id !== "experiment" || args.workspaceId !== "w") throw Error("确认范围不符");
              const usage = { cloudCalls:0, settledMicros:null, unknownChargeCalls:0, unsettledReservedMicros:null,
                knownCloudInputTokens:null, knownCloudOutputTokens:null, localInputTokens:61, localOutputTokens:9,
                systemCpuPeak:null, gpuUsedPeakBytes:null };
              const data = plan as { arms: unknown[] };
              report = { id:args.id, createdAt, status:"running", plan,
                results:[{ arm:data.arms[0], cases:[{...item, status:"completed", answer:"**日志保留 90 天。**", execution}], usage },
                  { arm:data.arms[1], cases:[{...item,status:"not_run"}], usage }] };
              await new Promise<void>(resolve => { stop = resolve; });
              report.status = "cancelled";
              return report;
            }
            case "cancel_rag_runtime_experiment": if (args.id === "experiment" && args.workspaceId === "w") stop?.(); return null;
            case "export_rag_runtime_experiment": return args.workspaceId === "w" && args.id === "experiment";
            default: if (command.startsWith("plugin:")) return 1; throw Error(command);
          }
        },
      },
    });
  });
  await page.goto("/#/benchmark?workspace=w&view=rag");
  await page.getByRole("button", { name: "模型组合对照", exact: true }).click();
  await page.getByRole("group", { name: "方案 B", exact: true }).getByLabel("运行方案").selectOption("remote");
  await page.getByLabel("题目", { exact: true }).selectOption("RAG-01");
  await page.getByLabel("费用上限（美元）").fill("0.05");
  await page.getByRole("button", { name: "查看运行计划", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "确认运行对照" });
  await expect(dialog.getByText(/合成资料.*US\$ 0\.050000/)).toBeVisible();
  await expect(dialog.getByText(/https:\/\/fixture\.invalid\/v1/)).toBeVisible();
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(page.getByRole("button", { name: "停止对照", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "查看运行计划", exact: true }).click();
  await dialog.getByRole("button", { name: "确认并运行", exact: true }).click();
  await expect(page.getByRole("heading", { name: "对照结果" })).toBeVisible();
  await page.getByRole("button", { name: "停止对照", exact: true }).click();
  await expect(page.getByLabel("历史对照", { exact: true })).toContainText(/已停止/);
  await page.getByText("RAG-01 日志保留期限是多少天？", { exact: true }).last().click();
  await expect(page.locator("strong").getByText("日志保留 90 天。")).toBeVisible();
  await page.getByRole("button", { name: "查看 A 的 3D 执行记录" }).click();
  const graph = page.getByRole("dialog", { name: "3D 执行记录" });
  await graph.getByRole("button", { name: "开始请求模型" }).click();
  await expect(graph.getByRole("region", { name: "执行节点详情" })).toBeVisible();
  await expect(graph.locator("canvas[aria-label='3D 执行画布'], .graph-empty")).toBeVisible();
  await graph.getByRole("button", { name: "关闭执行记录" }).click();
  await page.getByRole("button", { name: "导出完整报告" }).click();
  await expect(page.getByText("报告已保存")).toBeVisible();
  for (const [width,height] of [[1366,768],[1600,900],[1920,1080]]) {
    await page.setViewportSize({width,height});
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({path:info.outputPath(`rag-runtime-${width}.png`)});
  }
});
