import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative, nativeInvoke } from "./native";
import { errorText, parseFrontendError } from "../src/frontend-error";

test("H1 browser reports explicit errors and recovers from a render failure", async ({ page }, info) => {
  test.skip(info.project.name !== "edge", "Browser fixture");
  const failure = { code: "write_conflict", userMessage: "无法更新记录", publicDetails: "Authorization: Bearer fixture-secret-private", retryable: false, persistence: "unsaved", raw: "private content" };
  expect(parseFrontendError(failure)).toMatchObject({ code: "write_conflict", retryable: false, persistence: "unsaved" });
  expect(errorText(failure)).not.toMatch(/fixture-secret-private|private content/);
  expect(errorText({})).not.toContain("已保存");
  await page.addInitScript(() => {
    const state = { invalid: true };
    const settings = { model: "fixture", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 };
    Object.assign(window, { state, isTauri: true, __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} }, __TAURI_INTERNALS__: {
      metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback() {},
      invoke: async (command: string) => {
        if (command === "bootstrap") return { settings, workspaces: state.invalid ? null : [{ name: "恢复检查", id: "fixture" }], activeRun: null };
        if (command === "check_ollama") return { version: "fixture", models: [] };
        if (command === "list_conversations") return [];
        if (command.startsWith("plugin:")) return 1;
        return null;
      },
    } });
  });
  await page.goto("/#/chat");
  await expect(page.getByText("CortexOS 界面无法显示", { exact: true })).toBeVisible();
  await expect(page.getByText(/保存成功/)).toHaveCount(0);
  await page.evaluate(() => { (window as unknown as { state: { invalid: boolean } }).state.invalid = false; });
  await page.getByRole("button", { name: "重试显示", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "你的问题", exact: true })).toBeEnabled();
});

