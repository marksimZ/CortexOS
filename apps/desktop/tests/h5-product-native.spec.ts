import { expect, test, type Page } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

async function fixedOllama() {
  const calls = { chat: 0, offline: false, hold: false, fail: false };
  const server = createServer(async (request, response) => {
    const json = (value: unknown, status = 200) => {
      response.writeHead(status, { "Content-Type": "application/json" });
      response.end(JSON.stringify(value));
    };
    if (request.url === "/api/version") return json({ version: "h5-fixture" }, calls.offline ? 503 : 200);
    if (request.url === "/api/tags") return json({ models: [{ name: "qwen3:4b-instruct", size: 1 }] });
    if (request.url === "/api/show") return json({ details: { format: "gguf" } });
    if (request.url !== "/api/chat") return json({}, 404);
    for await (const chunk of request) void chunk;
    calls.chat++;
    if (calls.fail) return json({ error: "private-fixture-reply" }, 503);
    response.writeHead(200, { "Content-Type": "application/x-ndjson" });
    const frame = JSON.stringify({ message: { role: "assistant", content: calls.hold ? "本批固定回复的片段" : "H5 固定回复已送达。" }, done: !calls.hold }) + "\n";
    if (calls.hold) response.write(frame);
    else response.end(frame);
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Missing fixture port");
  return {
    endpoint: `http://127.0.0.1:${address.port}`, calls,
    close: () => { server.closeAllConnections(); server.close(); },
  };
}

async function guidedWorkspace(page: Page, option: string, name: string) {
  await page.getByRole("button", { name: option, exact: true }).click();
  const form = page.getByRole("dialog", { name: "创建工作区" });
  await form.getByLabel("工作区名称").fill(name);
  await form.getByRole("button", { name: "创建工作区", exact: true }).click();
  await expect(form).not.toBeVisible();
}

test("H5 local welcome, help, live stop and scoped chat history in WebView2", async ({}, info) => {
  test.setTimeout(70_000);
  const fixture = await fixedOllama();
  const app = await launchNative(info, { endpoint: fixture.endpoint });
  try {
    const { page } = app;
    await page.setViewportSize({ width: 1366, height: 768 });
    await test.step("首次使用显示只读检查结果，并直接打开恢复与模型设置", async () => {
      await expect(page.getByRole("region", { name: "首次使用" })).toBeVisible();
      await expect(page.getByText("本机模型已就绪", { exact: true })).toBeVisible();
      await page.screenshot({ path: info.outputPath("welcome-1366.png") });
      expect(fixture.calls.chat).toBe(0);
      await page.getByRole("link", { name: "从备份恢复", exact: true }).click();
      await expect(page.getByRole("button", { name: "本地数据", exact: true })).toHaveAttribute("aria-pressed", "true");
      await page.goBack();
      await page.getByRole("link", { name: "查看本机连接", exact: true }).click();
      await expect(page.getByRole("button", { name: "模型连接", exact: true })).toHaveAttribute("aria-pressed", "true");
      await page.goBack();
    });
    await test.step("创建工作区和发送本机对话，不需要云端设置", async () => {
      await guidedWorkspace(page, "用本机模型", "H5 本机对话");
      await page.getByRole("textbox", { name: "你的问题", exact: true }).fill("用固定回复验证普通对话。");
      await page.getByRole("button", { name: "发送消息", exact: true }).click();
      await expect(page.getByText("H5 固定回复已送达。", { exact: true })).toBeVisible();
      await expect(page.getByText("已完成", { exact: true })).toBeVisible();
    });
    const openTasks = page.getByRole("button", { name: "任务中心", exact: true });
    const tasks = page.getByRole("dialog", { name: "任务中心", exact: true });
    await test.step("任务中心停止执行，保留记录并回到原会话", async () => {
      fixture.calls.hold = true;
      await page.getByRole("textbox", { name: "你的问题", exact: true }).fill("测试任务中心停止。");
      await page.getByRole("button", { name: "发送消息", exact: true }).click();
      await expect.poll(() => fixture.calls.chat).toBe(2);
      await openTasks.click();
      await expect(tasks.getByText("进行中", { exact: true })).toBeVisible();
      await expect(page.getByRole("button", { name: "关闭窗口", exact: true })).toBeVisible();
      await page.screenshot({ path: info.outputPath("live-task-1366.png") });
      await tasks.getByRole("button", { name: "停止任务", exact: true }).click();
      await expect(tasks.getByText("已取消", { exact: true })).toBeVisible();
      await expect(tasks.getByText("当前没有等待或运行中的任务。", { exact: true })).toBeVisible();
      const completed = tasks.getByRole("listitem").filter({ hasText: "已完成" });
      await completed.getByRole("link", { name: "打开原对话", exact: true }).click();
      await expect(tasks).not.toBeVisible();
      await expect(page).toHaveURL(/#\/chat\?.*message=/);
      await expect(page.getByText("H5 固定回复已送达。", { exact: true })).toBeVisible();
    });
    await test.step("回到失败的回答并从原入口重试", async () => {
      fixture.calls.hold = false;
      fixture.calls.fail = true;
      await page.getByRole("textbox", { name: "你的问题", exact: true }).fill("测试失败处理。");
      await page.getByRole("button", { name: "发送消息", exact: true }).click();
      await expect.poll(() => fixture.calls.chat).toBe(3);
      await openTasks.click();
      const failed = tasks.getByRole("listitem").filter({ hasText: "失败" });
      await expect(failed).toContainText("HTTP 503");
      await expect(failed).not.toContainText("private-fixture-reply");
      await failed.getByRole("link", { name: "打开原对话", exact: true }).click();
      fixture.calls.fail = false;
      const messageId = new URLSearchParams(new URL(page.url()).hash.split("?")[1]).get("message");
      await page.locator(`[id="message-${messageId}"]`).getByRole("button", { name: "重试", exact: true }).click();
      await expect.poll(() => fixture.calls.chat).toBe(4);
      await expect(page.getByText("H5 固定回复已送达。", { exact: true })).toHaveCount(2);
    });
    await test.step("另一工作区只显示本区记录，帮助按当前页面排列", async () => {
      const first = await page.getByLabel("当前工作区", { exact: true }).inputValue();
      const records = await nativeInvoke<unknown[]>(page, "read_task_history", { workspaceId: first, activeId: null });
      expect(records).toHaveLength(4);
      await createWorkspace(page, "H5 独立工作区");
      await openTasks.click();
      await expect(tasks.getByText("当前工作区还没有记录。", { exact: true })).toBeVisible();
      expect(await tasks.getByRole("link", { name: "打开原对话" }).count()).toBe(0);
      await page.keyboard.press("Escape");
      await expect(openTasks).toBeFocused();
      await page.getByRole("navigation", { name: "主导航", exact: true }).getByRole("link", { name: "知识", exact: true }).click();
      await page.getByRole("button", { name: "使用帮助", exact: true }).click();
      const help = page.getByRole("dialog", { name: "使用帮助", exact: true });
      await expect(help.locator("summary").first()).toHaveText("知识查找与索引");
      await help.getByText("知识查找与索引", { exact: true }).click();
      await expect(help.getByRole("link", { name: "查看索引状态" })).toBeVisible();
      await page.getByRole("combobox", { name: "外观", exact: true }).selectOption("dark");
      await page.setViewportSize({ width: 900, height: 640 });
      await page.screenshot({ path: info.outputPath("help-dark-900.png") });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    });
    expect(app.consoleErrors).toEqual([]);
  } finally {
    await app.stop();
    fixture.close();
  }
});

test("H5 unavailable Ollama allows skipping and cloud setup leaves the personalized gate intact", async ({}, info) => {
  test.setTimeout(65_000);
  const fixture = await fixedOllama();
  fixture.calls.offline = true;
  const app = await launchNative(info, { endpoint: fixture.endpoint });
  try {
    const { page } = app;
    await expect(page.getByRole("region", { name: "首次使用" }).getByText("Ollama 未连接")).toBeVisible();
    await page.getByRole("button", { name: "跳过引导", exact: true }).click();
    await page.reload();
    await expect(page.getByRole("region", { name: "首次使用" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "创建工作区", exact: true })).toBeEnabled();
    await page.evaluate(() => localStorage.removeItem("cortexos.welcome-skipped"));
    await page.reload();
    await guidedWorkspace(page, "连接云端模型", "H5 云端引导");
    await expect(page.getByRole("button", { name: "云端模型", exact: true })).toHaveAttribute("aria-pressed", "true");
    const workspaceId = await page.getByLabel("当前工作区", { exact: true }).inputValue();
    await page.evaluate(id => { location.hash = `/benchmark?view=personalized&workspace=${id}&job=unconfigured`; }, workspaceId);
    await expect(page.getByRole("heading", { name: "为你的任务设计评测" })).toBeVisible();
    await expect(page.getByText("尚未连接", { exact: true })).toBeVisible();
    expect(fixture.calls.chat).toBe(0);
    expect(app.consoleErrors).toEqual([]);
  } finally {
    await app.stop();
    fixture.close();
  }
});
