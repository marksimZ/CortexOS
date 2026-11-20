import { expectPicker } from "./select-picker";
import { expect, test, type Page } from "@playwright/test";
import { createServer, type ServerResponse } from "node:http";
import { mkdir, writeFile } from "node:fs/promises";
import { launchNative, createWorkspace } from "./native";
test.use({ actionTimeout: 10000 });

async function splitChecks(page: Page, label: string, rootSelector: string, reopen: () => Promise<void>) {
  const root = page.locator(rootSelector);
  const divider = page.getByRole("separator", { name: label, exact: true });
  const initial = await divider.getAttribute("aria-valuenow");
  const box = (await divider.boundingBox())!, bounds = (await root.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + 80);
  await page.mouse.down();
  await page.mouse.move(bounds.x + bounds.width * 0.6, box.y + 80, { steps: 5 });
  await expect(page.locator(".pane-drag-shield")).toBeVisible();
  await page.keyboard.press("Escape");
  await page.mouse.up();
  await expect(page.locator(".pane-drag-shield")).toHaveCount(0);
  await expect(divider).toHaveAttribute("aria-valuenow", initial!);
  await divider.dblclick({ position: { x: 8, y: 80 } });
  await expect(divider).toHaveAttribute("aria-valuenow", "50");
  const ratio = await root.evaluate(element => {
    const gap = parseFloat(getComputedStyle(element).columnGap);
    const width = element.getBoundingClientRect().width;
    return element.firstElementChild!.getBoundingClientRect().width / (width - gap * width / (element as HTMLElement).offsetWidth);
  });
  expect(Math.abs(ratio - 0.5)).toBeLessThan(0.01);
  await divider.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(divider).toHaveAttribute("aria-valuenow", "48");
  await page.keyboard.press("Enter");
  const next = (await divider.boundingBox())!;
  await page.mouse.move(next.x + 8, next.y + 80);
  await page.mouse.down();
  await page.mouse.move(bounds.x + bounds.width * 0.9, next.y + 80, { steps: 6 });
  await page.mouse.up();
  await expect(divider).toHaveCount(0);
  await reopen();
  await expect(page.getByRole("separator", { name: label, exact: true })).toBeVisible();
}