test("H1 native draft isolation, keyboard, dialog close, form validation and restart", async ({}, info) => {
  test.skip(info.project.name !== "desktop", "Isolated native test");
  test.setTimeout(85_000);
  let requests = 0;
  const server = createServer(async (request, response) => {
    for await (const chunk of request) void chunk;
    const reply = (value: unknown) => response.end(JSON.stringify(value));
    response.setHeader("Content-Type", "application/json");
    if (request.url === "/api/version") return reply({ version: "h1-correctness" });
    if (request.url === "/api/tags") return reply({ models: [{ name: "qwen3:4b-instruct", details: { format: "gguf" } }] });
    if (request.url === "/api/ps") return reply({ models: [] });
    if (request.url === "/api/show") return reply({ details: { format: "gguf" }, capabilities: ["completion"] });
    if (request.url !== "/api/chat") { response.statusCode = 404; return response.end(); }
    requests++;
    response.setHeader("Content-Type", "application/x-ndjson");
    response.write(JSON.stringify({ message: { role: "assistant", content: "受控测试回答。" }, done: false }) + "\n");
    const timer = setTimeout(() => response.end(JSON.stringify({ message: { role: "assistant", content: "" }, done: true, done_reason: "stop" }) + "\n"), 700);
    response.once("close", () => clearTimeout(timer));
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Fixture not listening");
  const endpoint = `http://127.0.0.1:${address.port}`;
  let app = await launchNative(info, { endpoint });
  try {
    let { page } = app;
    await page.setViewportSize({ width: 1366, height: 768 });
    await createWorkspace(page, "草稿工作区一");
    const workspace = page.getByLabel("当前工作区", { exact: true });
    const first = await workspace.inputValue();
    const nav = (label: string) => page.getByRole("navigation", { name: "主导航", exact: true }).getByRole("link", { name: label, exact: true });
    const question = () => page.getByRole("textbox", { name: "你的问题", exact: true });
    await question().fill("一号草稿");
    await expect(page.getByText("草稿保存在本机", { exact: true })).toBeVisible();
    await createWorkspace(page, "草稿工作区二");
    await expect(question()).toHaveValue("");
    await question().fill("二号草稿");
    await workspace.selectOption(first);
    await expect(question()).toHaveValue("一号草稿");
    await nav("设置").click();
    await page.getByText("生成参数", { exact: true }).click();
    const max = page.getByLabel("回答长度上限（Token）", { exact: true });
    const total = page.getByLabel("输入与回答总容量（Token）", { exact: true });
    const save = page.getByRole("button", { name: "保存设置", exact: true });
    await max.fill(""); await save.click();
    await expect(max).toHaveAttribute("aria-invalid", "true");
    await expect(page.getByText("请输入数字。", { exact: true })).toBeVisible();
    await max.fill("1024"); await total.fill("1024"); await save.click();
    await expect(total).toBeFocused();
    await expect(page.getByText("总容量必须比回答上限多出至少 513 Token。", { exact: true })).toBeVisible();
    await total.fill("4096"); await max.fill("512");
    await nav("对话").click();
    await expect(question()).toHaveValue("一号草稿");
    await question().press("Enter");
    await expect(question()).toBeFocused();
    await expect(page.getByRole("button", { name: "停止生成", exact: true })).toBeVisible();
    await question().press("Enter");
    await expect(page.getByText("已完成", { exact: true })).toBeVisible();
    expect(requests).toBe(1);
    await expect(question()).toBeFocused();
    await expect(page.getByRole("region", { name: "对话消息", exact: true })).toHaveAttribute("tabindex", "0");
    const appearance = page.getByRole("combobox", { name: "外观", exact: true });
    await appearance.selectOption("light");
    await appearance.focus();
    const supportsPicker = await appearance.evaluate(e => getComputedStyle(e).appearance === "base-select");
    expect(supportsPicker).toBe(true);
    await page.keyboard.press("Space");
    await expect(appearance).toHaveJSProperty("value", "light");
    await expect.poll(() => appearance.evaluate(element => element.matches(":open"))).toBe(true);
    await page.keyboard.press("Escape");
    await expect.poll(() => appearance.evaluate(element => element.matches(":open"))).toBe(false);
    await expect(appearance).toBeFocused();
    await nav("知识").click();
    const launch = page.getByRole("button", { name: "新建知识", exact: true });
    await launch.click();
    let editor = page.getByRole("dialog", { name: "新建知识草稿", exact: true });
    const title = () => editor.getByLabel("标题", { exact: true });
    await expect(title()).toBeFocused();
    await expect(page.locator("#main-content")).toHaveJSProperty("inert", true);
    await title().fill("等待恢复的本机知识");
    await page.getByRole("button", { name: "关闭窗口", exact: true }).click();
    const guard = page.getByRole("dialog", { name: "保留未保存的修改？", exact: true });
    await expect(guard).toHaveCount(1);
    await guard.getByRole("button", { name: "继续编辑", exact: true }).click();
    await expect(guard).not.toBeVisible();
    await expect(title()).toBeFocused();
    await page.getByRole("button", { name: "关闭窗口", exact: true }).click();
    await guard.getByRole("button", { name: "保留本机草稿", exact: true }).click();
    await expect.poll(() => page.isClosed()).toBe(true);
    await app.stop();
    app = await launchNative(info, { endpoint });
    page = app.page;
    await page.setViewportSize({ width: 1366, height: 768 });
    await nav("知识").click();
    await page.getByRole("button", { name: "新建知识", exact: true }).click();
    editor = page.getByRole("dialog", { name: "新建知识草稿", exact: true });
    await expect(title()).toHaveValue("等待恢复的本机知识");
    const official = await nativeInvoke<unknown[]>(page, "list_knowledge", { workspaceId: first });
    expect(official).toHaveLength(0);
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "放弃修改", exact: true }).click();
    await expect(editor).not.toBeVisible();
    await page.getByRole("button", { name: "新建知识", exact: true }).click();
    await expect(title()).toHaveValue("");
  } finally {
    await app.stop();
    server.closeAllConnections(); server.close();
  }
});
