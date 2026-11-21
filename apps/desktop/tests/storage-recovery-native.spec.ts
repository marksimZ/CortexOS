import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

type History = {
  id: string;
  action: "archive" | "restore";
  backupLocation: string;
  canRestore: boolean;
  canCleanup: boolean;
  cleanup: { bytes: number; at: string } | null;
  outcome: { status: string; detail: string | null; savedBytes: number | null } | null;
}[];

test("native cache confirmation, cancellation, restart backup and conflict-free recovery", async ({}, info) => {
  test.setTimeout(120_000);
  const dataDirectory = info.outputPath("data");
  const profileDirectory = join(dataDirectory, "storage-test-webview");
  const cache = join(profileDirectory, "EBWebView", "Default", "Code Cache");
  await mkdir(cache, { recursive: true });
  const originalFile = join(cache, "cortexos-original-test.bin");
  await writeFile(originalFile, "original");
  const preserved = join(dataDirectory, "unmanaged-test.txt");
  await writeFile(preserved, "unmanaged");
  const launch = async () => {
    const desktop = await launchNative(info, { dataDirectory, profileDirectory, endpoint: "http://127.0.0.1:9" });
    desktop.page.setDefaultTimeout(8_000);
    return desktop;
  };
  let app = await launch();
  const records = () => nativeInvoke<History>(app.page, "storage_recovery_history");
  const settings = async () => {
    await app.page.getByRole("link", { name: "设置", exact: true }).click();
    await app.page.getByRole("button", { name: "本地数据", exact: true }).click();
  };
  const openArchive = async () => {
    const panel = app.page.getByRole("region", { name: "存储清单", exact: true });
    const refresh = panel.getByRole("button", { name: "预览可回收内容", exact: true });
    await refresh.click();
    await expect(refresh).toBeEnabled();
    const summary = panel.getByRole("list", { name: "界面缓存检查" }).locator("summary").filter({ hasText: "界面代码缓存" });
    const details = summary.locator("..");
    if (!await details.evaluate(el => (el as HTMLDetailsElement).open)) await summary.click();
    await details.getByRole("button", { name: "移到备份…", exact: true }).click();
    return app.page.getByRole("dialog", { name: "确认移到备份" });
  };
  try {
    await createWorkspace(app.page, "缓存恢复验收");
    await settings();
    let dialog = await openArchive();
    await expect(dialog.getByRole("button", { name: "取消", exact: true })).toBeFocused();
    await expect(dialog.getByText("文件保留在备份中", { exact: false })).toBeVisible();
    expect(await records()).toEqual([]);
    await dialog.getByRole("button", { name: "取消", exact: true }).click();
    expect(await records()).toEqual([]);
    await app.page.getByRole("button", { name: "移到备份…", exact: true }).click();
    await dialog.getByRole("button", { name: "确认预约", exact: true }).click();
    await expect(app.page.getByText("待下次启动", { exact: true })).toBeVisible();
    expect((await records()).filter(row => !row.outcome)).toHaveLength(1);
    expect(await readFile(originalFile, "utf8")).toBe("original");
    await app.page.getByRole("button", { name: "撤销预约", exact: true }).click();
    await expect(app.page.getByText("已撤销", { exact: true })).toBeVisible();
    expect(await readFile(originalFile, "utf8")).toBe("original");

    const preview = await nativeInvoke<{ reclaimPreview: { approvalId: string } }>(
      app.page, "storage_inventory", { scanId: randomUUID(), preview: true },
    );
    await expect(nativeInvoke(app.page, "storage_recovery_request", {
      request: { kind: "archive", cache: "../Local Storage", approvalId: preview.reclaimPreview.approvalId },
    })).rejects.toThrow("尚未通过预览核对");
    dialog = await openArchive();
    await app.page.setViewportSize({ width: 1366, height: 768 });
    await app.page.screenshot({ path: info.outputPath("recovery-confirm.png") });
    await dialog.getByRole("button", { name: "确认预约", exact: true }).click();
    await expect(app.page.getByText("待下次启动", { exact: true })).toBeVisible();
    await expect(nativeInvoke(app.page, "storage_recovery_request", {
      request: { kind: "archive", cache: "code-cache", approvalId: preview.reclaimPreview.approvalId },
    })).rejects.toThrow("已有一项预约");
    expect(await readFile(originalFile, "utf8")).toBe("original");
    await app.closeFromUi();
    await app.stop();
    app = await launch();

    const history = await records();
    const archived = history.find(row => row.outcome?.status === "archived");
    expect(archived, JSON.stringify(history)).toBeDefined();
    expect(archived?.canRestore).toBe(true);
    expect(archived?.outcome?.savedBytes).toBeGreaterThanOrEqual(8);
    expect(await readFile(join(archived!.backupLocation, "cortexos-original-test.bin"), "utf8")).toBe("original");
    await expect(stat(originalFile)).rejects.toHaveProperty("code", "ENOENT");
    await mkdir(cache, { recursive: true });
    const replacementFile = join(cache, "cortexos-rebuilt-test.bin");
    await writeFile(replacementFile, "rebuilt");
    await settings();
    const archiveRow = app.page.locator("details").filter({ hasText: "已移入备份" });
    await archiveRow.locator("summary").filter({ hasText: "已移入备份" }).click();
    await expect(archiveRow.getByText("0 字节（同盘备份）")).toBeVisible();
    await archiveRow.getByRole("button", { name: "恢复缓存…", exact: true }).click();
    const restoreDialog = app.page.getByRole("dialog", { name: "确认恢复缓存" });
    await restoreDialog.getByRole("button", { name: "确认预约", exact: true }).click();
    await expect(app.page.getByText("待下次启动", { exact: true })).toBeVisible();
    await app.closeFromUi();
    await app.stop();
    app = await launch();

    const final = await records();
    const restored = final.find(row => row.outcome?.status === "restored");
    expect(restored, JSON.stringify(final)).toBeDefined();
    expect(final.find(row => row.id === archived!.id)?.canRestore).toBe(false);
    expect(await readFile(originalFile, "utf8")).toBe("original");
    expect(await readFile(join(restored!.backupLocation, "cortexos-rebuilt-test.bin"), "utf8")).toBe("rebuilt");
    expect(await readFile(preserved, "utf8")).toBe("unmanaged");
    await writeFile(info.outputPath("history.json"), JSON.stringify(final, null, 2));
    await settings();
    await expect(app.page.getByText("已恢复", { exact: true })).toBeVisible();
    await app.page.getByText("已恢复", { exact: true }).scrollIntoViewIfNeeded();
    await app.page.screenshot({ path: info.outputPath("recovery-result.png") });

    // Archive again, then wait for WebView2 itself to recreate the Code Cache.
    dialog = await openArchive();
    await dialog.getByRole("button", { name: "确认预约", exact: true }).click();
    await expect(app.page.getByText("待下次启动", { exact: true })).toBeVisible();
    await app.closeFromUi();
    await app.stop();
    app = await launch();
    await settings();
    const rebuiltIndex = join(cache, "js", "index");
    await expect.poll(async () => (await stat(rebuiltIndex).catch(() => undefined))?.size ?? 0).toBeGreaterThan(0);
    const second = (await records()).find(row => row.id !== archived!.id && row.canCleanup)!;
    expect(second).toBeDefined();
    expect(await readFile(join(second.backupLocation, "cortexos-original-test.bin"), "utf8")).toBe("original");
    await expect(nativeInvoke(app.page, "storage_recovery_request", {
      request: { kind: "cleanupCache", id: second.id, confirmed: false },
    })).rejects.toThrow("请先确认清理");
    await app.page.locator("summary").filter({ hasText: "已移入备份" }).first().click();
    await app.page.getByRole("button", { name: "清理缓存备份…", exact: true }).click();
    const cleanupDialog = app.page.getByRole("dialog", { name: "确认清理缓存备份" });
    await expect(cleanupDialog.getByRole("button", { name: "取消", exact: true })).toBeFocused();
    await app.page.screenshot({ path: info.outputPath("cache-cleanup-confirm.png") });
    await cleanupDialog.getByRole("button", { name: "确认清理", exact: true }).click();
    await expect(app.page.getByText("已清理", { exact: true })).toBeVisible();
    const cleaned = (await records()).find(row => row.id === second.id)!;
    expect(cleaned.cleanup?.bytes).toBeGreaterThanOrEqual(8);
    expect(cleaned.canRestore).toBe(false);
    await expect(stat(second.backupLocation)).rejects.toHaveProperty("code", "ENOENT");
    expect((await stat(rebuiltIndex)).size).toBeGreaterThan(0);
    expect(await readFile(preserved, "utf8")).toBe("unmanaged");
    await writeFile(info.outputPath("cleanup-history.json"), JSON.stringify(cleaned, null, 2));
  } finally {
    await app.stop();
  }
});
