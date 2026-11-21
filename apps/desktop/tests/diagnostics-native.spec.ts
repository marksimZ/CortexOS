import { execFile, spawn } from "node:child_process";
import { once } from "node:events";
import { mkdir, readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import { expect, test } from "@playwright/test";
import { createWorkspace, launchNative, nativeInvoke } from "./native";
import type { DiagnosticsReport } from "../src/api/diagnostics";

const exec = promisify(execFile);
const privateSample = [
  "credential-diagnostic-fixture-91e26",
  "chat-private-diagnostic-34a6",
  "knowledge-private-diagnostic-b9c0",
  "code-private-diagnostic-97c6",
  "https://private-endpoint.example.invalid",
  "workspace-private-diagnostic-a15e",
];
const seed = [
  "import json,sqlite3,sys,time",
  "c=sqlite3.connect(sys.argv[1]); ws,secret,chat,knowledge,code,url=sys.argv[2:]",
  "c.execute('INSERT INTO model_cloud_connections VALUES(?,?,?)',(ws,'fixture',json.dumps(dict(provider='openai',catalogState='verified',address=url,apiKey=secret))))",
  "for reason in ('rate_limited','invalid_credential','service_error','network_unavailable',secret):",
  " c.execute('INSERT INTO model_cloud_calls (id,workspace_id,operation_id,purpose,connection_id,provider,model,capability,data_types_json,started_unix,state,failure) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)',(reason,ws,'fixture','test','fixture','openai',url,'generation','[]',int(time.time()),'failed',reason))",
  "c.execute('INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,error,created_at,finished_at) VALUES(?,?,?,?,?,?,?,?)',('fixture',ws,'failed',json.dumps(dict(purpose='harness_workspace',code=code)), 'fixture',chat,'2026-01-01',time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())))",
  "c.execute('INSERT INTO app_settings VALUES(?,?)',('fixture',json.dumps(dict(chat=chat,knowledge=knowledge))))",
  "c.commit()",
].join("\n");

test("native diagnostics export only known fields", async ({}, info) => {
  test.setTimeout(80_000);
  let online = true;
  let inference = 0;
  const server = createServer((req, res) => {
    res.setHeader("Content-Type", "application/json");
    switch (req.url) {
      case "/api/version":
        res.statusCode = online ? 200 : 503;
        return res.end(JSON.stringify({ version: privateSample[0] }));
      case "/api/tags":
      case "/api/ps":
        return res.end(JSON.stringify({ models: [] }));
      default:
        if (req.url === "/api/chat" || req.url === "/api/embed") inference++;
        res.statusCode = 404;
        return res.end("{}");
    }
  });
  await new Promise<void>(done => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  if (!address || typeof address === "string") throw Error("fixture endpoint unavailable");
  const dataDirectory = info.outputPath("data");
  let app: Awaited<ReturnType<typeof launchNative>> | undefined;
  try {
    await mkdir(info.outputDir, { recursive: true });
    app = await launchNative(info, { dataDirectory, endpoint: "http://127.0.0.1:" + address.port });
    const { page } = app;
    await page.setViewportSize({ width: 1366, height: 768 });
    await createWorkspace(page, privateSample[5]);
    const workspace = await page.getByLabel("当前工作区").inputValue();
    await exec("py.exe", ["-3", "-c", seed, join(dataDirectory, "cortexos.sqlite3"), workspace, ...privateSample.slice(0, 5)],
      { windowsHide: true, timeout: 15_000 });
    await page.getByRole("link", { name: "设置", exact: true }).click();
    await page.getByRole("button", { name: "关于", exact: true }).click();
    const panel = page.getByRole("region", { name: "本机诊断" });
    await panel.getByRole("button", { name: "检查运行状态" }).click();
    await expect(panel.getByText("结构版本 26")).toBeVisible();
    await panel.getByText("最近错误（24 小时）", { exact: true }).click();
    await expect(panel.getByText("云端请求 密钥无效（1）")).toBeVisible();
    await expect(panel.getByText("稍后手动重试。")).toHaveCount(2);
    await page.screenshot({ path: info.outputPath("diagnostics-1366.png"), animations: "disabled" });

    online = false;
    await expect(nativeInvoke<DiagnosticsReport>(page, "read_diagnostics")).resolves.toMatchObject({
      backend: "available", ollama: "unavailable", queue: { queued: 0, running: 0, cancelling: 0 },
      database: { schemaVersion: 26, cloudConnections: [{ provider: "openai", status: "verified", count: 1 }] },
    });
    const output = info.outputPath("diagnostics.json");
    await panel.getByRole("button", { name: "导出诊断报告…" }).click();
    await exec("pwsh.exe", ["-NoProfile", "-File", resolve("tests/save-report-dialog.ps1"), "-AppProcessId",
      String(app.processId), "-DialogTitle", "保存本机诊断报告", "-OutputPath", output], { windowsHide: true, timeout: 25_000 });
    await expect(panel.getByText("诊断报告已保存。")).toBeVisible();
    const contents = await readFile(output, "utf8");
    for (const value of [...privateSample, workspace, dataDirectory, "apiKey", "GPU-"]) {
      expect(contents).not.toContain(value);
    }
    const report = JSON.parse(contents) as DiagnosticsReport & { formatVersion: number };
    expect(report.formatVersion).toBe(1);
    expect(report.database?.problems).toEqual(expect.arrayContaining([
      expect.objectContaining({ area: "harness", category: "failed" }),
      expect.objectContaining({ category: "invalid_credential", nextAction: "check_credentials" }),
      expect.objectContaining({ category: "rate_limited", nextAction: "retry_manually_later" }),
      expect.objectContaining({ category: "service_error", nextAction: "retry_manually_later" }),
      expect.objectContaining({ category: "network_unavailable", nextAction: "check_connection" }),
    ]));
    expect(inference).toBe(0);

    const writer = spawn("py.exe", ["-3", "-u", "-c",
      "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute('BEGIN IMMEDIATE'); print('locked'); sys.stdin.read(1); c.rollback()",
      join(dataDirectory, "cortexos.sqlite3")], { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
    let locked = "";
    writer.stdout.on("data", value => { locked += value.toString(); });
    try {
      await expect.poll(() => locked.trim()).toBe("locked");
      await expect(nativeInvoke(page, "create_workspace", { name: "未提交的工作区" })).rejects.toThrow("本地数据读写失败");
      const original = await nativeInvoke<{ workspaces: { id: string }[] }>(page, "bootstrap");
      expect(original.workspaces.map(row => row.id)).toEqual([workspace]);
    } finally {
      if (writer.exitCode === null) {
        const exited = once(writer, "exit");
        writer.stdin.end();
        await exited;
      }
    }
    await expect(nativeInvoke(page, "create_workspace", { name: "锁释放后可以写入" })).resolves.toMatchObject({ name: "锁释放后可以写入" });
  } finally {
    await app?.stop();
    server.closeAllConnections();
    await new Promise<void>(done => server.close(() => done()));
  }
});
