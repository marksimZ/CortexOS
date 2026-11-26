import { expect, test } from "@playwright/test";
import { writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

test("page/section chunking cites matching section and jumps directly to source page line", async ({}, info) => {
  test.setTimeout(90_000);
  const docPath = info.outputPath("courseware.md");
  const filler = "这里是一段用于测试分页分块检索的长背景描述材料，保证触发分段索引。".repeat(22);
  writeFileSync(
    docPath,
    [
      "## 第 1 页：绪论与背景",
      filler,
      "",
      "## 第 2 页：核心推进原理",
      "核心关键在于火星热核推进公式：I_sp = 920s，用于深空快速转移轨道设计。",
      filler,
      "",
      "## 第 3 页：实验总结",
      filler,
    ].join("\n"),
    "utf8",
  );

  const fixture = createServer((req, res) => {
    if (req.url === "/api/chat") {
      req.resume();
      req.on("end", () => {
        res.writeHead(200, { "content-type": "application/x-ndjson" });
        res.end(
          JSON.stringify({
            message: { role: "assistant", content: "根据课件第 2 页，比冲为 920s。" },
            done: true,
          }) + "\n",
        );
      });
      return;
    }
    res.setHeader("content-type", "application/json");
    if (req.url === "/api/version") res.end('{"version":"citation-fixture"}');
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

    await createWorkspace(page, "页码溯源验收");
    const workspaceId = await page.getByLabel("当前工作区", { exact: true }).inputValue();

    const imported = await nativeInvoke<{ documents: { nodeId: string }[] }>(
      page,
      "import_dropped_files",
      { workspaceId, paths: [docPath] },
    );
    expect(imported.documents).toHaveLength(1);

    await page.getByRole("textbox", { name: "你的问题" }).fill("请查找火星热核推进公式的数值");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect(page.getByText("根据课件第 2 页，比冲为 920s。", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "打开回答依据", exact: true }).click();
    const evidence = page.locator("#chat-evidence");
    await expect(evidence).toBeVisible();
    const refButton = evidence.locator(".evidence-reference", { hasText: "第 2 页" }).first();
    await expect(refButton).toBeVisible();
    await refButton.click();

    await expect(evidence.locator(".evidence-section-badge")).toContainText("第 2 页");
    await page.screenshot({ path: info.outputPath("evidence-page-badge.png"), animations: "disabled" });

    await evidence.getByRole("button", { name: "定位原文页码" }).click();
    const sourcePre = page.locator(".numbered-source");
    await expect(sourcePre).toBeVisible();
    await expect(sourcePre.locator(".selected-source-line").first()).toBeVisible();
    await expect(sourcePre).toContainText("火星热核推进公式");
    await page.screenshot({ path: info.outputPath("source-page-located.png"), animations: "disabled" });
  } finally {
    await app.stop();
    fixture.closeAllConnections();
    fixture.close();
  }
});
