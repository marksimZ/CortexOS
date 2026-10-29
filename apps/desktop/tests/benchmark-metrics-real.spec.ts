import { expect, test } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { launchNative, nativeInvoke } from "./native";
import type { Batch, ModelRequest } from "../src/benchmark/personalized";

test("two bounded real local repetitions preserve actual timing, tokens and resource scope", async ({}, info) => {
  test.setTimeout(300_000);
  const savedDirectory = process.env.CORTEXOS_BENCHMARK_METRICS_SAVED;
  const saved: { report: Batch; details: { summary: { workspaceId: string } }[] } | null = savedDirectory ? JSON.parse(await readFile(resolve(savedDirectory, "real-metrics.json"), "utf8")) : null;
  const app = await launchNative(info, savedDirectory ? { dataDirectory: resolve(savedDirectory, "data") } : {});
  try {
    const { page } = app;
    const workspace = saved ? { id: saved.details[0].summary.workspaceId } : await nativeInvoke<{ id: string }>(page, "create_workspace", { name: "真实本地指标小范围验证" });
    const workspaceId = workspace.id;
    const before = await nativeInvoke<{ settings: { model: string } }>(page, "bootstrap");
    let batchId = saved?.report.id;
    if (!saved) {
      const catalog = await nativeInvoke<{ models: { name: string; digest: string | null }[] }>(page, "list_benchmark_models", { workspaceId });
      const selected = catalog.models.find(m => m.name === before.settings.model && m.digest) ?? catalog.models.find(m => m.digest);
      expect(selected).toBeTruthy();
      const datasets = await nativeInvoke<{ id: string; version: number; contentHash: string }[]>(page, "list_benchmark_datasets", { workspaceId });
      const dataset = datasets.find(d => d.id === "cortex-core-60" && d.version === 2)!;
      const local: ModelRequest = { model: selected!.name, expectedDigest: selected!.digest, parameters: { numCtx: 2048, numPredict: 64, temperature: 0, timeoutSeconds: 120 }, requirements: ["completion"] };
      const planned = await nativeInvoke<Batch>(page, "plan_standard_benchmark", { workspaceId, input: { datasetId: dataset.id, datasetVersion: 2, datasetHash: dataset.contentHash, models: [{ kind: "local", local }], maxOutputTokens: 64, price: null, pythonImage: null, caseIds: ["math-001"], repetitions: 2, releaseModels: [] } });
      batchId = planned.id;
      await nativeInvoke(page, "start_standard_benchmark", { workspaceId, batchId, approvedPlanHash: planned.planHash });
    }
    const get = () => nativeInvoke<Batch>(page, "get_standard_benchmark", { workspaceId, batchId });
    await expect.poll(async () => (await get()).status, { timeout: 260_000, intervals: [1000, 2000] }).toBe("completed");
    const report = await get(), model = report.reports![0];
    expect(model.performance.firstTextMs?.n).toBe(2);
    expect(model.performance.speedCases).toBe(2);
    expect(model.performance.tokensPerSecond).toBeGreaterThan(0);
    const details = [];
    for (const item of model.results) {
      const detail = await nativeInvoke<{ execution: { events: { kind: string; payload: { metrics: Record<string, number> } }[] }; resources: unknown[]; output: string }>(page, "get_benchmark_run", { workspaceId, experimentId: item.runId });
      const raw = detail.execution.events.find(e => e.kind === "response_completed")!.payload.metrics;
      expect(item.performance?.inputTokens).toBe(raw.promptEvalCount);
      expect(item.performance?.outputTokens).toBe(raw.evalCount);
      expect(item.performance?.tokensPerSecond).toBeCloseTo(raw.evalCount * 1e9 / raw.evalDurationNs);
      expect(detail.resources.length).toBeGreaterThanOrEqual(2);
      details.push(detail);
    }
    expect(await nativeInvoke(page, "list_conversations", { workspaceId })).toEqual([]);
    expect((await nativeInvoke<{ settings: unknown }>(page, "bootstrap")).settings).toEqual(before.settings);
    await writeFile(info.outputPath("real-metrics.json"), JSON.stringify({ reusedSavedRun: !!saved, workspaceId, note: "1 actual installed model, 1 fixed smoke question, 2 repetitions; no controlled cold cache or cross-model ranking", report, details }, null, 2));
    await page.evaluate(({ workspaceId, batchId }) => { location.hash = `/benchmark?workspace=${workspaceId}&batch=${batchId}`; }, { workspaceId, batchId });
    await page.reload();
    await expect(page.getByRole("heading", { name: "标准评测进度与结果", exact: true })).toBeVisible();
    await page.getByText("速度、Token 与资源", { exact: true }).click();
    await page.setViewportSize({ width: 1600, height: 900 });
    await page.locator(".benchmark-measurements").first().scrollIntoViewIfNeeded();
    await page.screenshot({ path: info.outputPath("real-metrics-1600.png") });
  } finally { await app.stop(); }
});
