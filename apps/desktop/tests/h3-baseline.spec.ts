import { execFile } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { cpus, release, totalmem } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { promisify } from "node:util";
import { expect, test } from "@playwright/test";
import { createWorkspace, launchNative, nativeInvoke } from "./native";
import { seedGraph } from "./graph-fixture";
import type { Bootstrap } from "../src/studio";

type Sample = {
  processes: { pid: number; webview: boolean; rss: number; private: number; handles: number; cpuTicks: number }[];
  wholeGpu: string[] | null;
};

const exec = promisify(execFile);
const target = "http://127.0.0.1:1";
const stats = (observations: number[]) => {
  const sorted = observations.toSorted((a, b) => a - b);
  const n = sorted.length;
  return { n, p50: (sorted[Math.floor((n - 1) / 2)] + sorted[Math.floor(n / 2)]) / 2, p95: sorted[Math.ceil(n * .95) - 1] };
};
const utc = () => new Date().toISOString();

test("H3 deterministic native data and idle baseline", async ({}, info) => {
  const idleSeconds = Number(process.env.CORTEXOS_H3_IDLE_SECONDS ?? 60);
  if (!Number.isInteger(idleSeconds) || idleSeconds < 30 || idleSeconds > 900) throw Error("Set an idle window between 30 and 900 seconds");
  test.setTimeout(180_000 + idleSeconds * 1000);
  await mkdir(info.outputDir, { recursive: true });
  const dataDirectory = info.outputPath("data");
  const sourcePath = join(dataDirectory, "fixture.txt");
  let app: Awaited<ReturnType<typeof launchNative>> | undefined;
  const starts = { freshProfile: [] as number[], existingProfile: [] as number[] };
  const timings = { importVersion: [] as number[], lexical: [] as number[], graphCycles: [] as number[], bootstrapIpc: [] as number[] };
  const samples: (Sample & { phase: string; at: string; ms: number })[] = [];
  const started = performance.now();
  const sample = async (phase: string) => {
    const { stdout } = await exec("pwsh.exe", ["-NoProfile", "-File", resolve("tests/h3-sample.ps1"),
      "-AppProcessId", String(app!.processId)], { windowsHide: true, timeout: 12_000 });
    const result = JSON.parse(stdout) as Sample;
    expect(result.processes.some(row => row.pid === app!.processId)).toBeTruthy();
    samples.push({ ...result, phase, at: utc(), ms: performance.now() - started });
  };
  try {
    for (let iteration = 0; iteration < 3; iteration++) {
      const profileDirectory = info.outputPath("profile-" + iteration);
      for (const name of ["freshProfile", "existingProfile"] as const) {
        const before = performance.now();
        app = await launchNative(info, { endpoint: target, dataDirectory, profileDirectory, sourcePath });
        await nativeInvoke(app.page, "bootstrap");
        starts[name].push(performance.now() - before);
        if (name === "existingProfile" && iteration === 2) break;
        await app.closeWindow();
        await app.stop();
        app = undefined;
      }
    }
    const { page } = app!;
    await page.setViewportSize({ width: 1600, height: 900 });
    page.setDefaultTimeout(15_000);
    await createWorkspace(page, "H3 一组");
    const workspaceMenu = page.getByLabel("当前工作区");
    const workspaceId = await workspaceMenu.inputValue();
    await createWorkspace(page, "H3 二组");
    const second = await workspaceMenu.inputValue();
    for (const id of [workspaceId, second]) await seedGraph(page, id, 80, 160);
    await workspaceMenu.selectOption(workspaceId);
    await nativeInvoke(page, "create_conversation", { workspaceId });
    const conversations = await nativeInvoke<{ id: string }[]>(page, "list_conversations", { workspaceId });
    const conversationId = conversations[0].id;
    for (let i = 0; i < 8; i++) {
      await writeFile(sourcePath, "H3 固定知识导入版本 " + i + "\n" + "本机备份知识资料。\n".repeat(256));
      const before = performance.now();
      const ticket = await nativeInvoke<{ticket: string}>(page, "choose_source", { workspaceId });
      await nativeInvoke(page, "commit_source", { workspaceId, ticket: ticket.ticket });
      timings.importVersion.push(performance.now() - before);
    }
    const knowledge = await nativeInvoke<string>(page, "save_knowledge", {
      input: { workspaceId, title: "SQLite 备份", content: "本机数据库使用一致性备份。", kind: "knowledge", changeReason: "H3 固定样本" },
    });
    await nativeInvoke(page, "approve_knowledge", { workspaceId, nodeId: knowledge, expectedRevision: 1 });
    for (let index = 0; index < 20; index++) {
      const before = performance.now();
      const result = await nativeInvoke<{ retrieval: { candidates: { nodeId: string }[] } }>(page, "preview_context", {
        workspaceId, conversationId, content: "SQLite 备份", contextSelection: [],
      });
      timings.lexical.push(performance.now() - before);
      expect(result.retrieval.candidates.some(node => node.nodeId === knowledge)).toBe(true);
    }
    await page.getByRole("link", { name: "对话", exact: true }).click();
    await sample("idle");
    const idleStart = performance.now();
    while (performance.now() - idleStart < idleSeconds * 1000) {
      await delay(4500);
      const before = performance.now();
      const found = await nativeInvoke<Bootstrap>(page, "bootstrap");
      timings.bootstrapIpc.push(performance.now() - before);
      expect(found.workspaces).toHaveLength(2);
      await sample("idle");
    }
    const idleObservedMs = performance.now() - idleStart;
    for (let iteration = 0; iteration < 12; iteration++) {
      const before = performance.now();
      await workspaceMenu.selectOption(iteration % 2 === 0 ? second : workspaceId);
      await page.getByRole("link", { name: "图谱", exact: true }).click();
      const count = page.locator(".graph-visible-count");
      await expect(count).toContainText(iteration % 2 === 0 ? "80" : "81");
      await page.getByRole("link", { name: "知识", exact: true }).click();
      timings.graphCycles.push(performance.now() - before);
      if (iteration % 3 === 2) await sample("switching");
    }
    await page.getByRole("link", { name: "对话", exact: true }).click();
    for (let i = 0; i < 3; i++) {
      await delay(4500);
      await sample("settled");
    }
    const integrity = await exec("py.exe", ["-3", "-c",
      "import pathlib,sqlite3,sys; c=sqlite3.connect(pathlib.Path(sys.argv[1]).resolve().as_uri()+'?mode=ro',uri=True); print(c.execute('pragma quick_check').fetchone()[0]); print(len(list(c.execute('pragma foreign_key_check'))))",
      join(dataDirectory, "cortexos.sqlite3")], { windowsHide: true, timeout: 12_000 });
    expect(integrity.stdout.trim().split(/\s+/)).toEqual(["ok", "0"]);
    const summary = {
      capturedAt: utc(), executor: "native-custom-protocol-loopback-unavailable", system: release(),
      cpu: cpus()[0]?.model, logicalProcessors: cpus().length, ramBytes: totalmem(),
      viewport: { width: 1600, height: 900 }, idleObservedMs, runMs: performance.now() - started,
      starts: Object.fromEntries(Object.entries(starts).map(([key, values]) => [key, { ...stats(values), values }])),
      timings: Object.fromEntries(Object.entries(timings).map(([key, values]) => [key, { ...stats(values), values }])),
      samples,
    };
    await writeFile(info.outputPath("h3-native-baseline.json"), JSON.stringify(summary, null, 2));
  } finally {
    if (app) await app.stop();
  }
});
