import { expect, test, type Page, type TestInfo } from "@playwright/test";
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { createWorkspace, launchNative, nativeInvoke } from "./native";
import type { Bootstrap } from "../src/studio";

type ModelRequest = {
  model: string;
  expectedDigest: string;
  parameters: {
    numCtx: number;
    numPredict: number;
    temperature: number;
    timeoutSeconds: number;
  };
  requirements: string[];
};
type Summary = {
  id: string;
  workspaceId: string;
  status: string;
  caseRunId: string;
  attemptId: string;
  executionRunId: string;
  developmentOnly: boolean;
};
type Detail = {
  summary: Summary;
  config: Record<string, unknown>;
  case: { case_id: string };
  model: unknown;
  execution: {
    run: {
      status: string;
      conversationId: string;
      assistantMessageId: string;
      errorMessage: string | null;
    };
    snapshot: { request: Record<string, unknown>; requestHash: string };
    contextManifest: Record<string, unknown>;
    events: { kind: string; payload: unknown }[];
  };
  output: string;
  attempt: { status: string; requestDisposition: string };
  evaluations: {
    metrics: { correctness: number | null };
    outputHash: string;
    status: string;
  }[];
  events: { kind: string }[];
};
type Input = {
  launchKey: string;
  developmentOnly: boolean;
  datasetHash: string;
  caseId: string;
  model: ModelRequest;
};
const sha = (s: string) => createHash("sha256").update(s).digest("hex");
const params = {
  numCtx: 4096,
  numPredict: 64,
  temperature: 0,
  timeoutSeconds: 180,
};
async function inputFor(
  page: Page,
  workspaceId: string,
  model: ModelRequest,
  launchKey: string,
): Promise<Input> {
  const datasets = await nativeInvoke<{ contentHash: string }[]>(
    page,
    "list_benchmark_datasets",
    { workspaceId },
  );
  return {
    launchKey,
    developmentOnly: true,
    datasetHash: datasets[0].contentHash,
    caseId: "math-001",
    model,
  };
}
async function record(info: TestInfo, name: string, value: unknown) {
  await mkdir(info.outputDir, { recursive: true });
  const file = info.outputPath(name);
  await writeFile(file, JSON.stringify(value, null, 2));
  await info.attach(name, { path: file, contentType: "application/json" });
}

type V2Case = {
  case_id: string;
  dataset_version: number;
  requirements: string[];
  scoring: { id: string };
  input: { response_schema: unknown; tools: unknown[] };
  expected: { kind: string; answers?: string[]; answer?: unknown; name?: string; arguments?: Record<string, unknown> };
};
type V2Detail = Detail & {
  case: V2Case;
  evaluations: (Detail["evaluations"][number] & { details: { state: string }; metrics: Record<string, unknown> })[];
  statistics: { planned: number; scored: number; attempted: number; correctness: number | null; datasetScoringCoverage: number; states: Record<string, number> };
};

