import { execFile } from "node:child_process";
import { mkdir, readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { promisify } from "node:util";
import { expect, test, type Page } from "@playwright/test";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

const exec = promisify(execFile);
const endpoint = "http://127.0.0.1:1";

async function dataSettings(page: Page) {
  await page.getByRole("link", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "本地数据", exact: true }).click();
  return page.getByRole("region", { name: "备份与恢复", exact: true });
}

test("Windows portability", async ({}, info) => {
  test.setTimeout(150_000);
  await mkdir(info.outputDir, { recursive: true });
  const backup = info.outputPath("transfer.cortexbackup");
  const exportPath = info.outputPath("knowledge.json");
  const source = { dataDirectory: info.outputPath("source-data"), profileDirectory: info.outputPath("source-webview"), endpoint };
  const destination = { dataDirectory: info.outputPath("target-data"), profileDirectory: info.outputPath("target-webview"), endpoint };
  const selectFile = (pid: number | undefined, title: string, path: string) => exec("pwsh.exe", [
    "-NoProfile", "-File", resolve("tests/save-report-dialog.ps1"), "-AppProcessId",
    String(pid), "-DialogTitle", title, "-OutputPath", path,
  ], { windowsHide: true, timeout: 25_000 });
  let app = await launchNative(info, source);
  let workspaceId = "";
  let draftKey = "";
  const fakeKey = "sk-h2-only-fake-unpublished-a923c58d34f";
  try {
    await createWorkspace(app.page, "迁移源工作区");
    const boot = await nativeInvoke<{ workspaces: { id: string; name: string }[] }>(app.page, "bootstrap");
    workspaceId = boot.workspaces.find(row => row.name === "迁移源工作区")!.id;
    draftKey = "cortexos.draft.v1.chat." + encodeURIComponent(workspaceId + ":new");
    await app.page.evaluate(({ workspaceId, draftKey }) => {
      localStorage.setItem("cortexos.workspace", workspaceId);
      localStorage.setItem("cortexos.appearance", "dark");
      localStorage.setItem(draftKey, JSON.stringify("只存在源实例的草稿"));
    }, { workspaceId, draftKey });
    await nativeInvoke(app.page, "save_knowledge", { input: {
      workspaceId, title: "迁移知识验收", kind: "knowledge", content: "独立配置下恢复的正文",
      changeReason: "新增迁移测试", nodeId: null, expectedRevision: null,
      sourceMessageId: null, sourceExcerpt: null,
    } });
    const connection = await nativeInvoke<{ id: string }>(app.page, "add_runtime_cloud_connection", {
      workspaceId, input: { provider: "openai", label: "验收连接", apiKey: fakeKey, address: null, compatibleRerank: false },
    });
    const binding = () => nativeInvoke<{ id: string; credentialAvailable: boolean }[]>(app.page, "list_runtime_cloud_connections", { workspaceId });
    expect(await binding()).toContainEqual(expect.objectContaining({ id: connection.id, credentialAvailable: true }));
    const section = await dataSettings(app.page);
    await section.getByRole("button", { name: "预览备份" }).click();
    const preview = app.page.getByRole("dialog", { name: "备份预览" });
    await expect(preview.getByText("工作区 迁移源工作区", { exact: true })).toBeVisible();
    await app.page.setViewportSize({ width: 1366, height: 768 });
    await app.page.screenshot({ path: info.outputPath("backup-preview-1366.png") });
    await preview.getByRole("button", { name: "选择位置并备份…" }).click();
    await selectFile(app.processId, "保存 CortexOS 备份", backup);
    await expect(section.getByText("备份已保存。", { exact: true })).toBeVisible();
    expect((await stat(backup)).size).toBeGreaterThan(1000);
    // Search the inflated application files for the fake key instead of only checking ZIP bytes.
    await exec("py.exe", ["-3", "-c", "import sys,zipfile; z=zipfile.ZipFile(sys.argv[1]); assert z.testzip() is None; assert all(sys.argv[2].encode() not in z.read(n) for n in z.namelist())", backup, fakeKey], { windowsHide: true });
    await section.getByText("导出当前工作区知识").click();
    await section.getByRole("button", { name: "导出知识…" }).click();
    await selectFile(app.processId, "导出工作区知识", exportPath);
    await expect(section.getByText("知识已导出。", { exact: true })).toBeVisible();
    const exported = JSON.parse(await readFile(exportPath, "utf8"));
    expect(exported.versions).toEqual([expect.objectContaining({ title: "迁移知识验收" })]);
    // Remove the synthetic credential through its original database identity.
    await nativeInvoke(app.page, "remove_runtime_cloud_connection", { workspaceId, connectionId: connection.id });
    await app.closeWindow();
    await app.stop();

    app = await launchNative(info, destination);
    await createWorkspace(app.page, "目标临时工作区");
    const destinationSection = await dataSettings(app.page);
    await destinationSection.getByRole("button", { name: "打开备份…" }).click();
    await selectFile(app.processId, "打开 CortexOS 备份", backup);
    const inspect = app.page.getByRole("dialog", { name: "检查备份" });
    await expect(inspect.getByText("文件与 SHA-256 校验通过")).toBeVisible();
    const start = inspect.getByRole("button", { name: "预约恢复" });
    await expect(start).toBeDisabled();
    await inspect.getByRole("checkbox", { name: "确认替换整个应用数据" }).check();
    await app.page.setViewportSize({ width: 1600, height: 900 });
    await app.page.screenshot({ path: info.outputPath("restore-confirm-1600.png") });
    await start.click();
    await expect(destinationSection.getByText("恢复已预约。", { exact: false })).toBeVisible();
    await app.closeWindow();
    await app.stop();

    app = await launchNative(info, destination);
    const after = await dataSettings(app.page);
    await expect(after.getByText("数据、草稿和界面偏好已恢复。")).toBeVisible();
    const restored = await nativeInvoke<{ workspaces: { id: string; name: string }[] }>(app.page, "bootstrap");
    expect(restored.workspaces).toEqual([expect.objectContaining({ id: workspaceId, name: "迁移源工作区" })]);
    expect(await app.page.evaluate(key => [localStorage.getItem(key), localStorage.getItem("cortexos.appearance")], draftKey)).toEqual([
      JSON.stringify("只存在源实例的草稿"), "dark",
    ]);
    expect(await binding()).toContainEqual(expect.objectContaining({ id: connection.id, credentialAvailable: false }));
    const history = await nativeInvoke<{ kind: string; safetyPath: string }[]>(app.page, "backup_history");
    expect(history).toHaveLength(1);
    expect(history[0].kind).toBe("restore");
    expect((await stat(history[0].safetyPath)).size).toBeGreaterThan(1000);
    await app.page.setViewportSize({ width: 1920, height: 1080 });
    await app.page.screenshot({ path: info.outputPath("restore-result-1920.png") });
    await app.page.getByRole("button", { name: "云端模型", exact: true }).click();
    await expect(app.page.getByText("此连接需要在本机绑定密钥。")).toBeVisible();
    await app.page.getByLabel("API 密钥", { exact: true }).fill(fakeKey);
    await app.page.getByRole("button", { name: "绑定密钥", exact: true }).click();
    await expect(app.page.getByText("密钥已在本机绑定。可以刷新模型列表检查连接。")).toBeVisible();
    expect(await binding()).toContainEqual(expect.objectContaining({ id: connection.id, credentialAvailable: true }));
    await nativeInvoke(app.page, "remove_runtime_cloud_connection", { workspaceId, connectionId: connection.id });
  } finally {
    await app.stop();
  }
});
