import { expect, test } from "@playwright/test";
import { execFile } from "node:child_process";
import { createServer } from "node:http";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { promisify } from "node:util";
import { createWorkspace, launchNative, nativeInvoke } from "./native";
import type { Conversation, Message, RunDetail } from "../src/studio";

test("model combination runs through native permissions and retains the report", async ({}, info) => {
  test.setTimeout(70_000);
  let calls = 0;
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const part of req) body += part;
    res.setHeader("Content-Type", "application/json");
    switch (req.url) {
      case "/api/version": return res.end(JSON.stringify({ version: "fixture" }));
      case "/api/tags": return res.end(JSON.stringify({ models: [{ name: "qwen3:4b-instruct", size: 1, digest: "d".repeat(64) }] }));
      case "/api/show": return res.end(JSON.stringify({ details: { format: "gguf" }, capabilities: ["completion"] }));
      case "/api/ps": return res.end(JSON.stringify({ models: [] }));
      case "/api/chat": {
        expect(body.includes("日志保留 90 天")).toBe(calls === 0);
        calls += 1;
        res.setHeader("Content-Type", "application/x-ndjson");
        return res.end(JSON.stringify({ done:true, message:{role:"assistant",content:calls === 1 ? "日志保留期限是90天[资料1:1-1]。" : "资料不足，无法作答。"},
          prompt_eval_count:20,eval_count:9 }) + "\n");
      }
      default: res.statusCode = 404; return res.end();
    }
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Missing fixture port");
  let app: Awaited<ReturnType<typeof launchNative>> | undefined;
  try {
    app = await launchNative(info, { endpoint: `http://127.0.0.1:${address.port}`, executable: process.env.CORTEXOS_RAG_TEST_EXE });
    const { page } = app;
    await createWorkspace(page, "F7 原生回环");
    const workspaceId = await page.getByLabel("当前工作区").inputValue();
    await page.evaluate(id => { location.hash = `#/benchmark?workspace=${id}&view=rag`; }, workspaceId);
    await page.getByRole("button", { name:"模型组合对照", exact:true }).click();
    await page.getByRole("group",{name:"方案 B",exact:true}).getByLabel("查找策略").selectOption("off");
    await page.getByLabel("题目", { exact:true }).selectOption("RAG-01");
    await page.getByRole("button", { name:"查看运行计划" }).click();
    await page.getByRole("button", { name:"确认并运行" }).click();
    await expect(page.getByLabel("历史对照", { exact:true })).toContainText(/已完成/);
    const catalog = await nativeInvoke<{ report: { id: string; results: { cases: { metrics: { citationPrecision: number | null }; execution: RunDetail }[] }[] } }>(
      page, "get_rag_runtime_experiment", { workspaceId },
    );
    expect(calls).toBe(2);
    expect(catalog.report.results[0].cases[0].metrics.citationPrecision).toBe(1);
    expect(catalog.report.results[1].cases[0].execution.contextManifest.knowledgeRefs).toEqual([]);
    await page.getByText("RAG-01 日志保留期限是多少天？", { exact:true }).last().click();
    await page.getByRole("button", { name:"查看 A 的 3D 执行记录" }).click();
    const trace = page.getByRole("dialog", { name:"3D 执行记录" });
    await trace.getByRole("button", { name:"知识查找完成", exact:true }).click();
    await expect(trace.getByRole("region", { name:"执行节点详情" })).toBeVisible();
    await expect(trace.locator('canvas[data-webgl-ready="true"]')).toBeVisible();
    await page.screenshot({ path:info.outputPath("rag-runtime-native-trace.png") });
    await trace.getByRole("button", { name:"关闭执行记录" }).click();
    await page.screenshot({ path:info.outputPath("rag-runtime-native-report.png") });
    const exportPath = info.outputPath("rag-runtime-native-export.json");
    await page.getByRole("button", { name:"导出完整报告", exact:true }).click();
    await promisify(execFile)("pwsh.exe", [
      "-NoProfile", "-File", resolve("tests/save-report-dialog.ps1"), "-AppProcessId", String(app.processId),
      "-OutputPath", exportPath, "-DialogTitle", "保存模型组合对照",
    ], { windowsHide:true, timeout:25_000 });
    await expect(page.getByText("报告已保存", { exact:true })).toBeVisible();
    const exported = JSON.parse(await readFile(exportPath, "utf8"));
    expect(exported).toEqual(catalog.report);
    expect(exported).toMatchObject({
      planHash: expect.stringMatching(/^[a-f0-9]{64}$/),
      plan: { scorer:"cortex-rag-response-rubric-2", caseIds: ["RAG-01"], arms: [{ strategy:"fts5" }, { strategy:"off" }] },
      dataset: { version:1, cases:expect.any(Array), documents:expect.any(Array) },
      results: ["fts5", "off"].map(() => ({
        usage: { localInputTokens:20, localOutputTokens:9, cloudCalls:0 },
        cases:[{ execution:{ events:expect.any(Array), snapshot:{ requestHash:expect.stringMatching(/^[a-f0-9]{64}$/) } } }],
      })),
      cloudCalls: [], localEmbeddings: [],
    });
    const other = await nativeInvoke<{ id: string }>(page, "create_workspace", { name: "F7 隔离" });
    await expect(nativeInvoke(page, "get_rag_runtime_experiment", { workspaceId:other.id,id:catalog.report.id })).rejects.toBeTruthy();
    await expect(nativeInvoke(page, "export_rag_runtime_experiment", { workspaceId:other.id,id:catalog.report.id })).rejects.toBeTruthy();
    await page.getByRole("button", { name:"任务中心", exact:true }).click();
    const record = page.getByRole("dialog", { name:"任务中心", exact:true })
      .getByRole("listitem").filter({ hasText:"模型组合对照" });
    await expect(record).toContainText("已完成");
    await record.getByRole("link", { name:"查看记录", exact:true }).click();
    await expect(page).toHaveURL(new RegExp(`experiment=${catalog.report.id}`));
    await expect(page.getByLabel("历史对照", { exact:true })).toHaveValue(catalog.report.id);
  } finally {
    await app?.stop();
    await new Promise<void>(resolve => server.close(() => resolve()));
  }
});