test("native v2 scoring preserves real response objects, separate outcomes and dataset versions", async ({}, info) => {
  test.setTimeout(120_000);
  const digest = "a".repeat(64);
  let selected: V2Case;
  let mode = "gold";
  let toolsSupported = true;
  let v2ShowCount = 0;
  const requests: { raw: string; body: Record<string, unknown> }[] = [];
  const records: V2Detail[] = [];
  const server = createServer(async (req, res) => {
    const send = (value: unknown) => { res.setHeader("Content-Type", "application/json"); res.end(JSON.stringify(value)); };
    const details = { format: "gguf", family: "fixture", parameter_size: "synthetic", quantization_level: "synthetic" };
    if (req.url === "/api/tags") return send({ models: [{ name: "fixture-v2", digest, size: 1, details }] });
    if (req.url === "/api/version") return send({ version: "synthetic-v2-provider" });
    if (req.url === "/api/ps") return send({ models: [] });
    let raw = "";
    for await (const chunk of req) raw += chunk.toString();
    if (req.url === "/api/show") { v2ShowCount++; return send({ details, capabilities: toolsSupported && !(mode === "cap-change" && v2ShowCount >= 2) ? ["completion", "tools"] : ["completion"], model_info: { "general.architecture": "fixture", "fixture.context_length": 8192 } }); }
    if (req.url !== "/api/chat") { res.statusCode = 404; return res.end("{}"); }
    requests.push({ raw, body: JSON.parse(raw) });
    res.setHeader("Content-Type", "application/x-ndjson");
    let content = "";
    let toolCalls: unknown[] | undefined;
    switch (selected.expected.kind) {
      case "aliases": content = selected.expected.answers![0]; break;
      case "integer": content = String(selected.expected.answer); break;
      case "json": {
        const value = structuredClone(selected.expected.answer) as Record<string, unknown>;
        if (mode === "bad-citation") (value.citations as Record<string, unknown>[])[0] = { claim_id: "floor", source_id: "map", start: 1, end: 1 };
        if (mode === "bad-type") value.age = "28";
        content = JSON.stringify(value); break;
      }
      case "tool": {
        const call = { function: { name: selected.expected.name, arguments: selected.expected.arguments } };
        if (mode === "text-tool") content = JSON.stringify(call);
        else toolCalls = [call];
        break;
      }
      default: content = "No Python model call should occur without a selected image";
    }
    if (mode === "bad-stream") { res.end(JSON.stringify({ message: { role: "assistant", content: "95" }, done: false }) + "\n{invalid}\n"); return; }
    if (mode === "fragments") {
      for (const argumentsValue of [{ query: "发布清单" }, { limit: 3 }]) {
        res.write(JSON.stringify({ message: { role: "assistant", content: "", tool_calls: [{ function: { index: 0, name: "search_notes", arguments: argumentsValue } }] }, done: false }) + "\n");
      }
      toolCalls = undefined;
    }
    res.end(JSON.stringify({ message: { role: "assistant", content, ...(toolCalls ? { tool_calls: toolCalls } : {}) }, done: true, done_reason: mode === "truncated" ? "length" : "stop", eval_count: 11, prompt_eval_count: 17, eval_duration: 10_000_000 }) + "\n");
  });
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  if (!address || typeof address === "string") throw Error("Missing fixture port");
  let app: Awaited<ReturnType<typeof launchNative>> | undefined;
  try {
    app = await launchNative(info, { endpoint: `http://127.0.0.1:${address.port}` });
    const { page } = app;
    const workspace = await nativeInvoke<{ id: string }>(page, "create_workspace", { name: "v2受控评分验证" });
    const before = await nativeInvoke<Bootstrap>(page, "bootstrap");
    const datasets = await nativeInvoke<{ version: number; contentHash: string }[]>(page, "list_benchmark_datasets", { workspaceId: workspace.id });
    const v2 = datasets.find((d) => d.version === 2)!;
    let serial = 0;
    const run = async (caseId: string, state: string, scenario = "gold") => {
      mode = scenario;
      v2ShowCount = 0;
      selected = await nativeInvoke<V2Case>(page, "get_benchmark_case", { workspaceId: workspace.id, datasetId: "cortex-core-60", version: 2, caseId });
      const input = { datasetVersion: 2, developmentOnly: true, datasetHash: v2.contentHash, caseId, launchKey: `controlled-v2-${++serial}`, model: { model: "fixture-v2", expectedDigest: digest, parameters: { ...params, numPredict: 512 }, requirements: selected.requirements.filter((r) => r !== "python_sandbox") } };
      const started = await nativeInvoke<Summary>(page, "start_benchmark", { workspaceId: workspace.id, input });
      const get = () => nativeInvoke<V2Detail>(page, "get_benchmark_run", { workspaceId: workspace.id, experimentId: started.id });
      await expect.poll(async () => (await get()).evaluations.length, { timeout: 25_000 }).toBe(1);
      await expect.poll(async () => (await nativeInvoke<Bootstrap>(page, "bootstrap")).activeTask ?? null).toBeNull();
      const d = await get();
      expect(d.evaluations[0].details.state).toBe(state);
      expect(d.case.dataset_version).toBe(2);
      expect(d.statistics.planned).toBe(1);
      if (["Passed", "Failed"].includes(state)) {
        expect(d.statistics.scored).toBe(1);
        expect(d.statistics.datasetScoringCoverage).toBeCloseTo(1 / 60);
      } else {
        expect(d.statistics.scored).toBe(0);
        expect(d.statistics.correctness).toBeNull();
      }
      const requestCount = requests.length;
      await nativeInvoke<Summary>(page, "start_benchmark", { workspaceId: workspace.id, input });
      expect(requests).toHaveLength(requestCount);
      expect((await get()).evaluations).toHaveLength(1);
      records.push(d); return d;
    };
    for (const caseId of ["zh-014", "math-006", "schema-001", "rag-008", "tool-002"]) {
      const d = await run(caseId, "Passed");
      const wire = requests.at(-1)!;
      expect(d.execution.snapshot.requestHash).toBe(sha(wire.raw));
      expect(wire.body).not.toHaveProperty("expected");
      expect(wire.body).not.toHaveProperty("scoring");
      if (caseId.startsWith("schema")) expect(wire.body.format).toEqual(selected!.input.response_schema);
      if (caseId.startsWith("rag")) { expect(wire.body).not.toHaveProperty("format"); expect(JSON.stringify(wire.body.messages)).toContain("服务台位于二层"); }
      if (caseId.startsWith("tool")) {
        expect(wire.body.tools).toEqual(selected!.input.tools);
        expect(d.output).toBe("");
        expect(d.execution.events.some((e) => e.kind === "native_tool_calls_received")).toBe(true);
      }
    }
    const wrongEvidence = await run("rag-008", "Failed", "bad-citation");
    expect(wrongEvidence.evaluations[0].metrics.answerCorrect).toBe(true);
    expect(wrongEvidence.evaluations[0].metrics.citationPrecision).toBe(0);
    await run("schema-001", "Failed", "bad-type");
    const fake = await run("tool-002", "Failed", "text-tool");
    expect(fake.execution.events.some((e) => e.kind === "native_tool_calls_received")).toBe(false);
    await run("tool-002", "Passed", "fragments");
    toolsSupported = false;
    const count = requests.length;
    const unsupported = await run("tool-002", "Unsupported");
    expect(unsupported.statistics.attempted).toBe(0);
    expect(requests).toHaveLength(count);
    toolsSupported = true;
    await run("tool-002", "Unsupported", "cap-change");
    expect(requests).toHaveLength(count);
    await run("code-001", "Execution Error");
    expect(requests).toHaveLength(count);
    const malformed = await run("math-001", "Execution Error", "bad-stream");
    expect(malformed.output).toBe("95");
    await run("math-001", "Needs Review", "truncated");
    expect(await nativeInvoke(page, "bootstrap")).toEqual(before);
    expect(app.consoleErrors).toEqual([]);
    await record(info, "v2-scoring-controlled-evidence.json", { environment: "controlled_provider_not_model_quality", datasets, records, requests });
  } finally {
    await app?.stop();
    server.closeAllConnections();
    await new Promise<void>((done) => server.close(() => done()));
  }
});

