import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { mkdir, writeFile } from "node:fs/promises";
import { launchNative, nativeInvoke } from "./native";

type Tag = { name: string; digest: string | null; quantization: string | null };
type Catalog = {
  providerId: string;
  models: Tag[];
  loaded: unknown[] | null;
  version: string | null;
};
type Preflight = {
  status: string;
  request: unknown;
  catalog: Catalog;
  inspection: {
    capabilities: { name: string; status: string; evidence: string }[];
    contextLength: number | null;
    metadata: unknown;
  } | null;
  issues: { code: string; message: string }[];
};
const parameters = {
  numCtx: 4096,
  numPredict: 512,
  temperature: 0.3,
  timeoutSeconds: 180,
};

test("native benchmark definitions and controlled model preflight retain isolation", async ({}, info) => {
  test.setTimeout(90_000);
  const a = "a".repeat(64),
    b = "b".repeat(64);
  let scenario = "normal",
    changed = false;
  const requests: { method: string; path: string; body: string }[] = [];
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const chunk of req) body += chunk.toString();
    requests.push({ method: req.method!, path: req.url!, body });
    res.setHeader("Content-Type", "application/json");
    const details = {
      format: scenario === "format" ? "safetensors" : "gguf",
      family: "fixture",
      parameter_size: "1B",
      quantization_level: "fixture_Q4",
    };
    if (req.url === "/api/tags") {
      if (scenario === "unavailable") {
        res.statusCode = 503;
        res.end("{}");
      } else
        res.end(
          JSON.stringify({
            models: [
              {
                name: "fixture:local",
                digest: scenario === "no_digest" ? undefined : changed ? b : a,
                size: 123,
                details,
              },
            ],
          }),
        );
    } else if (req.url === "/api/version")
      res.end(JSON.stringify({ version: "controlled-preflight" }));
    else if (req.url === "/api/ps") {
      if (scenario === "ps_failed") {
        res.statusCode = 503;
        res.end("{}");
      } else
        res.end(
          JSON.stringify({
            models: [
              {
                name: "fixture:local",
                digest: a,
                size: 123,
                size_vram: 100,
                context_length: 2048,
              },
            ],
          }),
        );
    } else if (req.url === "/api/show") {
      if (scenario === "swap") changed = true;
      res.end(
        JSON.stringify({
          details,
          ...(scenario === "remote" ? { remote_host: "cloud.example" } : {}),
          capabilities:
            scenario === "unknown"
              ? undefined
              : scenario === "unsupported"
                ? []
                : ["completion", "tools", "thinking"],
          model_info:
            scenario === "unknown"
              ? {}
              : {
                  "general.architecture": "fixture",
                  "fixture.context_length": scenario === "small" ? 2048 : 8192,
                },
        }),
      );
    } else {
      res.statusCode = 400;
      res.end(
        JSON.stringify({ error: "Unexpected request; no generation allowed" }),
      );
    }
  });
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("Missing fixture port");
  const app = await launchNative(info, {
    endpoint: `http://127.0.0.1:${address.port}`,
  });
  try {
    const { page } = app;
    const ws = await nativeInvoke<{ id: string }>(page, "create_workspace", {
      name: "切片2隔离预检",
    });
    const before = await nativeInvoke(page, "bootstrap");
    const invoke = (request: unknown, workspaceId = ws.id) =>
      nativeInvoke<Preflight>(page, "inspect_benchmark_model", {
        workspaceId,
        request,
      });
    const request = {
      model: "fixture:local",
      expectedDigest: a,
      parameters,
      requirements: ["completion"],
    };
    const datasets = await nativeInvoke<
      {
        id: string;
        version: number;
        contentHash: string;
        manifest: {
          case_count: number;
          review_status: string;
          case_order: string[];
        };
      }[]
    >(page, "list_benchmark_datasets", { workspaceId: ws.id });
    expect(datasets).toHaveLength(2);
    expect(datasets.map((dataset) => dataset.version)).toEqual([1, 2]);
    expect(datasets[0].manifest.case_count).toBe(60);
    expect(new Set(datasets[0].manifest.case_order).size).toBe(60);
    expect(datasets[0].manifest.review_status).toBe("needs_human_review");
    const c = await nativeInvoke<{
      fixture: { lines: string[] }[];
      content_hash: string;
    }>(page, "get_benchmark_case", {
      workspaceId: ws.id,
      datasetId: datasets[0].id,
      version: 1,
      caseId: "rag-008",
    });
    expect(c.fixture[0].lines[1]).toBe("服务台位于二层。");
    expect(c.content_hash).toMatch(/^[a-f0-9]{64}$/);
    const v2 = await nativeInvoke<{
      dataset_version: number;
      expected: { kind: string; answer: { facts: { floor: number } } };
      scoring: { version: number; contract: { citation_policy: string } };
    }>(page, "get_benchmark_case", {
      workspaceId: ws.id,
      datasetId: "cortex-core-60",
      version: 2,
      caseId: "rag-008",
    });
    expect(v2.dataset_version).toBe(2);
    expect(v2.expected.answer.facts.floor).toBe(2);
    expect(v2.scoring.version).toBe(2);
    expect(v2.scoring.contract.citation_policy).toBe("enumerated_supporting_spans");
    await expect(
      nativeInvoke(page, "get_benchmark_case", {
        workspaceId: "missing",
        datasetId: datasets[0].id,
        version: 1,
        caseId: "rag-008",
      }),
    ).rejects.toBeTruthy();
    const catalog = await nativeInvoke<Catalog>(page, "list_benchmark_models", {
      workspaceId: ws.id,
    });
    expect(catalog.models[0].digest).toBe(a);
    expect(catalog.loaded).toHaveLength(1);
    const ready = await invoke(request);
    expect(ready.status).toBe("ready_for_probe");
    expect(ready.inspection?.contextLength).toBe(8192);
    expect(
      ready.inspection?.capabilities.find((c) => c.name === "tool_calls"),
    ).toMatchObject({ status: "supported", evidence: "api_show_advertised" });
    expect(
      (await invoke({ ...request, model: "not-installed:fixture" })).status,
    ).toBe("not_installed");
    expect((await invoke({ ...request, expectedDigest: b })).status).toBe(
      "identity_changed",
    );
    expect((await invoke({ ...request, expectedDigest: null })).status).toBe(
      "needs_verification",
    );
    expect(
      (
        await invoke({
          ...request,
          requirements: ["completion", "json_schema"],
        })
      ).issues,
    ).toContainEqual(expect.objectContaining({ code: "capability_unknown" }));
    scenario = "unknown";
    const unknown = await invoke(request);
    expect(unknown.status).toBe("needs_verification");
    expect(unknown.inspection?.contextLength).toBeNull();
    expect(unknown.issues).toContainEqual(
      expect.objectContaining({ code: "context_unknown" }),
    );
    scenario = "no_digest";
    const noDigest = await invoke(request);
    expect(noDigest.status).toBe("needs_verification");
    expect(noDigest.issues).toContainEqual(
      expect.objectContaining({ code: "digest_unknown" }),
    );
    scenario = "unavailable";
    await expect(invoke(request)).rejects.toBeTruthy();
    scenario = "unsupported";
    expect((await invoke(request)).issues).toContainEqual(
      expect.objectContaining({ code: "capability_unsupported" }),
    );
    for (const value of ["remote", "format", "small"]) {
      scenario = value;
      expect((await invoke(request)).status).toBe("blocked");
    }
    scenario = "ps_failed";
    const missingPs = await invoke(request);
    expect(missingPs.catalog.loaded).toBeNull();
    expect(
      (missingPs.catalog as Catalog & { warnings: string[] }).warnings.length,
    ).toBeGreaterThan(0);
    scenario = "swap";
    expect((await invoke(request)).status).toBe("identity_changed");
    const count = requests.length;
    await expect(invoke(request, "missing")).rejects.toBeTruthy();
    await expect(
      invoke({ ...request, parameters: { ...parameters, numCtx: 999999 } }),
    ).rejects.toBeTruthy();
    expect(requests).toHaveLength(count);
    expect(
      requests.every((r) =>
        ["/api/tags", "/api/version", "/api/ps", "/api/show"].includes(r.path),
      ),
    ).toBe(true);
    expect(
      requests
        .filter((r) => r.method === "POST")
        .every(
          (r) =>
            r.path === "/api/show" &&
            JSON.stringify(JSON.parse(r.body)) ===
              JSON.stringify({ model: "fixture:local" }),
        ),
    ).toBe(true);
    expect(await nativeInvoke(page, "bootstrap")).toEqual(before);
    await mkdir(info.outputDir, { recursive: true });
    const evidence = info.outputPath("controlled-preflight.json");
    await writeFile(
      evidence,
      JSON.stringify({ datasets, v1Case: c, v2Case: v2, ready, unknown, missingPs, requests }, null, 2),
    );
    await info.attach("controlled-preflight.json", {
      path: evidence,
      contentType: "application/json",
    });
    expect(app.consoleErrors).toEqual([]);
  } finally {
    await app.stop();
    await new Promise<void>((done, reject) =>
      server.close((e) => (e ? reject(e) : done())),
    );
  }
});

