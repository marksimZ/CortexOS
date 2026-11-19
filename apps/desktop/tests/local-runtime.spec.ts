import { expect, test } from "@playwright/test";

test.use({ headless: true });

test("local device preferences persist and unloading is confirmed", async ({ page }, info) => {
  await page.addInitScript(() => {
    const settings = { model: "fixture", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 };
    let device = { device: "auto", keepAliveSeconds: 0 };
    let loaded = true;
    let releases = 0;
    Object.assign(window, {
      isTauri: true,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: { metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback() {},
        invoke: async (command: string, args: { input?: typeof device; model?: string; workspaceId?: string; digest?: string }) => {
          switch (command) {
            case "bootstrap": return { workspaces: [{ id: "w", name: "本机验证" }], settings, activeRun: null };
            case "check_ollama": return { version: "fixture", models: [{ name: "fixture", size: 1024 ** 3 }] };
            case "list_conversations": return [];
            case "local_model_preferences": if (args.input) device = args.input; return { ...device };
            case "inspect_local_model_runtime": return { inspection: { model: args.model, local: true, generation: true, embedding: false, contextCapacity: 8192 },
              catalog: { loaded: loaded ? [{ name: "fixture", digest: "f".repeat(64), sizeBytes: 4 * 1024 ** 3, sizeVramBytes: 3 * 1024 ** 3, contextLength: 4096 }] : [] } };
            case "release_local_runtime_model":
              if (args.workspaceId !== "w" || args.model !== "fixture" || args.digest !== "f".repeat(64) || ++releases !== 1) throw Error("Wrong unload request");
              loaded = false;
              return { status: "released" };
            default: if (command.startsWith("plugin:")) return 1; throw Error(`Unexpected ${command}`);
          }
        },
      },
    });
  });
  await page.goto("/#/settings?workspace=w");
  await page.getByText("本机运行与资源", { exact: true }).click();
  const device = page.getByLabel("对话计算设备", { exact: true });
  await device.selectOption("cpu");
  await page.getByLabel("回答后保留模型（秒）", { exact: true }).fill("30");
  await page.getByRole("button", { name: "保存本机运行设置", exact: true }).click();
  await expect(page.getByText("下次对话将使用新的运行设置。", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "刷新模型状态", exact: true }).click();
  await expect(page.getByText(/显存 3.00 GiB/)).toBeVisible();
  await expect(page.getByText(/生成支持，向量不支持/)).toBeVisible();
  for (const theme of ["light", "dark"]) {
    await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
    await expect(page.locator("html")).toHaveAttribute("data-color-scheme", theme);
    for (const [width, height] of [[1366, 768], [1920, 1080]]) {
      await page.setViewportSize({ width, height });
      await page.getByText(/显存 3.00 GiB/).scrollIntoViewIfNeeded();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: info.outputPath(`local-runtime-${theme}-${width}.png`) });
    }
  }
  await page.getByRole("button", { name: "卸载 fixture", exact: true }).click();
  await page.getByRole("button", { name: "取消", exact: true }).click();
  await expect(page.getByText(/显存 3.00 GiB/)).toBeVisible();
  await page.getByRole("button", { name: "卸载 fixture", exact: true }).click();
  await page.getByRole("button", { name: "确认卸载 fixture", exact: true }).click();
  await expect(page.getByText("当前没有已加载模型。", { exact: true })).toBeVisible();
  await expect(device).toHaveValue("cpu");
});
