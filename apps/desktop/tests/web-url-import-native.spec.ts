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

  const ollamaFixture = createServer((req, res) => {
    if (req.url === "/api/chat") {
      req.resume();
      req.on("end", () => {
        res.writeHead(200, { "content-type": "application/x-ndjson" });
        res.end(
          JSON.stringify({
            message: { role: "assistant", content: "量子比特可以同时处于 0 与 1 的叠加态。" },
            done: true,
          }) + "\n",
        );
      });
      return;
    }
    res.setHeader("content-type", "application/json");
    if (req.url === "/api/version") res.end('{"version":"web-mount-fixture"}');
    else if (req.url === "/api/show") res.end('{"details":{"format":"gguf"}}');
    else if (req.url === "/api/tags") res.end('{"models":[{"name":"qwen3:4b-instruct","size":1}]}');
    else res.end("{}");
  });
  await new Promise<void>((done) => ollamaFixture.listen(0, "127.0.0.1", done));
  const ollamaAddress = ollamaFixture.address();
  if (!ollamaAddress || typeof ollamaAddress === "string") throw Error("ollama fixture address unavailable");

  const app = await launchNative(info, {
    endpoint: `http://127.0.0.1:${ollamaAddress.port}`,
    webPageUrl: articleUrl,
  });
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

    await page.getByRole("textbox", { name: "你的问题" }).fill("量子比特有什么特点？");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.getByText("量子比特可以同时处于 0 与 1 的叠加态。", { exact: true })).toBeVisible();
  } finally {
    await app.stop();
    ollamaFixture.closeAllConnections();
    ollamaFixture.close();
    webServer.closeAllConnections();
    webServer.close();
  }
});