test("native development benchmark freezes wire requests, isolates data and shares cancellation with chat", async ({}, info) => {
  test.setTimeout(100_000);
  const a = "a".repeat(64),
    b = "b".repeat(64);
  let mode = "complete",
    digest = a,
    showCount = 0;
  const wire: Record<string, unknown>[] = [];
  const wireBodies: string[] = [];
  const server = createServer(async (req, res) => {
    const json = (v: unknown) => {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify(v));
    };
    const details = {
      format: "gguf",
      family: "fixture",
      parameter_size: "fixture",
      quantization_level: "fixture",
    };
    if (req.url === "/api/version")
      return json({ version: "slice3-controlled" });
    if (req.url === "/api/tags")
      return json({
        models: [
          { name: "benchmark-fixture", digest, size: 100, details },
          { name: "qwen3:4b-instruct", digest: a, size: 100, details },
        ],
      });
    if (req.url === "/api/ps") return json({ models: [] });
    let raw = "";
    for await (const chunk of req) raw += chunk.toString();
    const body = JSON.parse(raw || "{}");
    if (req.url === "/api/show") {
      if (body.model === "benchmark-fixture") showCount++;
      if (mode === "swap" && showCount === 2) digest = b;
      if (mode === "preflight_delay")
        await new Promise((done) => setTimeout(done, 1200));
      return json({
        details,
        capabilities: ["completion", "thinking"],
        model_info: {
          "general.architecture": "fixture",
          "fixture.context_length": 8192,
        },
      });
    }
    if (req.url !== "/api/chat") {
      res.statusCode = 404;
      res.end("{}");
      return;
    }
    wire.push(body);
    wireBodies.push(raw);
    res.writeHead(200, { "Content-Type": "application/x-ndjson" });
    const frame = (content: string, done = false) =>
      JSON.stringify({
        message: { role: "assistant", content },
        done,
        ...(done
          ? { done_reason: "stop", eval_count: 1, eval_duration: 1_000_000 }
          : {}),
      }) + "\n";
    if (mode === "slow" || mode === "chat_slow") {
      res.write(frame(mode === "chat_slow" ? "聊天部分" : "95"));
      const timer = setTimeout(() => res.end(frame("迟到内容", true)), 1800);
      timer.unref();
      return;
    }
    if (mode === "malformed") {
      res.end(frame("95") + "{bad}\n");
      return;
    }
    const bytes = Buffer.from(frame(" 95\n", true));
    res.write(bytes.subarray(0, bytes.length - 5));
    setTimeout(() => res.end(bytes.subarray(bytes.length - 5)), 40);
  });
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  if (!address || typeof address === "string")
    throw Error("Missing fixture port");
  const app = await launchNative(info, {
    endpoint: `http://127.0.0.1:${address.port}`,
  });
  try {
    const { page } = app;
    await createWorkspace(page, "开发单题闭环");
    const bootstrap = await nativeInvoke<Bootstrap>(page, "bootstrap");
    const workspaceId = bootstrap.workspaces.find(
      (w) => w.name === "开发单题闭环",
    )!.id;
    const other = await nativeInvoke<{ id: string }>(page, "create_workspace", {
      name: "隔离工作区",
    });
    const knowledgeId = await nativeInvoke<string>(page, "save_knowledge", {
      input: {
        workspaceId,
        nodeId: null,
        expectedRevision: null,
        title: "长期知识不应发送",
        kind: "knowledge",
        content: "计算37+58。PRODUCTION_KNOWLEDGE_SENTINEL错误答案是99999。",
        changeReason: "controlled isolation fixture",
        sourceMessageId: null,
        sourceExcerpt: null,
      },
    });
    await nativeInvoke(page, "approve_knowledge", {
      workspaceId,
      nodeId: knowledgeId,
      expectedRevision: 1,
    });
    const knowledgeBefore = await nativeInvoke(page, "get_knowledge", {
      workspaceId,
      nodeId: knowledgeId,
    });
    const preview = await nativeInvoke<{ knowledgeRefs: { nodeId: string }[] }>(
      page,
      "preview_context",
      {
        workspaceId,
        conversationId: null,
        content: "计算37+58。只返回最终整数，不加单位或解释。",
        contextSelection: [],
      },
    );
    expect(preview.knowledgeRefs.some((k) => k.nodeId === knowledgeId)).toBe(
      true,
    );
    await page
      .getByRole("textbox", { name: "你的问题", exact: true })
      .fill("CHAT_HISTORY_SENTINEL:计算37+58。");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(
      page
        .getByRole("article", { name: "模型回答" })
        .last()
        .getByText("已完成", { exact: true }),
    ).toBeVisible();
    await expect
      .poll(
        async () =>
          (await nativeInvoke<Bootstrap>(page, "bootstrap")).activeTask,
      )
      .toBeNull();
    const baseline = wire.length;
    expect(baseline).toBe(1);
    const runsBefore = await nativeInvoke(page, "list_runs", { workspaceId });
    const conversationsBefore = await nativeInvoke(page, "list_conversations", {
      workspaceId,
    });
    const settingsBefore = (await nativeInvoke<Bootstrap>(page, "bootstrap"))
      .settings;
    const base = await inputFor(
      page,
      workspaceId,
      {
        model: "benchmark-fixture",
        expectedDigest: a,
        parameters: params,
        requirements: ["completion"],
      },
      "development-success",
    );
    const start = (input: Input) =>
      nativeInvoke<Summary>(page, "start_benchmark", { workspaceId, input });
    const detail = (id: string, scope = workspaceId) =>
      nativeInvoke<Detail>(page, "get_benchmark_run", {
        workspaceId: scope,
        experimentId: id,
      });
    const first = await start(base);
    await expect
      .poll(async () => (await detail(first.id)).summary.status)
      .toBe("completed");
    const completed = await detail(first.id);
    expect(completed.output).toBe(" 95\n");
    expect(completed.evaluations).toHaveLength(1);
    expect(completed.evaluations[0].metrics.correctness).toBe(1);
    expect(completed.evaluations[0].outputHash).toBe(sha(completed.output));
    expect(wire).toHaveLength(baseline + 1);
    expect(completed.execution.snapshot.request).toEqual(wire[baseline]);
    expect(completed.execution.snapshot.requestHash).toBe(
      sha(wireBodies[baseline]),
    );
    expect(wire[baseline].messages as unknown[]).toHaveLength(2);
    expect(JSON.stringify(wire[baseline])).not.toContain(
      "PRODUCTION_KNOWLEDGE_SENTINEL",
    );
    expect(JSON.stringify(wire[baseline])).not.toContain(
      "CHAT_HISTORY_SENTINEL",
    );
    expect(wire[baseline].options).toEqual({
      num_ctx: 4096,
      num_predict: 64,
      temperature: 0,
    });
    expect(completed.execution.contextManifest).toMatchObject({
      purpose: "benchmark",
      historyUsed: false,
      retrievalUsed: false,
      knowledgeRefs: [],
      reviewStatus: "needs_human_review",
    });
    expect(completed.execution.events.map((e) => e.kind)).toEqual([
      "request_received",
      "context_prepared",
      "model_inspection_started",
      "model_inspection_completed",
      "model_request_started",
      "model_response_received",
      "response_completed",
    ]);
    expect(
      await nativeInvoke(page, "list_conversations", { workspaceId }),
    ).toEqual(conversationsBefore);
    expect(await nativeInvoke(page, "list_runs", { workspaceId })).toEqual(
      runsBefore,
    );
    await expect(
      nativeInvoke(page, "get_run", {
        workspaceId,
        runId: first.executionRunId,
      }),
    ).rejects.toBeTruthy();
    await expect(
      nativeInvoke(page, "load_messages", {
        workspaceId,
        conversationId: completed.execution.run.conversationId,
      }),
    ).rejects.toBeTruthy();
    await expect(detail(first.id, other.id)).rejects.toBeTruthy();
    await expect(
      nativeInvoke(page, "save_knowledge", {
        input: {
          workspaceId,
          nodeId: null,
          expectedRevision: null,
          title: "不许自动保存",
          kind: "knowledge",
          content: "95",
          changeReason: "must reject",
          sourceMessageId: completed.execution.run.assistantMessageId,
          sourceExcerpt: "95",
        },
      }),
    ).rejects.toBeTruthy();
    expect((await start(base)).id).toBe(first.id);
    await expect(
      start({
        ...base,
        model: { ...base.model, parameters: { ...params, temperature: 0.8 } },
      }),
    ).rejects.toBeTruthy();
    expect(wire).toHaveLength(baseline + 1);

    mode = "slow";
    const slowInput = { ...base, launchKey: "development-cancel" };
    const slow = await start(slowInput);
    await expect.poll(async () => (await detail(slow.id)).output).toBe("95");
    const active = await nativeInvoke<Bootstrap>(page, "bootstrap");
    expect(active.activeRun).toBeNull();
    expect(active.activeTask).toMatchObject({
      id: slow.id,
      kind: "benchmark",
      executionRunId: slow.executionRunId,
    });
    expect((await start(slowInput)).id).toBe(slow.id);
    await expect(
      start({ ...base, launchKey: "duplicate-while-busy" }),
    ).rejects.toBeTruthy();
    await page
      .getByRole("textbox", { name: "你的问题", exact: true })
      .fill("CHAT_SENTINEL");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(
      page.getByText("已有聊天或评测正在进行，请先停止或等待完成。", {
        exact: true,
      }),
    ).toBeVisible();
    expect(wire).toHaveLength(baseline + 2);
    await expect(
      nativeInvoke(page, "cancel_benchmark", {
        workspaceId: other.id,
        experimentId: slow.id,
      }),
    ).rejects.toBeTruthy();
    await expect(
      nativeInvoke(page, "cancel_run", {
        workspaceId,
        runId: slow.executionRunId,
      }),
    ).rejects.toBeTruthy();
    await nativeInvoke(page, "cancel_benchmark", {
      workspaceId,
      experimentId: slow.id,
    });
    await expect
      .poll(async () => (await detail(slow.id)).summary.status)
      .toBe("cancelled");
    const cancelled = await detail(slow.id);
    expect(cancelled.output).toBe("95");
    expect(cancelled.evaluations).toEqual([]);
    expect(cancelled.attempt.requestDisposition).toBe("unknown");
    await new Promise((done) => setTimeout(done, 1900));
    expect(await detail(slow.id)).toEqual(cancelled);
    expect(wire).toHaveLength(baseline + 2);
    expect((await start(slowInput)).id).toBe(slow.id);

    mode = "preflight_delay";
    const preparing = start({
      ...base,
      launchKey: "cancel-during-prepare",
    }).then(
      (value) => ({ value, error: null }),
      (error) => ({ value: null, error: String(error) }),
    );
    await expect
      .poll(
        async () =>
          (await nativeInvoke<Bootstrap>(page, "bootstrap")).activeTask?.kind,
      )
      .toBe("benchmark");
    const preparingTask = (await nativeInvoke<Bootstrap>(page, "bootstrap"))
      .activeTask!;
    expect(preparingTask.executionRunId).toBeNull();
    await nativeInvoke(page, "cancel_benchmark", {
      workspaceId,
      experimentId: preparingTask.id,
    });
    expect((await preparing).error).toContain("停止");
    expect(wire).toHaveLength(baseline + 2);

    mode = "swap";
    showCount = 0;
    digest = a;
    const swapped = await start({ ...base, launchKey: "swap-before-send" });
    await expect
      .poll(async () => (await detail(swapped.id)).summary.status)
      .toBe("failed");
    const swapResult = await detail(swapped.id);
    expect(swapResult.attempt.requestDisposition).toBe("not_sent");
    expect(swapResult.evaluations).toEqual([]);
    expect(wire).toHaveLength(baseline + 2);

    mode = "malformed";
    digest = a;
    const malformed = await start({ ...base, launchKey: "malformed-output" });
    await expect
      .poll(async () => (await detail(malformed.id)).summary.status)
      .toBe("failed");
    const failed = await detail(malformed.id);
    expect(failed.output).toBe("95");
    expect(failed.evaluations).toEqual([]);

    mode = "chat_slow";
    await page
      .getByRole("textbox", { name: "你的问题", exact: true })
      .fill("正常聊天互斥检查");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "停止生成", exact: true }),
    ).toBeVisible();
    await expect(
      start({ ...base, launchKey: "benchmark-during-chat" }),
    ).rejects.toBeTruthy();
    expect(
      (await nativeInvoke<Bootstrap>(page, "bootstrap")).activeTask?.kind,
    ).toBe("chat");
    await page.getByRole("button", { name: "停止生成", exact: true }).click();
    await expect
      .poll(
        async () =>
          (await nativeInvoke<Bootstrap>(page, "bootstrap")).activeTask,
      )
      .toBeNull();
    expect(
      await nativeInvoke(page, "get_knowledge", {
        workspaceId,
        nodeId: knowledgeId,
      }),
    ).toEqual(knowledgeBefore);
    expect((await nativeInvoke<Bootstrap>(page, "bootstrap")).settings).toEqual(
      settingsBefore,
    );
    await expect(
      start({ ...base, launchKey: "bad-mode", developmentOnly: false }),
    ).rejects.toBeTruthy();
    await expect(
      start({ ...base, launchKey: "unsupported-case", caseId: "code-001" }),
    ).rejects.toBeTruthy();
    await record(info, "controlled-runner-evidence.json", {
      completed,
      cancelled,
      swapResult,
      failed,
      wire,
      normalChatSettings: settingsBefore,
    });
    expect(app.consoleErrors).toEqual([]);
  } finally {
    await app.stop();
    await new Promise<void>((done, reject) =>
      server.close((error) => (error ? reject(error) : done())),
    );
  }
});

