import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

test("streaming thinking frames render live/collapsed separately from content and persist cleanly", async ({}, info) => {
  test.setTimeout(90_000);
  const fixture = createServer((req, res) => {
    if (req.url === "/api/chat") {
      req.resume();
      req.on("end", () => {
        res.writeHead(200, { "content-type": "application/x-ndjson" });
        res.write(
          JSON.stringify({
            message: { role: "assistant", content: "", thinking: "第一步：拆解问题核心。" },
            done: false,
          }) + "\n",
        );
        setTimeout(() => {
          res.write(
            JSON.stringify({
              message: { role: "assistant", content: "", thinking: "第二步：整理结构化结论。" },
              done: false,
            }) + "\n",
          );
          setTimeout(() => {
            res.end(
              JSON.stringify({
                message: { role: "assistant", content: "这是最终正式回答。", thinking: "" },
                done: true,
              }) + "\n",
            );
          }, 180);
        }, 180);
      });
      return;
    }
    res.setHeader("content-type", "application/json");
    if (req.url === "/api/version") res.end('{"version":"thinking-fixture"}');
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

    await createWorkspace(page, "思考流验收");
    const workspaceId = await page.getByLabel("当前工作区", { exact: true }).inputValue();

    await page.getByRole("textbox", { name: "你的问题" }).fill("演示深度思考流程");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();

    await expect(page.getByText("这是最终正式回答。", { exact: true })).toBeVisible();
    const toggle = page.locator(".thinking-toggle");
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(toggle).toContainText("已深度思考");
    await expect(toggle).toContainText("模型输出思考文本");
    await expect(page.locator(".thinking-body")).toHaveCount(0);
    await expect(page.locator(".message.assistant .message-content")).not.toContainText("第一步：拆解问题核心。");

    await page.screenshot({ path: info.outputPath("thinking-collapsed.png"), animations: "disabled" });

    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    const body = page.locator(".thinking-body");
    await expect(body).toBeVisible();
    await expect(body).toContainText("第一步：拆解问题核心。第二步：整理结构化结论。");

    await page.screenshot({ path: info.outputPath("thinking-expanded.png"), animations: "disabled" });

    const conversations = await nativeInvoke<{ id: string }[]>(page, "list_conversations", { workspaceId });
    expect(conversations.length).toBeGreaterThan(0);
    const messages = await nativeInvoke<Record<string, unknown>[]>(page, "load_messages", {
      workspaceId,
      conversationId: conversations[0].id,
    });
    const assistant = messages.find((item) => item.role === "assistant");
    expect(assistant).toMatchObject({
      role: "assistant",
      content: "这是最终正式回答。",
      thinking: "第一步：拆解问题核心。第二步：整理结构化结论。",
    });
  } finally {
    await app.stop();
    fixture.closeAllConnections();
    fixture.close();
  }
});