test("workspace hybrid retrieval uses the configured index and recovers from invalid vectors", async ({}, info) => {
  test.setTimeout(90_000);
  const model = "fixture-embeddings:latest";
  const digest = "d".repeat(64);
  const replacement = "e".repeat(64);
  let installedDigest = digest;
  const requests: { input: string[]; options: { num_gpu: number } }[] = [];
  const chats: unknown[] = [];
  const fileContent = "发布前生成一致性快照。";
  const dataDirectory = info.outputPath("data");
  const sourcePath = info.outputPath("data", "backup.md");
  await mkdir(dataDirectory, { recursive: true });
  await writeFile(sourcePath, fileContent, "utf8");
  let invalid = false;
  let releaseBlocker!: () => void;
  let blockerStarted = false;
  const blocker = new Promise<void>(done => { releaseBlocker = done; });
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const part of req) body += part;
    const input = body ? JSON.parse(body) : {};
    const reply = (value: unknown, contentType = "application/json") => {
      res.setHeader("Content-Type", contentType);
      res.end(JSON.stringify(value) + "\n");
    };
    switch (req.url) {
      case "/api/version": return reply({ version: "rag-fixture" });
      case "/api/tags": return reply({ models: [{ name: "qwen3:4b-instruct", size: 1 }, { name: model, size: 1, digest: installedDigest }] });
      case "/api/ps": return reply({ models: [] });
      case "/api/show": return reply({ details: { format: "gguf" }, capabilities: [input.model === model ? "embedding" : "completion"] });
      case "/api/embed":
        if (input.input.includes("SQLite 备份\n其它工作区内容不得使用。")) {
          blockerStarted = true;
          await blocker;
          res.statusCode = 503;
          return reply({ error: "fixture finished" });
        }
        requests.push(input);
        return reply({ embeddings: input.input.map(() => invalid ? [1, 0, 1] : [1, 0]) });
      case "/api/chat":
        chats.push(input);
        return reply({ message: { role: "assistant", content: "已处理测试问题 [资料1:1-1]。" }, done: true,
          done_reason: "stop", prompt_eval_count: 640, eval_count: 18 }, "application/x-ndjson");
      default: res.statusCode = 404; res.end();
    }
  });
  await new Promise<void>(done => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Missing fixture port");
  let app: Awaited<ReturnType<typeof launchNative>> | undefined;
  try {
    app = await launchNative(info, {
      endpoint: `http://127.0.0.1:${address.port}`,
      executable: process.env.CORTEXOS_RAG_TEST_EXE,
      dataDirectory, sourcePath,
    });
    const page = app.page;
    await createWorkspace(page, "混合检索");
    const workspaceId = await page.getByLabel("当前工作区").inputValue();
    const other = await nativeInvoke<{ id: string }>(page, "create_workspace", { name: "其它工作区" });
    const save = (scope: string, title: string, content: string) => nativeInvoke<string>(page, "save_knowledge", {
      input: { workspaceId: scope, title, content, kind: "knowledge", changeReason: "混合检索专项" },
    });
    const approve = (scope: string, nodeId: string) => nativeInvoke(page, "approve_knowledge", { workspaceId: scope, nodeId, expectedRevision: 1 });
    const file = await nativeInvoke<{ ticket: string }>(page, "choose_source", { workspaceId });
    const source = await nativeInvoke<{ id: string }>(page, "commit_source", { workspaceId, ticket: file.ticket });
    const nodeId = await nativeInvoke<string>(page, "save_source_knowledge", {
      input: { workspaceId, title: "SQLite 备份", content: fileContent, kind: "knowledge", changeReason: "混合检索专项" },
      source: { sourceId: source.id, version: 1, startLine: 1, endLine: 1 },
    });
    await approve(workspaceId, nodeId);
    await save(workspaceId, "待审核资料", "草稿不得构建索引。");
    await approve(other.id, await save(other.id, "SQLite 备份", "其它工作区内容不得使用。"));
    const status = (scope = workspaceId) => nativeInvoke(page, "rag_index_status", { workspaceId: scope });
    await expect(status()).resolves.toMatchObject({ eligible: 1, indexed: 0, index: null });
    expect(requests).toHaveLength(0);
    await page.getByRole("link", { name: "设置", exact: true }).click();
    await page.getByText("知识检索", { exact: true }).click();
    await page.getByRole("button", { name: "读取本地向量模型" }).click();
    const occupied = nativeInvoke(page, "rag_build_index", {
      workspaceId: other.id, input: { model, digest, confirmed: true },
    }).then(() => false, () => true);
    try {
      await expect.poll(() => blockerStarted).toBe(true);
      await page.getByRole("button", { name: "构建索引…" }).click();
      await page.getByRole("button", { name: "确认并构建" }).click();
      await expect(page.getByRole("status").filter({ hasText: "正在等待" })).toBeVisible();
      await page.screenshot({ path: info.outputPath("index-waiting.png") });
      await page.getByRole("button", { name: "停止构建" }).click();
      await expect(page.getByText("已取消等待。", { exact: true })).toBeVisible();
      expect(requests).toHaveLength(0);
      await nativeInvoke(page, "rag_cancel_index", { workspaceId: other.id });
      await expect(occupied).resolves.toBe(true);
      await expect(status(other.id)).resolves.toMatchObject({ indexed: 0, index: null });
    } finally {
      releaseBlocker();
      await occupied;
    }
    await page.getByRole("button", { name: "构建索引…" }).click();
    await expect(page.getByRole("dialog", { name: "确认构建向量索引" })).toBeVisible();
    expect(requests).toHaveLength(0);
    await page.getByRole("button", { name: "确认并构建" }).click();
    await expect(page.getByText("索引构建完成", { exact: true })).toBeVisible();
    expect(requests).toMatchObject([{ input: ["SQLite 备份\n发布前生成一致性快照。"], options: { num_gpu: 0 } }]);
    await expect(status()).resolves.toMatchObject({ indexed: 1, index: { digest, dimensions: 2, enabled: false } });
    await expect(status(other.id)).resolves.toMatchObject({ eligible: 1, indexed: 0, index: null });
    await page.getByRole("checkbox", { name: "按语义与词语查找" }).check();
    await page.getByRole("checkbox", { name: "按关键词覆盖重新排序" }).check();
    await expect.poll(status).toMatchObject({ index: { enabled: true, rerank: true } });
    await expect(nativeInvoke(page, "rag_build_index", { workspaceId, input: { model, digest, confirmed: true } }))
      .resolves.toMatchObject({ indexed: 1, index: { digest, dimensions: 2, enabled: true, rerank: true } });
    expect(requests).toHaveLength(1);
    await page.screenshot({ path: info.outputPath("rag-settings.png") });

    await page.getByRole("link", { name: "对话", exact: true }).click();
    let conversationId: string;
    const send = async (content: string, count: number) => {
      await page.getByRole("textbox", { name: "你的问题", exact: true }).fill(content);
      await page.getByRole("button", { name: "发送消息", exact: true }).click();
      await expect.poll(() => chats.length).toBe(count);
      await expect(page.getByText("已完成", { exact: true })).toHaveCount(count);
      conversationId = (await nativeInvoke<Conversation[]>(page, "list_conversations", { workspaceId }))[0].id;
      const messages = await nativeInvoke<Message[]>(page, "load_messages", { workspaceId, conversationId });
      return nativeInvoke<RunDetail>(page, "get_run", { workspaceId, runId: messages.at(-1)!.modelRunId });
    };
    const first = await send("SQLite 如何备份？", 1);
    expect(first.contextManifest.retrieval).toMatchObject({
      algorithm: "bm25-ollama-rrf-v2", semanticStatus: "completed", reranker: "local_term_coverage_v1",
      candidates: [{ nodeId, version: 1, lexicalRank: 1, semanticRank: 1, fusionRank: 1, included: true }],
    });
    expect(first.snapshot?.request).toEqual(chats[0]);
    expect(JSON.stringify(chats[0])).not.toMatch(/草稿不得构建|其它工作区内容/);
    expect(first.events.filter(event => event.kind.startsWith("retrieval_")).map(event => event.kind)).toEqual(["retrieval_started", "retrieval_completed"]);
    expect(first.events.map(event => event.occurredAt)).toEqual(first.events.map(event => event.occurredAt).sort());
    expect(first.contextManifest.retrieval?.stages?.map(stage => stage.kind)).toEqual([
      "prepare_vector", "lexical_lookup", "semantic_lookup", "merge_rankings", "rerank", "conflict_lookup", "select_context",
    ]);
    const evidence = page.getByLabel("回答依据", { exact: true });
    await evidence.getByText("查找过程", { exact: true }).click();
    await evidence.getByText("查找步骤与用时", { exact: true }).click();
    await expect(evidence.getByText("已按关键词覆盖重新排序。", { exact: true })).toBeVisible();
    await expect(evidence.getByText("重新排序", { exact: true })).toBeVisible();
    await evidence.getByText("查看哪些知识被选用及原因", { exact: true }).click();
    await evidence.getByText("SQLite 备份，v1", { exact: true }).click();
    await expect(evidence.getByText(/词语第 1，语义第 1，合并后第 1，重排得分/)).toBeVisible();
    await expect(evidence.getByText("backup.md，文件 v1，来源摘录第 1–1 行", { exact: true })).toBeVisible();
    await evidence.getByRole("button", { name: "查看此处原文", exact: true }).click();
    await expect(page.getByRole("region", { name: "文件内容" })).toContainText(fileContent);
    await page.getByRole("button", { name: "关闭资料文件", exact: true }).click();
    await evidence.getByText("发给模型的资料（1）", { exact: true }).click();
    await evidence.getByText("SQLite 备份，本次使用 v1", { exact: true }).click();
    await expect(evidence.getByText("整次输入 640 Token（模型服务报告）", { exact: true })).toBeVisible();
    const located = page.getByRole("button", { name: "[资料1:1-1]，查看引用文字", exact: true });
    for (const theme of ["light", "dark"] as const) {
      await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
      for (const [width, height] of [[1366, 768], [1600, 900], [1920, 1080]]) {
        await page.setViewportSize({ width, height });
        await evidence.getByText("查找步骤与用时", { exact: true }).scrollIntoViewIfNeeded();
        expect(await evidence.locator(".evidence-content").evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
        await page.screenshot({ path: info.outputPath(`rag-inspector-${theme}-${width}.png`) });
      }
    }
    await evidence.getByRole("button", { name: "定位回答中的 [资料1:1-1]", exact: true }).click();
    await expect(located).toBeFocused();
    await page.getByRole("button", { name: "3D 执行轨迹", exact: true }).click();
    await page.getByRole("button", { name: /知识查找完成/ }).click();
    const trace = page.getByLabel("执行节点详情");
    await trace.getByText("查找步骤与用时", { exact: true }).click();
    await expect(trace.getByText("按语义查找", { exact: true })).toBeVisible();
    await expect(page.locator('canvas[data-webgl-ready="true"]').or(page.getByRole("heading", { name: "3D 不可用", exact: true }))).toBeVisible({ timeout: 10_000 });
    await page.screenshot({ path: info.outputPath("rag-inspector-trace.png") });
    await trace.getByText("发给模型的资料（1）", { exact: true }).click();
    await trace.getByText("SQLite 备份，本次使用 v1", { exact: true }).click();
    await trace.getByRole("button", { name: "定位回答中的 [资料1:1-1]", exact: true }).click();
    await expect(located).toBeFocused();
    const preview = await nativeInvoke<{ retrieval: { algorithm: string } }>(page, "preview_context", {
      workspaceId, conversationId: conversationId!, content: "SQLite 备份", contextSelection: [],
    });
    expect(preview.retrieval.algorithm).toBe("fts5-cjk-bigram-latin-bm25-v2");
    expect(requests).toHaveLength(2);

    invalid = true;
    const oldIndex = await status();
    const fallback = await send("再查 SQLite 备份", 2);
    expect(fallback.contextManifest.retrieval).toMatchObject({
      algorithm: "fts5-cjk-bigram-latin-bm25-v2", semanticStatus: "unavailable", candidates: [{ nodeId, included: true }],
    });
    expect(fallback.contextManifest.retrieval?.stages?.map(stage => stage.kind)).toEqual([
      "prepare_vector", "lexical_lookup", "conflict_lookup", "select_context",
    ]);
    const before = requests.length;
    await page.getByRole("checkbox", { name: "自动查找知识", exact: true }).uncheck();
    const disabled = await send("SQLite 备份", 3);
    expect(disabled.contextManifest.retrievalUsed).toBe(false);
    expect(requests).toHaveLength(before);
    await expect(nativeInvoke(page, "rag_build_index", { workspaceId, input: { model, digest: replacement, confirmed: true } })).rejects.toBeTruthy();
    expect(await status()).toEqual(oldIndex);
    installedDigest = replacement;
    await expect(nativeInvoke(page, "rag_build_index", { workspaceId, input: { model, digest: replacement, confirmed: true } }))
      .resolves.toMatchObject({ indexed: 1, index: { digest: replacement, dimensions: 3, enabled: true } });
    await page.getByRole("checkbox", { name: "自动查找知识", exact: true }).check();
    const semanticOnly = await send("资料怎样留副本？", 4);
    expect(semanticOnly.contextManifest.retrieval).toMatchObject({
      semanticStatus: "completed", embedding: { digest: replacement, dimensions: 3 },
      candidates: [{ nodeId, version: 1, semanticRank: 1, included: true }],
    });
    expect(semanticOnly.contextManifest.retrieval?.candidates[0].lexicalRank).toBeUndefined();
    const unchanged = requests.length;
    const addedNode = await save(workspaceId, "另一个项目规则", "这条规则是本轮唯一新增的向量输入。");
    await approve(workspaceId, addedNode);
    const rebuild = () => nativeInvoke(page, "rag_build_index", { workspaceId, input: { model, digest: replacement, confirmed: true } });
    await expect(rebuild()).resolves.toMatchObject({ indexed: 2, index: { digest: replacement } });
    expect(requests.slice(unchanged).map(request => request.input)).toEqual([["另一个项目规则\n这条规则是本轮唯一新增的向量输入。"]]);
    await nativeInvoke(page, "revoke_source", { workspaceId, sourceId: source.id, expectedRevision: 1 });
    await expect(rebuild()).resolves.toMatchObject({ indexed: 1, eligible: 1 });
    expect(requests).toHaveLength(unchanged + 1);
    if (process.env.CORTEXOS_H3_RAG_TIMINGS === "1") {
      const observed = [];
      for (let i = 0; i < 8; i++) {
        const before = performance.now();
        const run = await send("另一个项目规则是什么？", chats.length + 1);
        const elapsedMs = performance.now() - before;
        expect(run.contextManifest.retrieval?.semanticStatus).toBe("completed");
        observed.push(elapsedMs);
      }
      const ordered = [...observed].sort((a, b) => a - b);
      await writeFile(info.outputPath("hybrid-end-to-end.json"), JSON.stringify({
        source: "loopback embedding and reply, visible completion and run detail",
        n: ordered.length, p50: (ordered[3] + ordered[4]) / 2, p95: ordered[7], elapsedMs: observed,
      }, null, 2));
    }
    expect(app.consoleErrors).toEqual([]);
  } finally {
    await app?.stop();
    server.closeAllConnections();
    await new Promise<void>(done => server.close(() => done()));
  }
});