test("one actual local development case persists its request, output and exact score", async ({}, info) => {
  test.skip(
    process.env.CORTEXOS_BENCHMARK_REAL_RUN !== "1",
    "Explicit bounded real local generation",
  );
  test.setTimeout(240_000);
  const app = await launchNative(info);
  try {
    const { page } = app;
    const workspace = await nativeInvoke<{ id: string }>(
      page,
      "create_workspace",
      { name: "真实本地单题开发验证" },
    );
    const before = await nativeInvoke<Bootstrap>(page, "bootstrap");
    const models = await nativeInvoke<{
      models: { name: string; digest: string | null }[];
    }>(page, "list_benchmark_models", { workspaceId: workspace.id });
    const selected =
      models.models.find((m) => m.name === before.settings.model) ??
      models.models.find((m) => /^qwen/i.test(m.name));
    expect(selected?.digest).toMatch(/^[a-f0-9]{64}$/);
    const input = await inputFor(
      page,
      workspace.id,
      {
        model: selected!.name,
        expectedDigest: selected!.digest!,
        parameters: params,
        requirements: ["completion"],
      },
      "real-single-development-case",
    );
    const summary = await nativeInvoke<Summary>(page, "start_benchmark", {
      workspaceId: workspace.id,
      input,
    });
    const get = () =>
      nativeInvoke<Detail>(page, "get_benchmark_run", {
        workspaceId: workspace.id,
        experimentId: summary.id,
      });
    await expect
      .poll(async () => (await get()).summary.status, {
        timeout: 210_000,
        intervals: [500, 1000, 2000],
      })
      .toBe("completed");
    const result = await get();
    expect(result.output.trim()).not.toBe("");
    expect(result.evaluations).toHaveLength(1);
    expect(result.evaluations[0].metrics.correctness).toBe(
      result.output.trim() === "95" ? 1 : 0,
    );
    expect(result.evaluations[0].outputHash).toBe(sha(result.output));
    expect(result.execution.snapshot.request.model).toBe(selected!.name);
    expect(result.attempt.requestDisposition).toBe("finished");
    expect(
      await nativeInvoke<Summary>(page, "start_benchmark", {
        workspaceId: workspace.id,
        input,
      }),
    ).toMatchObject({ id: summary.id, status: "completed" });
    expect(
      await nativeInvoke(page, "list_conversations", {
        workspaceId: workspace.id,
      }),
    ).toEqual([]);
    expect(
      await nativeInvoke(page, "list_runs", { workspaceId: workspace.id }),
    ).toEqual([]);
    expect((await nativeInvoke<Bootstrap>(page, "bootstrap")).settings).toEqual(
      before.settings,
    );
    await record(info, "actual-single-development-case.json", {
      result,
      note: "One local development case; not a 60-case benchmark or cross-model quality result",
    });
  } finally {
    await app.stop();
  }
});