test("native read-only preflight captures two actually installed Qwen identities", async ({}, info) => {
  test.skip(
    process.env.CORTEXOS_BENCHMARK_REAL_MODELS !== "1",
    "Explicit read-only local metadata check",
  );
  test.setTimeout(90_000);
  const app = await launchNative(info);
  try {
    const ws = await nativeInvoke<{ id: string }>(
      app.page,
      "create_workspace",
      { name: "本机模型只读预检" },
    );
    const before = await nativeInvoke(app.page, "bootstrap");
    const catalog = await nativeInvoke<Catalog>(
      app.page,
      "list_benchmark_models",
      { workspaceId: ws.id },
    );
    const qwen = catalog.models.filter((m) => /^qwen/i.test(m.name));
    expect(qwen.length).toBeGreaterThanOrEqual(2);
    const results: Preflight[] = [];
    for (const model of qwen.slice(0, 2)) {
      expect(model.digest).toMatch(/^[a-f0-9]{64}$/);
      const result = await nativeInvoke<Preflight>(
        app.page,
        "inspect_benchmark_model",
        {
          workspaceId: ws.id,
          request: {
            model: model.name,
            expectedDigest: model.digest,
            parameters,
            requirements: ["completion"],
          },
        },
      );
      expect(result.status).toBe("ready_for_probe");
      expect(result.inspection?.contextLength).toBeGreaterThanOrEqual(
        parameters.numCtx,
      );
      results.push(result);
    }
    expect(
      new Set(
        results.map(
          (r) =>
            r.catalog.models.find(
              (m) => m.name === (r.request as { model: string }).model,
            )?.digest,
        ),
      ).size,
    ).toBe(2);
    expect(await nativeInvoke(app.page, "bootstrap")).toEqual(before);
    await mkdir(info.outputDir, { recursive: true });
    const evidence = info.outputPath("actual-local-model-metadata.json");
    await writeFile(
      evidence,
      JSON.stringify(
        {
          catalog,
          results,
          note: "Metadata only; no generation, loading, unloading or capability probe performed",
        },
        null,
        2,
      ),
    );
    await info.attach("actual-local-model-metadata.json", {
      path: evidence,
      contentType: "application/json",
    });
  } finally {
    await app.stop();
  }
});
