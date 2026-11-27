import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative } from "./native";

test("cross-conversation full-text search finds matching turn and jumps to located message", async ({}, info) => {
  test.setTimeout(90_000);
  let seq = 0;
  const fixture = createServer((req, res) => {
    if (req.url === "/api/chat") {
      req.resume();
      req.on("end", () => {
        seq += 1;
        res.writeHead(200, { "content-type": "application/x-ndjson" });
        res.end(
          JSON.stringify({
            message: { role: "assistant", content: `收到第 ${seq} 轮内容` },
            done: true,
          }) + "\n",
        );
      });
      return;
    }
    res.setHeader("content-type", "application/json");
    if (req.url === "/api/version") res.end('{"version":"search-fixture"}');
    else if (req.url === "/api/show") res.end('{"details":{"format":"gguf"}}');
    else if (req.url === "/api/tags") res.end('{"models":[{"name":"qwen3:4b-instruct","size":1}]}');
    else res.end("{}");
  });
  await new Promise<void>((done) => fixture.listen(0, "127.0.0.1", done));
  const address = fixture.address();
  if (!address || typeof address === "string") throw Error("fixture address unavailable");
  const app = await launchNative(info, { endpoint: "http://127.0.0.1:" + address.port });
  try {
    const { page } = app;
    await expect(page.getByRole("button", { name: "新建工作区", exact: true })).toBeVisible();
    const skipNaming = page.getByRole("button", { name: "暂不起名", exact: true });
    await skipNaming.waitFor({ state: "visible", timeout: 2000 }).catch(() => {});
    if (await skipNaming.isVisible()) {
      await skipNaming.click();
      await expect(skipNaming).not.toBeVisible();
    }

    await createWorkspace(page, "跨会话搜索验收");

    await page.getByRole("textbox", { name: "你的问题" }).fill("请核对火星基地定价方案细节");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(page.getByText("收到第 1 轮内容", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "新建对话", exact: true }).first().click();
    await expect(page.getByRole("article")).toHaveCount(0);
    await page.getByRole("textbox", { name: "你的问题" }).fill("周末郊游清单安排");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(page.getByText("收到第 2 轮内容", { exact: true })).toBeVisible();

    const searchBox = page.getByLabel("搜索历史对话");
    await searchBox.fill("定价方案");

    const resultsRegion = page.getByRole("region", { name: "对话搜索结果" });
    const hit = resultsRegion.locator(".conversation-search-hit").first();
    await expect(hit).toBeVisible();
    await expect(hit).toContainText("火星基地定价方案");
    await page.screenshot({ path: info.outputPath("chat-search-results.png"), animations: "disabled" });

    await hit.click();
    const located = page.locator("article.message[data-located='true']");
    await expect(located).toBeVisible();
    await expect(located).toContainText("火星基地定价方案");
    await page.screenshot({ path: info.outputPath("chat-search-located.png"), animations: "disabled" });
  } finally {
    await app.stop();
    fixture.closeAllConnections();
    fixture.close();
  }
});
