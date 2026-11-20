import { execFileSync } from "node:child_process";
import { expect, test } from "@playwright/test";
import { createServer, type ServerResponse } from "node:http";
import { writeFile } from "node:fs/promises";
import { setTimeout as delay } from "node:timers/promises";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

test("H1 native screenshots, route timings and controlled 3000 character stream", async ({}, info) => {
  test.setTimeout(150_000);
  let stream: ServerResponse | undefined;
  const server = createServer(async (request, response) => {
    for await (const chunk of request) void chunk;
    const reply = (value: unknown) => response.end(JSON.stringify(value));
    response.setHeader("Content-Type", "application/json");
    switch (request.url) {
      case "/api/version": return reply({ version: "h1-fixture" });
      case "/api/tags": return reply({ models: [{ name: "qwen3:4b-instruct", details: { format: "gguf" } }] });
      case "/api/show": return reply({ details: { format: "gguf" }, capabilities: ["completion"] });
      case "/api/ps": return reply({ models: [] });
      case "/api/chat":
        stream = response;
        response.setHeader("Content-Type", "application/x-ndjson");
        response.flushHeaders();
        return;
      default: response.statusCode = 404; return response.end();
    }
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw Error("fixture port unavailable");
  const app = await launchNative(info, {
    endpoint: `http://127.0.0.1:${address.port}`,
    executable: process.env.CORTEXOS_H1_EXECUTABLE,
  });
  try {
    const { page, context } = app;
    await page.setViewportSize({ width: 1600, height: 900 });
    await page.addInitScript(() => {
      const profile = { roots: 0, shells: 0, sidebars: 0, chat: 0, other: 0 };
      type Fiber = {
        child?: Fiber; sibling?: Fiber; memoizedProps?: unknown;
        stateNode?: Element; tag: number;
      };
      const committedProps = new WeakMap<Element, unknown>();
      const hooks = {
        supportsFiber: true,
        inject: () => 1,
        onCommitFiberRoot: (_: number, root: { current: Fiber }) => {
          profile.roots++;
          const visit = (node?: Fiber) => {
            for (; node; node = node.sibling) {
              const classes = node.tag === 5 && node.stateNode?.getAttribute("class");
              if (classes && node.stateNode && committedProps.get(node.stateNode) !== node.memoizedProps) {
                const matches = (value: string) => classes.split(" ").includes(value);
                if (matches("prototype-shell")) profile.shells++;
                if (matches("conversation-list")) profile.sidebars++;
                if (matches("chat-main")) profile.chat++;
                if (matches("settings-content")) profile.other++;
                committedProps.set(node.stateNode, node.memoizedProps);
              }
              if (node.child) visit(node.child);
            }
          };
          visit(root.current.child);
        },
      };
      Object.assign(window, { h1Profile: profile, __REACT_DEVTOOLS_GLOBAL_HOOK__: hooks });
    });
    await page.reload();
    const cdp = await context.newCDPSession(page);
    const scripts = new Map<string, string>();
    cdp.on("Debugger.scriptParsed", event => scripts.set(event.url, event.scriptId));
    await cdp.send("Debugger.enable");
    const initialUrls = await page.evaluate(() => [...new Set([
      ...[...document.querySelectorAll<HTMLScriptElement>("script[src]")].map(element => element.src),
      ...[...document.querySelectorAll<HTMLLinkElement>('link[rel="modulepreload"]')].map(element => element.href),
    ])]);
    const initialScripts = await Promise.all(initialUrls.map(async url => {
      const id = scripts.get(url);
      const source = id ? (await cdp.send("Debugger.getScriptSource", { scriptId: id })).scriptSource : null;
      return { name: new URL(url).pathname, bytes: source === null ? null : Buffer.byteLength(source, "utf8") };
    }));
    await createWorkspace(page, "H1 合成工作区");
    const workspaceId = await page.getByLabel("当前工作区").inputValue();
    for (const title of ["界面基线资料", "性能测量约定", "复现条件"]) {
      const id = await nativeInvoke<string>(page, "save_knowledge", { input: { workspaceId, title, kind: "knowledge", content: "合成资料供界面基线验证。", changeReason: "H1 界面测试" } });
      await nativeInvoke(page, "approve_knowledge", { workspaceId, nodeId: id, expectedRevision: 1 });
    }
    await cdp.send("Performance.enable");
    await cdp.send("Profiler.enable");
    const profile = () => page.evaluate(() => ({ ...(window as unknown as { h1Profile: Record<string, number> }).h1Profile }));
    await page.getByRole("textbox", { name: "你的问题" }).fill("请给出用于验证界面显示的合成回答。");
    await page.getByRole("button", { name: "发送消息", exact: true }).click();
    await expect.poll(() => !!stream).toBe(true);
    const fragment = "段落用于测量流式回答显示和正文解析。\n\n- 界面验证\n";
    const answer = fragment.repeat(3000).slice(0, 3000);
    const first = await profile();
    await cdp.send("Profiler.start");
    await cdp.send("Profiler.startPreciseCoverage", { callCount: true, detailed: false });
    const write = (content: string, done: boolean) => JSON.stringify({ message: { role: "assistant", content }, done, ...(done && { done_reason: "stop" }) }) + "\n";
    for (let offset = 0; offset < answer.length; offset += 50) {
      stream!.write(write(answer.slice(offset, offset + 50), false));
      await delay(30);
    }
    await expect(page.locator(".message.assistant .message-content")).toContainText("界面验证");
    const during = await profile();
    const streamingCoverage = await cdp.send("Profiler.takePreciseCoverage");
    stream!.end(write("", true));
    await expect(page.getByText("已完成", { exact: true })).toBeVisible();
    await delay(150);
    const after = await profile();
    const finishingCoverage = await cdp.send("Profiler.takePreciseCoverage");
    await cdp.send("Profiler.stopPreciseCoverage");
    const cpu = await cdp.send("Profiler.stop");
    const parser = [];
    for (const script of streamingCoverage.result.filter(script => script.url.includes("/assets/"))) {
      const { scriptSource } = await cdp.send("Debugger.getScriptSource", { scriptId: script.scriptId });
      const marker = scriptSource.search(/\.runSync\(\w+\.parse\(/);
      if (marker < 0) continue;
      const candidates = script.functions.filter(fn => fn.ranges[0].startOffset <= marker && fn.ranges[0].endOffset > marker);
      const entry = candidates.sort((a, b) => b.ranges[0].startOffset - a.ranges[0].startOffset)[0];
      if (!entry) continue;
      const range = entry.ranges[0];
      const terminal = finishingCoverage.result.find(other => other.scriptId === script.scriptId)?.functions.find(fn => fn.ranges[0].startOffset === range.startOffset);
      const call = scriptSource.slice(range.startOffset, range.endOffset);
      const nodes = new Map(cpu.profile.nodes.map(node => [node.id, node]));
      const parents = new Map(cpu.profile.nodes.flatMap(node => (node.children ?? []).map(child => [child, node.id] as const)));
      const stackContains = (id: number) => {
        for (let current: number | undefined = id; current !== undefined; current = parents.get(current)) {
          const frame = nodes.get(current)?.callFrame;
          if (frame?.scriptId === script.scriptId && frame.functionName === entry.functionName) return true;
        }
        return false;
      };
      const estimatedCpuMs = (cpu.profile.samples ?? []).reduce((sum, id, index) => sum + (stackContains(id) ? (cpu.profile.timeDeltas?.[index] ?? 0) / 1000 : 0), 0);
      parser.push({estimatedCpuMs, functionName: entry.functionName, file: script.url, streamInputs: range.count, terminalInputs: terminal?.ranges[0].count ?? 0, sample: call.slice(0, 220)});
    }
    await writeFile(info.outputPath("stream.cpuprofile"), JSON.stringify(cpu.profile));
    expect(during.roots).toBeGreaterThan(first.roots);
    const conversation = (await nativeInvoke<{id: string}[]>(page, "list_conversations", {workspaceId}))[0];
    const persisted = await nativeInvoke<{role: string; content: string}[]>(page, "load_messages", {workspaceId, conversationId: conversation.id});
    expect(persisted.find(message => message.role === "assistant")?.content).toBe(answer);
    const appearance = page.getByRole("combobox", { name: "外观", exact: true });
    const routes = {
      chat: ".composer-area",
      knowledge: ".knowledge-workbench",
      graph: 'canvas[data-webgl-ready="true"]',
      benchmark: ".personal-step",
      settings: "#model-section",
    };
    const frames = [];
    const screens = [];
    for (const theme of ["light", "dark"]) {
      await appearance.selectOption(theme);
      for (const width of [800, 1366, 1600, 1920]) {
        const height = ({ 800: 600, 1366: 768, 1600: 900, 1920: 1080 })[width]!;
        await page.setViewportSize({ width, height });
        for (const [route, selector] of Object.entries(routes)) {
          const start = performance.now();
          await page.evaluate(route => { location.hash = `#/${route}`; }, route);
          await expect(page.locator(selector).first()).toBeVisible();
          const elapsedMs = performance.now() - start;
          const bounds = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, scrollWidth: document.documentElement.scrollWidth, pixelRatio: devicePixelRatio }));
          const screenshot = `${route}-${theme}-${width}.png`;
          await page.screenshot({ path: info.outputPath(screenshot) });
          screens.push({ route, theme, screenshot, elapsedMs, ...bounds });
          if (route === "graph" && width === 1600) {
            const canvas = page.locator(selector);
            const box = (await canvas.boundingBox())!;
            await page.evaluate(() => {
              const samples: number[] = [];
              Object.assign(window, { h1Frames: samples });
              const start = performance.now();
              const draw = (timestamp: number) => { samples.push(timestamp); if (timestamp - start < 750) requestAnimationFrame(draw); };
              requestAnimationFrame(draw);
            });
            await page.mouse.move(box.x + box.width * .45, box.y + box.height * .5);
            await page.mouse.down();
            await page.mouse.move(box.x + box.width * .55, box.y + box.height * .55, { steps: 12 });
            await page.mouse.up();
            await delay(780);
            frames.push(await page.evaluate(() => (window as unknown as { h1Frames: number[] }).h1Frames.slice(1).map((v, i) => v - (window as unknown as { h1Frames: number[] }).h1Frames[i])));
          }
        }
      }
    }
    await writeFile(info.outputPath("baseline.json"), JSON.stringify({ browser: await page.evaluate(() => navigator.userAgent), chars: answer.length, chunks: 60, first, during, after, parser, initialScripts, screens, frames }, null, 2));
  } finally {
    await app.stop();
    server.closeAllConnections();
    server.close();
  }
});

test("H1 synthetic 100, 500 and 1000 message scroll baseline", async ({}, info) => {
  test.setTimeout(100_000);
  const app = await launchNative(info, { endpoint: "http://127.0.0.1:1", executable: process.env.CORTEXOS_H1_EXECUTABLE });
  try {
    const { page } = app;
    page.setDefaultTimeout(7_000);
    await page.setViewportSize({ width: 1600, height: 900 });
    await createWorkspace(page, "H1 合成长历史");
    const workspaceId = await page.getByLabel("当前工作区").inputValue();
    execFileSync("py", ["-3.13", "tests/h1-history-fixture.py", info.outputPath("data", "cortexos.sqlite3"), workspaceId], {windowsHide:true});
    const open = async (count: number) => {
      await page.evaluate(({ workspaceId, count }) => { location.hash = `#/chat?workspace=${workspaceId}&conversation=h1-fixture-${count}`; }, { workspaceId, count });
      const lastId = `message-h1-fixture-${count}-run-${count / 2 - 1}-answer`;
      await expect(page.locator(`#${lastId}`)).toBeAttached();
    };
    const results = [];
    for (const count of [100, 500, 1000]) {
      const trials = [];
      for (let trial = 0; trial < 3; trial++) {
        if (trial) {
          await open(count === 100 ? 500 : 100);
        }
        const start = performance.now();
        await open(count);
        const initialMs = performance.now() - start;
        const mounted = await page.locator(".message-scroll article").count();
        const scroll = await page.locator(".message-scroll").evaluate(element => new Promise<{ frames: number[]; distance: number }>((resolve) => {
          const frames: number[] = [];
          const start = performance.now();
          const max = element.scrollHeight - element.clientHeight;
          const tick = (now: number) => {
            frames.push(now);
            element.scrollTop = Math.max(0, max - (now - start) * 4);
            if (now - start < 600) requestAnimationFrame(tick);
            else resolve({ frames: frames.slice(1).map((n, i) => n - frames[i]), distance: max - element.scrollTop });
          };
          requestAnimationFrame(tick);
        }));
        expect(scroll.distance).toBeGreaterThan(0);
        trials.push({ initialMs, mounted, ...scroll });
      }
      results.push({ count, trials });
    }
    const older = page.getByRole("button", {name: /^加载更早的消息/});
    if (await older.count()) {
      const list = page.locator(".message-scroll article");
      await expect(list).toHaveCount(60);
      const first = page.locator("#message-h1-fixture-1000-run-470-question");
      await older.scrollIntoViewIfNeeded();
      const beforeTop = await first.evaluate(element => element.getBoundingClientRect().top);
      await older.click();
      await expect(list).toHaveCount(120);
      await expect.poll(async () => Math.abs((await first.evaluate(element => element.getBoundingClientRect().top)) - beforeTop)).toBeLessThan(4);
      const message = "h1-fixture-1000-run-0-answer";
      await page.evaluate(({workspaceId,message}) => {
        location.hash = `#/chat?workspace=${workspaceId}&conversation=h1-fixture-1000&message=${message}`;
      }, {workspaceId,message});
      await expect(page.locator(`#message-${message}`)).toBeFocused();
      await expect(list).toHaveCount(60);
      await page.getByRole("button", {name: /^加载更新的消息/}).click();
      await expect(list).toHaveCount(120);
      await page.getByRole("button", {name: "回到最新内容", exact: true}).click();
      await expect(page.locator("#message-h1-fixture-1000-run-499-answer")).toBeAttached();
      await expect(list).toHaveCount(60);
    }
    await writeFile(info.outputPath("history.json"), JSON.stringify(results, null, 2));
  } finally { await app.stop(); }
});

test("H1.4 native theme contrast and narrow high contrast keyboard controls", async ({}, info) => {
  test.skip(process.env.CORTEXOS_H14_CHECK !== "1", "Run against the H1.4 design system.");
  const app = await launchNative(info, { endpoint: "http://127.0.0.1:1" });
  try {
    const { page } = app;
    await page.setViewportSize({ width: 800, height: 600 });
    await page.evaluate(() => { location.hash = "#/settings"; });
    const settings = page.getByRole("navigation", { name: "设置分类" });
    const appearance = page.getByLabel("外观", { exact: true });
    await expect(settings).toBeVisible();
    const pairs: Record<string, Record<string, number>> = {};
    for (const theme of ["light", "dark"] as const) {
      await appearance.selectOption(theme);
      await expect(page.locator("html")).toHaveAttribute("data-color-scheme", theme);
      pairs[theme] = await page.evaluate(() => {
        const style = getComputedStyle(document.documentElement);
        const sample = document.createElement("canvas").getContext("2d", { willReadFrequently: true })!;
        const luminance = (token: string) => {
          const color = style.getPropertyValue(token).trim();
          if (!CSS.supports("color", color)) throw new Error(`无效颜色 ${token}`);
          sample.fillStyle = color;
          sample.fillRect(0, 0, 1, 1);
          const [r, g, b] = Array.from(sample.getImageData(0, 0, 1, 1).data.slice(0, 3), value => {
            const channel = value / 255;
            return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
          });
          return r * 0.2126 + g * 0.7152 + b * 0.0722;
        };
        const ratio = (a: string, b: string) => {
          const values = [luminance(a), luminance(b)].sort((a, b) => b - a);
          return (values[0] + 0.05) / (values[1] + 0.05);
        };
        return {
          text: ratio("--text", "--background"),
          muted: ratio("--muted", "--background"),
          primaryButton: ratio("--on-primary", "--primary"),
          danger: ratio("--danger", "--background"),
          success: ratio("--success-text", "--background"),
        };
      });
      for (const value of Object.values(pairs[theme])) expect(value).toBeGreaterThanOrEqual(4.5);
      expect(await settings.evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
    }
    await page.emulateMedia({ forcedColors: "active" });
    const data = settings.getByRole("button", { name: "本地数据", exact: true });
    await page.keyboard.press("Tab");
    await data.focus();
    await expect(data).toBeFocused();
    await expect(data).toHaveCSS("outline-style", "solid");
    await data.press("Enter");
    await expect(data).toHaveAttribute("aria-pressed", "true");
    await expect(data).toHaveCSS("text-decoration-line", "underline");
    await expect(page.locator("#data-section")).toBeVisible();
    await page.screenshot({ path: info.outputPath("settings-forced-colors-800.png") });
    await writeFile(info.outputPath("contrast.json"), JSON.stringify(pairs, null, 2));
  } finally {
    await app.stop();
  }
});
