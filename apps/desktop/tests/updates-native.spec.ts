import { execFileSync } from "node:child_process";
import { once } from "node:events";
import { copyFile, mkdir, mkdtemp, readFile, unlink } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { launchNative, nativeInvoke } from "./native";

test("Windows updater checks channels, rejects altered packages and verifies a signed RC", async ({}, info) => {
  const candidate = process.env.CORTEXOS_H4_PACKAGE;
  test.skip(!candidate, "Requires a built candidate installer");
  if (!candidate) return;
  test.setTimeout(90_000);

  const keyDirectory = await mkdtemp(join(tmpdir(), "cortexos-h4-updater-"));
  const keyFile = join(keyDirectory, "rehearsal.key");
  const installer = info.outputPath("signed-rc.exe");
  const version = /^CortexOS_(\d+\.\d+\.\d+-rc\.\d+)_x64-setup\.exe$/.exec(basename(candidate))?.[1];
  if (!version) throw new Error("Use a versioned CortexOS Windows candidate installer");
  const cli = resolve("node_modules/@tauri-apps/cli/tauri.js");
  const localEnv = { ...process.env };
  delete localEnv.TAURI_SIGNING_PRIVATE_KEY;
  delete localEnv.TAURI_SIGNING_PRIVATE_KEY_PATH;
  delete localEnv.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;
  const signer = (...args: string[]) =>
    execFileSync(process.execPath, [cli, "signer", ...args], {
      stdio: "ignore",
      env: localEnv,
      windowsHide: true,
      timeout: 15_000,
    });

  await mkdir(info.outputDir, { recursive: true });
  await copyFile(candidate, installer);
  try {
    signer("generate", "--ci", "--write-keys", keyFile);
    signer("sign", "--private-key-path", keyFile, "--app-version", version, installer);
    process.env.CORTEXOS_TEST_UPDATE_PUBKEY = (await readFile(keyFile + ".pub", "utf8")).trim();
  } finally {
    await unlink(keyFile).catch((error: NodeJS.ErrnoException) => {
      if (error.code !== "ENOENT") throw error;
    });
  }
  const signature = (await readFile(installer + ".sig", "utf8")).trim();
  const good = await readFile(installer);
  const bad = Buffer.from(good);
  bad[bad.length - 1] ^= 1;
  let mode = "tampered";
  let origin = "";
  let downloads = 0;
  let checks = 0;
  const server = createServer((req, res) => {
    if (req.url === "/update.exe") {
      downloads++;
      if (mode === "unavailable") {
        res.writeHead(503).end();
        return;
      }
      if (mode === "oversize") {
        res.writeHead(200, { "Content-Length": 129 * 1024 * 1024 }).end("oversize");
        return;
      }
      res.writeHead(200, { "Content-Length": good.length });
      res.end(mode === "tampered" ? bad : good);
      return;
    }
    checks++;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({
      version: mode === "wrong-version" ? "0.2.2-rc.1" : mode === "same-version" ? "0.2.0+published" : version,
      notes: "隔离验收更新说明",
      url: mode === "other-origin" ? "http://127.0.0.1:1/update.exe" : `${origin}/update.exe`,
      signature,
    }));
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("No mock update server");
  origin = `http://127.0.0.1:${address.port}`;
  process.env.CORTEXOS_TEST_UPDATE_ORIGIN = origin;
  try {
    const app = await launchNative(info);
    try {
      const { page } = app;
      page.setDefaultTimeout(10_000);
      await page.getByRole("link", { name: "设置", exact: true }).click();
      await page.getByRole("button", { name: "关于", exact: true }).click();
      const panel = page.getByRole("region", { name: "应用更新" });
      const check = panel.getByRole("button", { name: "检查更新" });
      const download = panel.getByRole("button", { name: "下载更新" });
      const alert = panel.getByRole("alert");
      await expect(check).toBeVisible();
      expect(checks).toBe(0);
      await check.click();
      await expect(panel.getByRole("status")).toHaveText("当前渠道没有更新。");
      expect(downloads).toBe(0);
      await panel.getByRole("combobox", { name: "更新渠道" }).selectOption("preview");
      await check.click();
      await expect(panel.getByText(`可用版本 ${version}`)).toBeVisible();
      await panel.getByText("更新说明", { exact: true }).click();
      await expect(panel.getByText("隔离验收更新说明")).toBeVisible();
      await download.click();
      await expect(alert).toContainText("签名或版本校验失败");
      mode = "unavailable";
      await download.click();
      await expect(alert).toContainText("下载失败");
      mode = "oversize";
      await download.click();
      await expect(alert).toContainText("超过大小限制");
      mode = "wrong-version";
      await check.click();
      await expect(panel.getByText("可用版本 0.2.2-rc.1")).toBeVisible();
      await download.click();
      await expect(alert).toContainText("签名或版本校验失败");
      mode = "same-version";
      await check.click();
      await expect(panel.getByRole("status")).toHaveText("当前渠道没有更新。");
      mode = "other-origin";
      await check.click();
      await expect(alert).toContainText("不在该渠道内");
      expect(downloads).toBe(4);
      mode = "valid";
      await check.click();
      await download.click();
      await expect(panel.getByRole("status")).toHaveText("已校验更新包。");
      await expect(panel.getByRole("button", { name: /安装更新/ })).toHaveCount(0);
      await expect(nativeInvoke(page, "install_update", { id: "invalid" })).rejects.toThrow("安装版");
      expect(downloads).toBe(5);
      expect(app.consoleErrors).toEqual([]);
      await page.screenshot({ path: info.outputPath("updater-1440.png") });
      await page.setViewportSize({ width: 800, height: 600 });
      await panel.scrollIntoViewIfNeeded();
      await page.screenshot({ path: info.outputPath("updater-800.png") });
    } finally {
      await app.stop();
    }
  } finally {
    delete process.env.CORTEXOS_TEST_UPDATE_PUBKEY;
    delete process.env.CORTEXOS_TEST_UPDATE_ORIGIN;
    server.close();
  }
});
