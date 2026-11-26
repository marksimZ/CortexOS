import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative } from "./native";

test("one-click web URL import saves readable text to knowledge and mounts it to chat context", async ({}, info) => {
  test.setTimeout(90_000);
  const webServer = createServer((req, res) => {
    if (req.url === "/quantum-notes") {
      res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      res.end(
        "<!doctype html><html><head><title>量子计算入门笔记</title></head><body><article><h1>量子比特</h1><p>量子比特可以同时处于 0 与 1 的叠加态。</p></article><script>window.__xss = 1;</script></body></html>",
      );
      return;
    }
    res.writeHead(404);
    res.end("not found");
  });
  await new Promise<void>((done) => webServer.listen(0, "127.0.0.1", done));
  const address = webServer.address();
  if (!address || typeof address === "string") throw Error("web fixture address unavailable");
  const articleUrl = `http://127.0.0.1:${address.port}/quantum-notes`;

  const app = await launchNative(info, { webPageUrl: articleUrl });
  try {
    const { page } = app;
    await expect(page.getByRole("button", { name: "新建工作区", exact: true })).toBeVisible();
    const skipNaming = page.getByRole("button", { name: "暂不起名", exact: true });
    await skipNaming.waitFor({ state: "visible", timeout: 2000 }).catch(() => {});
    if (await skipNaming.isVisible()) {
      await skipNaming.click();
      await expect(skipNaming).not.toBeVisible();
    }

    await createWorkspace(page, "网页入库验收");

    const importButton = page.getByRole("button", { name: "导入网页链接", exact: true });
    await importButton.click();
    const popover = page.getByRole("dialog", { name: "导入网页正文并挂载", exact: true });
    await expect(popover).toBeVisible();

    await popover.getByPlaceholder("粘贴公开网页链接（https://...）").fill(articleUrl);
    await page.screenshot({ path: info.outputPath("web-import-popover.png"), animations: "disabled" });

    await popover.getByRole("button", { name: "抓取并挂载", exact: true }).click();
    await expect(page.getByText("已抓取网页《量子计算入门笔记》并挂载为本轮上下文")).toBeVisible();
    await expect(popover).not.toBeVisible();
    await expect(page.getByText("手动选择: 1 项知识").or(page.getByText("手动选择：1 项知识")).first()).toBeVisible();

    await page.screenshot({ path: info.outputPath("web-import-mounted.png"), animations: "disabled" });
  } finally {
    await app.stop();
    webServer.closeAllConnections();
    webServer.close();
  }
});
