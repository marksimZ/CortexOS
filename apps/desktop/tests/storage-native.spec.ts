import { randomUUID } from "node:crypto";
import { mkdir, readFile, stat, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

type Report = {
  fileBytes: number;
  partialFileTotal: boolean;
  referenceError: string | null;
  items: {
    id: string;
    location: string | null;
    size: { logicalBytes: number; allocatedBytes: number | null; files: number; skippedEntries: number } | null;
    payloadBytes: number | null;
    references: { label: string; count: number }[] | null;
  }[];
  reclaimPreview: {
    recoveryDirectory: string | null;
    recoveryFreeBytes: number | null;
    cacheItems: { item: Report["items"][number]; assessment: string }[];
  } | null;
};

test("native storage inventory measures known files, preserves data and reports real references", async ({}, info) => {
  const dataDirectory = info.outputPath("data");
  const privateDirectory = info.outputPath("outside-data");
  await mkdir(dataDirectory, { recursive: true });
  await mkdir(privateDirectory, { recursive: true });
  const selectedDirectory = join(dataDirectory, "selected-input");
  await mkdir(selectedDirectory);
  const sourcePath = join(selectedDirectory, "source.md");
  const source = "交付验收需要双人确认。";
  await writeFile(sourcePath, source);
  await writeFile(join(privateDirectory, "external.bin"), Buffer.alloc(32768));
  const backup = join(dataDirectory, `before-migration-2-${randomUUID()}.sqlite3`);
  await writeFile(backup, Buffer.alloc(4096));
  await writeFile(join(dataDirectory, "unknown-cache.sqlite3"), Buffer.alloc(65536));
  await symlink(privateDirectory, join(dataDirectory, `before-migration-3-${randomUUID()}.sqlite3`), "junction");

  const app = await launchNative(info, { dataDirectory, sourcePath, endpoint: "http://127.0.0.1:9" });
  try {
    const { page } = app;
    await createWorkspace(page, "存储审计");
    const workspaceId = await page.getByLabel("当前工作区").inputValue();
    const { ticket } = await nativeInvoke<{ ticket: string }>(page, "choose_source", { workspaceId });
    const saved = await nativeInvoke<{ id: string }>(page, "commit_source", { workspaceId, ticket });
    const nodeId = await nativeInvoke<string>(page, "save_source_knowledge", {
      input: { workspaceId, title: "交付验收", kind: "knowledge", content: source, changeReason: "隔离检查" },
      source: { sourceId: saved.id, version: 1, startLine: 1, endLine: 1 },
    });
    await nativeInvoke(page, "approve_knowledge", { workspaceId, nodeId, expectedRevision: 1 });
    await nativeInvoke(page, "capture_knowledge_snapshot", { workspaceId, name: "验收依据" });
    const db = join(dataDirectory, "cortexos.sqlite3");
    const wal = `${db}-wal`;
    const original = await Promise.all([readFile(db), readFile(wal), readFile(backup)]);

    await expect(nativeInvoke(page, "storage_inventory", { scanId: "invalid" })).rejects.toThrow("读取编号无效");
    const report = await nativeInvoke<Report>(page, "storage_inventory", { scanId: randomUUID() });
    await writeFile(info.outputPath("inventory.json"), JSON.stringify(report, null, 2));
    const item = (id: string) => report.items.find(row => row.id === id)!;
    expect(report.referenceError).toBeNull();
    expect(report.reclaimPreview).toBeNull();
    expect(report.partialFileTotal).toBe(true);
    expect(item("database").size?.logicalBytes).toBe((await stat(db)).size);
    expect(item("wal").size?.logicalBytes).toBe((await stat(wal)).size);
    expect(item("backups").size).toMatchObject({ logicalBytes: 4096, files: 1, skippedEntries: 1 });
    expect(item("backups").size?.allocatedBytes).toBeGreaterThan(0);
    expect(item("webview").size).toBeNull();
    expect(item("sources").payloadBytes).toBe(Buffer.byteLength(source));
    expect(item("sources").references?.map(row => row.count)).toEqual([1, 1]);
    expect(item("history").references).toContainEqual({ label: "知识快照中的版本引用", count: 1 });
    expect(report.fileBytes).toBe(report.items.reduce((total, row) => total + (row.size?.logicalBytes ?? 0), 0));
    expect(JSON.stringify(report)).not.toContain("outside-data");

    await page.getByRole("link", { name: "设置", exact: true }).click();
    await page.getByRole("button", { name: "本地数据", exact: true }).click();
    const inventory = page.getByRole("region", { name: "存储清单" });
    await inventory.getByRole("button", { name: "读取存储清单", exact: true }).click();
    await expect(inventory.getByText("已统计的应用文件", { exact: true })).toBeVisible();
    await inventory.getByText("已保存的资料副本").click();
    await expect(inventory.getByText("知识到文件版本的引用", { exact: true })).toBeVisible();
    await expect(inventory.getByText("部分引用未能读取", { exact: false })).toHaveCount(0);
    await page.getByText("工作区已创建", { exact: true }).waitFor({ state: "hidden" });
    for (const [width, height, theme] of [[1366, 768, "light"], [1600, 900, "light"], [1920, 1080, "dark"]] as const) {
      await page.setViewportSize({ width, height });
      await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
      await inventory.getByText("存储清单", { exact: true }).scrollIntoViewIfNeeded();
      expect(await inventory.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      const pathLabel = inventory.locator("details[open]").getByText("位置", { exact: true });
      expect(await pathLabel.evaluate(el => el.clientWidth >= el.scrollWidth && el.clientHeight < 30)).toBe(true);
      await page.screenshot({ path: info.outputPath(`storage-${width}-${theme}.png`) });
    }

    const previewReport = await nativeInvoke<Report>(page, "storage_inventory", { scanId: randomUUID(), preview: true });
    await writeFile(info.outputPath("reclaim-preview.json"), JSON.stringify(previewReport, null, 2));
    expect(previewReport.referenceError).toBeNull();
    const preview = previewReport.reclaimPreview!;
    expect(preview.recoveryDirectory).toBe(join(dataDirectory, "storage-recovery"));
    expect(typeof preview.recoveryFreeBytes).toBe("number");
    expect(preview.cacheItems.map(row => row.item.id)).toEqual(["web-cache", "code-cache", "gpu-cache"]);
    for (const row of preview.cacheItems) {
      expect(row.assessment).toBe("unverified");
      expect(row.item.location).toBeNull();
      expect(row.item.size).toBeNull();
    }
    expect(previewReport.items.find(row => row.id === "history")?.references)
      .toContainEqual({ label: "知识快照中的版本引用", count: 1 });

    await inventory.getByRole("button", { name: "预览可回收内容", exact: true }).click();
    const panel = inventory.getByRole("region", { name: "回收预览", exact: true });
    await expect(panel.getByText("未核实，受保护", { exact: true })).toHaveCount(3);
    await panel.getByText("查看受保护的数据与引用", { exact: true }).click();
    await panel.locator("summary").filter({ hasText: "知识版本与历史记录" }).click();
    await expect(panel.getByText("知识快照中的版本引用", { exact: true })).toBeVisible();
    for (const [width, height, theme] of [[1366, 768, "light"], [1600, 900, "light"], [1920, 1080, "dark"]] as const) {
      await page.setViewportSize({ width, height });
      await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
      await panel.getByRole("heading", { name: "回收预览" }).scrollIntoViewIfNeeded();
      expect(await panel.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
      await page.screenshot({ path: info.outputPath(`reclaim-${width}-${theme}.png`) });
    }
    await expect(stat(join(dataDirectory, "storage-recovery"))).rejects.toHaveProperty("code", "ENOENT");
    expect(await Promise.all([readFile(db), readFile(wal), readFile(backup)])).toEqual(original);
    expect(await readFile(sourcePath, "utf8")).toEqual(source);
  } finally {
    await app.stop();
  }
});
