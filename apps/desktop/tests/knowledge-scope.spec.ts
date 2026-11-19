import { expect, test } from "@playwright/test";
import type { KnowledgeScope } from "../src/studio";

test.use({ headless: true });

test("knowledge scope, graph paths, and query routes reflect the requested options", async ({ page }, info) => {
  await page.addInitScript(() => {
    const w = window as unknown as {
      isTauri: boolean;
      __TAURI_INTERNALS__: unknown;
      __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
      scopes: (KnowledgeScope | null)[];
      graphChoices: boolean[];
    };
    w.isTauri = true;
    w.scopes = [];
    w.graphChoices = [];
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" } },
      transformCallback: () => 1,
      unregisterCallback: () => {},
      invoke: async (command: string, args: {
        knowledgeScope?: KnowledgeScope; name?: string; baseSnapshotId?: string;
        overrides?: { nodeId: string; version: number }[]; useGraph?: boolean; content?: string;
      }) => {
        switch (command) {
          case "bootstrap":
            return { workspaces: [{ id: "fixture", name: "历史交付资料", createdAt: "2026-10-25" }],
              settings: { model: "fixture", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 },
              activeRun: null };
          case "check_ollama": return { version: "fixture", models: [{ name: "fixture" }] };
          case "list_conversations":
          case "list_knowledge":
          case "list_knowledge_snapshots": return [];
          case "list_context_options": return { items: [], hasMore: false };
          case "capture_knowledge_snapshot":
            return { id: "delivery-v1", name: args.name, factDate: "2026-10-28", itemCount: 2 };
          case "list_knowledge_snapshot_members":
            return [{ nodeId: "delivery", title: "交付地点", version: 1 }, { nodeId: "exception", title: "例外通知", version: 1 }];
          case "get_knowledge":
            return { versions: [{ version: 2, title: "试验范围", kind: "knowledge", approvedAt: null },
              { version: 1, title: "交付地点", kind: "knowledge", approvedAt: "2026-10-28" }] };
          case "create_knowledge_branch":
            if (args.baseSnapshotId !== "delivery-v1" || JSON.stringify(args.overrides) !== JSON.stringify([{ nodeId: "delivery", version: 2 }])) {
              throw new Error("Incorrect branch selection");
            }
            return { id: "experiment-v2", name: args.name, baseSnapshotId: args.baseSnapshotId, factDate: "2026-10-28", itemCount: 2 };
          case "get_knowledge_clusters":
            if (args.knowledgeScope?.kind !== "branch") throw new Error("Incorrect cluster scope");
            return { truncated:false, groups:[{ id:"delivery",name:"试验范围",nodeCount:2,edgeCount:1,fileCount:2,
              relationCounts:{ contradicts:1 }, examples:[{ node:{nodeId:"delivery",version:2,title:"试验范围",
                source:{sourceId:"test-file",version:1,name:"试验记录.md",startLine:2,endLine:3}},
                excerpt:"这是实验分支中的交付规则。",excerptTruncated:false }] }]};
          case "preview_context": {
            const selected = args.knowledgeScope ?? null;
            w.scopes.push(selected);
            w.graphChoices.push(args.useGraph === true);
            const knowledgeScope = selected?.kind === "snapshot" || selected?.kind === "branch"
              ? { kind: selected.kind, name: selected.kind === "snapshot" ? "首次交付" : "西楼实验", factDate: "2026-10-28", eligibleCount: 2, unavailableCount: 0 }
              : selected?.kind === "factDate"
                ? { kind: "factDate", factDate: selected.date, eligibleCount: 0, unknownDateCount: 2 }
                : null;
            const conflicts = selected?.kind === "branch" ? [{
              edgeId: "registered-pair", edgeVersion: 1, recordedIn: "saved_snapshot",
              from: { nodeId: "delivery", title: "试验范围", version: 2, citationId: "资料1" },
              to: { nodeId: "exception", title: "例外通知", version: 1, citationId: "资料2" },
            }] : undefined;
            const candidates = conflicts?.flatMap((edge) => [edge.from, edge.to])
              .map((item) => ({ ...item, included: true, reason: "included" })) ?? [];
            const graph = args.useGraph && selected?.kind === "branch" ? {
              status:"matched",maxHops:2,truncated:false,
              paths:[{ steps:[{ ...conflicts![0],relation:"contradicts",reversed:false }],
                nodes:[{ ...conflicts![0].from,source:{sourceId:"test-file",version:1,name:"试验记录.md",startLine:2,endLine:3}},
                  { ...conflicts![0].to,source:{sourceId:"notice-file",version:2,name:"例外通知.md",startLine:1,endLine:2}}],
                citationId:"资料2",crossDocument:true }],
            } : undefined;
            const greeting = !selected && args.content === "你好！";
            const route = selected
              ? { plannedMethod:"version_aware", usedMethod:"version_aware", reason:"version_scope" }
              : greeting
                ? { plannedMethod:"none", usedMethod:"none", reason:"standalone_greeting" }
                : { plannedMethod:"hybrid_rerank", usedMethod:"fts5", reason:"embedding_index", fallbackReason:"preview" };
            return { knowledgeScope, messageRefs: [], estimatedUnits: 580, budgetUnits: 3328,
              retrieval: { enabled: !greeting, status: greeting ? "disabled" : conflicts ? "matched" : "no_match",
                includedCount: candidates.length, candidates, conflicts, graph, route: { policyVersion:1, ...route } } };
          }
          default:
            if (command.startsWith("plugin:")) return 1;
            throw new Error(`Unexpected mock command: ${command}`);
        }
      },
    };
  });
  await page.setViewportSize({ width: 1366, height: 768 });
  await page.goto("/#/chat");
  await page.getByRole("textbox", { name: "你的问题", exact: true }).fill("交付地点在哪里？");
  await page.getByText("手动选择：0 项知识", { exact: true }).click();
  await page.getByText("调整知识范围", { exact: true }).click();
  const mode = page.getByLabel("查找范围", { exact: true });
  await mode.selectOption("snapshot");
  await expect(page.getByRole("button", { name: "发送消息", exact: true })).toBeDisabled();
  await page.getByText("保存当前知识快照", { exact: true }).click();
  await page.getByLabel("快照名称", { exact: true }).fill("首次交付");
  await page.getByRole("button", { name: "保存快照", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "首次交付" })).toBeVisible();
  await page.getByLabel("选择快照", { exact: true }).selectOption("delivery-v1");
  await expect(page.getByText(/尚无已确认知识/)).toHaveCount(0);
  await page.getByRole("button", { name: "查看本次会用的资料", exact: true }).click();
  await expect(page.getByText("使用快照「首次交付」。", { exact: true })).toBeVisible();
  for (const [width, height] of [[1366, 768], [1600, 900], [1920, 1080]]) {
    await page.setViewportSize({ width, height });
    await page.locator(".context-picker").evaluate(node => node.scrollTop = 0);
    await page.screenshot({ path: info.outputPath(`snapshot-${width}.png`) });
  }
  await page.setViewportSize({ width: 1366, height: 768 });
  await page.getByText("从此快照创建实验分支", { exact: true }).click();
  await page.getByLabel("分支名称", { exact: true }).fill("西楼实验");
  await page.getByLabel("要替换的知识", { exact: true }).selectOption("delivery");
  await page.getByLabel("实验版本", { exact: true }).selectOption("2");
  await expect(page.getByLabel("实验版本", { exact: true }).locator("option:checked")).toContainText("待确认草稿");
  await page.getByRole("button", { name: "加入替换", exact: true }).click();
  await page.getByText("交付地点 使用 v2", { exact: true }).scrollIntoViewIfNeeded();
  await expect(page.getByRole("button", { name: "回到最新内容", exact: true })).toBeHidden();
  await page.screenshot({ path: info.outputPath("branch-form-1366.png") });
  await page.getByRole("button", { name: "保存并使用分支", exact: true }).click();
  await expect(page.getByText("在实验分支查找", { exact: true })).toBeVisible();
  await page.getByText("图谱查找", { exact: true }).click();
  const graphOption = page.getByRole("checkbox", { name: "每次都沿知识关系查找（最多两跳）", exact: true });
  await graphOption.check();
  await page.getByRole("button", { name: "查看知识簇概览", exact: true }).click();
  await page.getByText("试验范围（2 项知识，2 份文件）", { exact: true }).click();
  const summary = page.getByText("这是实验分支中的交付规则。", { exact: true });
  await summary.scrollIntoViewIfNeeded();
  await expect(summary).toBeInViewport();
  await page.screenshot({ path: info.outputPath("clusters-1366.png") });
  await page.getByRole("button", { name: "查看本次会用的资料", exact: true }).click();
  await expect(page.getByText("使用实验分支「西楼实验」。", { exact: true })).toBeVisible();
  await page.getByText("图谱查找路径（1）", { exact: true }).click();
  const path = page.getByText("1 跳，资料2，跨文件", { exact: true });
  await path.scrollIntoViewIfNeeded();
  await expect(path).toBeInViewport();
  await page.screenshot({ path: info.outputPath("graph-path-1366.png") });
  await page.getByText("已记录的反驳关系（1）", { exact: true }).click();
  const conflict = page.getByText("试验范围 v2 反驳 例外通知 v1", { exact: true });
  await conflict.scrollIntoViewIfNeeded();
  await expect(conflict).toBeInViewport();
  await page.screenshot({ path: info.outputPath("branch-conflict-1366.png") });
  await mode.selectOption("factDate");
  await expect(page.getByText("已切换到实验分支「西楼实验」。", { exact: true })).toHaveCount(0);
  await page.getByLabel("事实日期", { exact: true }).fill("2026-10-20");
  await page.getByRole("button", { name: "查看本次会用的资料", exact: true }).click();
  await expect(page.getByText(/已跳过 2 个日期未记录/)).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { scopes: unknown[] }).scopes)).toEqual([
    { kind: "snapshot", snapshotId: "delivery-v1" },
    { kind: "branch", snapshotId: "experiment-v2" },
    { kind: "factDate", date: "2026-10-20" },
  ]);
  expect(await page.evaluate(() => (window as unknown as { graphChoices: boolean[] }).graphChoices))
    .toEqual([false, true, true]);

  await mode.selectOption("current");
  await page.getByText("调整知识范围", { exact: true }).click();
  await page.getByText("图谱查找", { exact: true }).click();
  await graphOption.uncheck();
  const preview = page.getByRole("button", { name: "查看本次会用的资料", exact: true });
  await preview.click();
  const forecast = page.getByText("预览按词语查找，发送时还会尝试语义查找并重新排序。", { exact: true });
  await forecast.scrollIntoViewIfNeeded();
  await expect(forecast).toBeInViewport();
  await page.screenshot({ path: info.outputPath("route-preview-1366.png") });
  await page.getByRole("textbox", { name: "你的问题", exact: true }).fill("你好！");
  await preview.click();
  const skipped = page.getByText("这条消息是问候或致谢，已跳过自动查找。", { exact: true });
  await skipped.scrollIntoViewIfNeeded();
  await expect(skipped).toBeInViewport();
  await expect(forecast).toHaveCount(0);
  await page.screenshot({ path: info.outputPath("route-skipped-1366.png") });
});