test("native input cooldown, reading position, split cancellation and file extensions stay predictable", async ({}, info) => {
  test.setTimeout(100_000);
  await mkdir(info.outputPath("data"), { recursive: true });
  const name = "项目资料-版本与边界说明-用于核对很长的文件名称仍然保留扩展名.md";
  const source = info.outputPath("data", name);
  await writeFile(source, "# 合成资料\n文件预览与快照功能仍然可用。\n");
  let response: ServerResponse | undefined;
  let requests = 0;
  const frame = (content: string, done = false) => JSON.stringify({ message: { role: "assistant", content }, done, ...(done ? { done_reason: "stop" } : {}) }) + "\n";
  const server = createServer(async (req, res) => {
    res.setHeader("Content-Type", "application/json");
    if (req.url === "/api/version") return res.end(JSON.stringify({ version: "ui-micro-fixture" }));
    if (req.url === "/api/tags") return res.end(JSON.stringify({ models: [{ name: "qwen3:4b-instruct" }] }));
    if (req.url === "/api/show") return res.end(JSON.stringify({ details: { format: "gguf" } }));
    for await (const part of req) void part;
    requests++;
    response = res;
    res.setHeader("Content-Type", "application/x-ndjson");
    res.write(frame(Array.from({ length: 70 }, (_, i) => `第 ${i + 1} 段：这是滚动位置检查用的合成回答。\n\n`).join("")));
  });
  await new Promise<void>(done => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  if (!address || typeof address === "string") throw Error("No local test port");
  const app = await launchNative(info, { endpoint: `http://127.0.0.1:${address.port}`, sourcePath: source });
  const { page } = app;
  try {
    await createWorkspace(page, "交互微调验收");
    const input = page.getByRole("textbox", { name: "你的问题", exact: true });
    await input.fill("输入法选字检查");
    await input.dispatchEvent("compositionstart", { data: "选字" });
    await input.dispatchEvent("keydown", { key: "Enter", keyCode: 229, isComposing: true });
    expect(requests).toBe(0);
    await input.dispatchEvent("compositionend", { data: "选字" });
    await input.press("Enter");
    await page.waitForTimeout(200);
    await input.press("Enter");
    expect(requests).toBe(0);
    await input.press("Shift+Enter");
    await expect(input).toHaveValue("输入法选字检查\n");
    await page.waitForTimeout(550);
    await input.press("Enter");
    await expect.poll(() => requests).toBe(1);
    const scroll = page.locator(".message-scroll");
    await expect(page.locator(".message.assistant .message-content")).toContainText("第 70 段");
    const distance = () => scroll.evaluate(el => el.scrollHeight - el.clientHeight - el.scrollTop);
    await expect.poll(distance).toBeLessThan(2);
    const rect = (await scroll.boundingBox())!;
    await page.mouse.move(rect.x + rect.width / 2, rect.y + rect.height / 2);
    const bottomTop = await scroll.evaluate(el => el.scrollTop);
    await page.mouse.wheel(0, -5);
    await expect.poll(() => scroll.evaluate(el => el.scrollTop)).toBeLessThan(bottomTop - 1);
    const top = await scroll.evaluate(el => el.scrollTop);
    response!.write(frame("\n\n新到达的内容不能把上滑阅读的位置拽走。\n\n"));
    await expect(page.locator(".message.assistant .message-content")).toContainText("不能把上滑");
    const movingTop = await scroll.evaluate(el => el.scrollTop);
    // Native wheel inertia may continue upward; new content must never pull downward.
    expect(movingTop - top).toBeLessThan(2);
    let previous = movingTop, stable = 0;
    await expect.poll(async () => {
      const current = await scroll.evaluate(el => el.scrollTop);
      stable = Math.abs(current - previous) < 0.5 ? stable + 1 : 0;
      previous = current;
      return stable;
    }, { intervals: [32] }).toBeGreaterThanOrEqual(4);
    const restingTop = previous;
    response!.write(frame("\n\n惯性停止后，阅读位置也应保持不动。\n\n"));
    await expect(page.locator(".message.assistant .message-content")).toContainText("惯性停止后");
    expect(Math.abs(await scroll.evaluate(el => el.scrollTop) - restingTop)).toBeLessThan(2);
    await scroll.evaluate(el => {
      const line = parseFloat(getComputedStyle(el.querySelector(".message-content")!).lineHeight);
      el.scrollTop = el.scrollHeight - el.clientHeight - line * 2;
      el.dispatchEvent(new Event("scroll"));
    });
    await expect(page.getByRole("button", { name: "回到最新内容", exact: true })).toHaveCount(0);
    await scroll.evaluate(el => {
      const line = parseFloat(getComputedStyle(el.querySelector(".message-content")!).lineHeight);
      el.scrollTop = el.scrollHeight - el.clientHeight - line * 4;
      el.dispatchEvent(new Event("scroll"));
    });
    await page.getByRole("button", { name: "回到最新内容", exact: true }).click();
    response!.write(frame("\n\n回到底部后继续跟随。\n\n"));
    await expect(page.locator(".message.assistant .message-content")).toContainText("继续跟随");
    await expect.poll(distance).toBeLessThan(2);
    response!.end(frame("\n回答完成。", true));
    await expect(page.getByText("已完成", { exact: true })).toBeVisible();
    await splitChecks(page, "调整回答依据宽度", ".chat-layout", () => page.getByRole("button", { name: "打开回答依据", exact: true }).click());
    const secondary = page.getByRole("button", { name: "查看完整处理记录", exact: true });
    expect(await secondary.evaluate(el => getComputedStyle(el).borderTopWidth)).toBe("0px");
    const secondaryWidth = (await secondary.boundingBox())!.width;
    await secondary.hover();
    expect((await secondary.boundingBox())!.width).toBe(secondaryWidth);
    for (const [width, height] of [[1366, 768], [1600, 900], [1920, 1080]]) {
      await page.setViewportSize({ width, height });
      for (const theme of ["light", "dark"]) {
        await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
        const toolbar = await page.getByRole("group", { name: "窗口操作", exact: true }).boundingBox();
        const evidence = await page.getByRole("complementary", { name: "回答依据", exact: true }).boundingBox();
        expect(toolbar!.y + toolbar!.height).toBeLessThanOrEqual(evidence!.y);
        await page.screenshot({ path: info.outputPath(`chat-${width}-${theme}.png`) });
      }
    }
    await page.getByRole("navigation", { name: "主导航" }).getByRole("link", { name: "知识", exact: true }).click();
    await page.getByRole("button", { name: "资料文件", exact: true }).click();
    await page.getByRole("button", { name: "添加文件", exact: true }).click();
    await page.getByRole("button", { name: "确认导入", exact: true }).click();
    const sourceDialog = page.getByRole("dialog", { name: "资料文件", exact: true });
    const filename = sourceDialog.locator(".source-list .file-name");
    await expect(filename).toHaveAttribute("title", name);
    await expect(filename.locator(".file-name-extension")).toHaveText(".md");
    expect(await filename.locator(".file-name-stem").evaluate(el => el.scrollWidth > el.clientWidth)).toBe(true);
    await splitChecks(page, "调整文件预览宽度", ".source-workbench", () => page.getByRole("button", { name: "显示文件预览", exact: true }).click());
    await expect(sourceDialog).toBeVisible();
    await page.getByRole("combobox", { name: "文件版本", exact: true }).click();
    const select = page.getByRole("combobox", { name: "文件版本", exact: true });
    await expectPicker(select, true);
    const corners = await select.evaluate(el => {
      const outer = getComputedStyle(el, "::picker(select)"), inner = getComputedStyle(el.querySelector("option")!);
      return { outer: parseFloat(outer.borderTopLeftRadius), inner: parseFloat(inner.borderTopLeftRadius), gap: parseFloat(outer.paddingLeft) };
    });
    expect(corners.outer).toBe(corners.inner + corners.gap);
    await page.keyboard.press("Escape");
    await expect(sourceDialog).toBeVisible();
    for (const zoom of [1, 1.25, 1.5]) {
      await page.evaluate(value => { document.documentElement.style.zoom = String(value); }, zoom);
      if (zoom === 1.25) await splitChecks(page, "调整文件预览宽度", ".source-workbench", () => page.getByRole("button", { name: "显示文件预览", exact: true }).click());
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: info.outputPath(`source-zoom-${zoom}.png`) });
    }
    await page.evaluate(() => { document.documentElement.style.zoom = "1"; });
    await page.getByRole("button", { name: "关闭资料文件", exact: true }).click();
    expect(app.consoleErrors).toEqual([]);
  } finally {
    response?.destroy();
    await app.stop();
    server.closeAllConnections();
    await new Promise<void>(done => server.close(() => done()));
  }
});
