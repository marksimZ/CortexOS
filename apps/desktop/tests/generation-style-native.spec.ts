import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

interface CapturedChatBody {
  options?: {
    temperature?: number;
    top_p?: number;
    num_predict?: number;
    num_ctx?: number;
  };
  think?: boolean;
}

test("generation style presets, deep thinking, and advanced tuning sync across composer, settings, and Ollama payload", async ({}, info) => {
  test.setTimeout(90_000);
  const chatBodies: CapturedChatBody[] = [];
  const fixture = createServer(async (req, res) => {
    if (req.url === "/api/chat") {
      const chunks: Buffer[] = [];
      for await (const chunk of req) chunks.push(chunk);
      chatBodies.push(JSON.parse(Buffer.concat(chunks).toString("utf8")) as CapturedChatBody);
      res.writeHead(200, { "content-type": "application/x-ndjson" });
      res.end(
        JSON.stringify({
          message: { role: "assistant", content: "参数校验回复 " + chatBodies.length },
          done: true,
        }) + "\n",
      );
      return;
    }
    res.setHeader("content-type", "application/json");
    if (req.url === "/api/version") res.end('{"version":"style-fixture"}');
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

    await createWorkspace(page, "参数验收");

    const styleButton = page.getByRole("button", { name: "回答风格与生成参数", exact: true });
    await styleButton.click();
    const popover = page.getByRole("dialog", { name: "回答风格与生成参数设置", exact: true });
    await expect(popover).toBeVisible();
    await page.screenshot({ path: info.outputPath("composer-style-popover.png"), animations: "disabled" });

    const creativeButton = popover.getByRole("button", { name: /灵感发散/ });
    await creativeButton.click();
    await expect(creativeButton).toHaveAttribute("aria-pressed", "true");
    await expect(creativeButton).toBeEnabled();

    const popoverThinking = popover.getByRole("checkbox");
    await popoverThinking.check();
    await expect(popoverThinking).toBeChecked();
    await expect(popoverThinking).toBeEnabled();

    await page.getByRole("textbox", { name: "你的问题" }).fill("测试发散与深度思考");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(page.getByText("参数校验回复 1", { exact: true })).toBeVisible();
    expect(chatBodies).toHaveLength(1);
    expect(chatBodies[0].options?.temperature).toBeCloseTo(1.1, 4);
    expect(chatBodies[0].options?.top_p).toBeCloseTo(0.95, 4);
    expect(chatBodies[0].think).toBe(true);

    await styleButton.click();
    await expect(popover).toBeVisible();
    const preciseButton = popover.getByRole("button", { name: /严谨准确/ });
    await preciseButton.click();
    await expect(preciseButton).toHaveAttribute("aria-pressed", "true");
    await expect(preciseButton).toBeEnabled();
    await popoverThinking.uncheck();
    await expect(popoverThinking).not.toBeChecked();
    await expect(popoverThinking).toBeEnabled();

    await page.keyboard.press("Escape");
    await expect(popover).not.toBeVisible();

    await page.getByRole("textbox", { name: "你的问题" }).fill("测试严谨模式");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(page.getByText("参数校验回复 2", { exact: true })).toBeVisible();
    expect(chatBodies).toHaveLength(2);
    expect(chatBodies[1].options?.temperature).toBeCloseTo(0.2, 4);
    expect(chatBodies[1].options?.top_p).toBeCloseTo(0.85, 4);
    expect("think" in chatBodies[1]).toBe(false);

    await page.getByRole("link", { name: "设置", exact: true }).click();
    await page.getByRole("button", { name: "我的助手", exact: true }).click();
    const assistantSection = page.locator("#assistant-section");
    await expect(assistantSection.getByRole("button", { name: /严谨准确/ })).toHaveAttribute("aria-pressed", "true");

    const styleCard = assistantSection.locator(".generation-style-panel");
    await styleCard.locator("details.generation-advanced > summary").click();
    await styleCard.getByLabel("回答长度上限（Token）").fill("768");
    await styleCard.getByRole("button", { name: "保存高级参数", exact: true }).click();

    await expect
      .poll(async () => (await nativeInvoke<{ settings: Record<string, unknown> }>(page, "bootstrap")).settings)
      .toMatchObject({
        temperature: 0.2,
        topP: 0.85,
        numPredict: 768,
        deepThinking: false,
      });
    await styleCard.scrollIntoViewIfNeeded();
    await page.screenshot({ path: info.outputPath("settings-assistant-style.png"), animations: "disabled" });

    await page.getByRole("button", { name: "模型连接", exact: true }).click();
    const modelSection = page.locator("#model-section");
    const modelAdvanced = modelSection.locator("details.advanced-settings").first();
    await modelAdvanced.locator("> summary").click();
    await expect(modelAdvanced.getByLabel("核采样（top_p）")).toHaveValue("0.85");
    await expect(modelAdvanced.getByRole("checkbox")).not.toBeChecked();
    await expect(modelSection.getByRole("button", { name: "保存设置", exact: true })).toBeDisabled();
    await modelAdvanced.scrollIntoViewIfNeeded();
    await page.screenshot({ path: info.outputPath("settings-model-advanced.png"), animations: "disabled" });
  } finally {
    await app.stop();
    fixture.closeAllConnections();
    fixture.close();
  }
});
