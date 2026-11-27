import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative } from "./native";

test("first-launch selects local/cloud modes, top-right monitors both, and settings toggles sync live", async ({}, info) => {
  test.setTimeout(90_000);
  const fixture = createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    if (req.url === "/api/version") res.end('{"version":"onboarding-fixture"}');
    else if (req.url === "/api/show") res.end('{"details":{"format":"gguf"}}');
    else if (req.url === "/api/tags") res.end('{"models":[{"name":"qwen3:4b-instruct","size":1}]}');
    else res.end("{}");
  });
  await new Promise<void>((done) => fixture.listen(0, "127.0.0.1", done));
  const address = fixture.address();
  if (!address || typeof address === "string") throw Error("fixture address unavailable");

  const app = await launchNative(info, { endpoint: `http://127.0.0.1:${address.port}` });
  try {
    const { page } = app;
    const dialog = page.getByRole("dialog", { name: "欢迎使用 CortexOS" });
    await expect(dialog).toBeVisible({ timeout: 20_000 });

    const modeGroup = dialog.getByRole("group", { name: "模型使用方式" });
    const localCheckbox = modeGroup.getByRole("checkbox", { name: /本地模型（Ollama）/ });
    const cloudCheckbox = modeGroup.getByRole("checkbox", { name: /云端模型/ });
    await expect(localCheckbox).toBeChecked();
    await expect(cloudCheckbox).toBeChecked();

    await page.screenshot({ path: info.outputPath("onboarding-model-choice.png"), animations: "disabled" });

    // Unchecking both must show error and disable confirm button
    await localCheckbox.uncheck();
    await cloudCheckbox.uncheck();
    await expect(dialog.getByText("请至少勾选一种模型使用方式（本地或云端）。")).toBeVisible();
    const confirmBtn = dialog.getByRole("button", { name: /确认并开始|保存并开始/ });
    await expect(confirmBtn).toBeDisabled();

    // Re-check both and confirm
    await localCheckbox.check();
    await cloudCheckbox.check();
    await expect(confirmBtn).toBeEnabled();
    await confirmBtn.click();
    await expect(dialog).not.toBeVisible();

    const statusGroup = page.getByRole("group", { name: "模型连接状态" });
    await expect(statusGroup.getByRole("link", { name: "本机模型已就绪" })).toBeVisible();
    await expect(statusGroup.getByRole("link", { name: "云端待建工作区" })).toBeVisible();

    await createWorkspace(page, "双通道状态验收");
    await expect(statusGroup.getByRole("link", { name: "云端未配置" })).toBeVisible();
    await page.screenshot({ path: info.outputPath("topbar-local-and-cloud-status.png"), animations: "disabled" });

    // Navigate via topbar cloud status pill to Settings -> Cloud
    await statusGroup.getByRole("link", { name: "云端未配置" }).click();
    await expect(page.getByRole("heading", { name: "云端连接" })).toBeVisible();

    await page.getByRole("button", { name: "添加连接", exact: true }).click();
    await page.getByLabel("连接名称", { exact: true }).fill("测试云端连接");
    await page.getByLabel("API 密钥", { exact: true }).fill("sk-test-local-dummy-key-123456");
    await page.getByRole("button", { name: "保存连接", exact: true }).click();

    await expect(statusGroup.getByRole("link", { name: "云端待验证" })).toBeVisible();
    await page.screenshot({ path: info.outputPath("topbar-cloud-pending-status.png"), animations: "disabled" });

    // Clean up dummy OS credential & confirm live update back to 云端未配置
    await page.getByRole("button", { name: "移除连接", exact: true }).click();
    await page.getByRole("button", { name: "确认移除", exact: true }).click();
    await expect(statusGroup.getByRole("link", { name: "云端未配置" })).toBeVisible();

    // Toggle local/cloud preference inside Settings -> 模型连接
    await page.getByRole("button", { name: "模型连接", exact: true }).click();
    const prefGroup = page.getByRole("group", { name: "模型使用方式偏好" });
    const settingsLocal = prefGroup.getByRole("checkbox", { name: /本地模型（Ollama）/ });
    const settingsCloud = prefGroup.getByRole("checkbox", { name: /云端模型（API）/ });
    await expect(settingsLocal).toBeChecked();
    await expect(settingsCloud).toBeChecked();

    await settingsLocal.uncheck();
    await expect(statusGroup.getByRole("link", { name: "本机模型已就绪" })).toHaveCount(0);
    await expect(statusGroup.getByRole("link", { name: "云端未配置" })).toBeVisible();

    // Attempting to uncheck the remaining cloud checkbox is prevented
    await settingsCloud.click();
    await expect(prefGroup.getByText("至少保留一种模型使用方式（本地或云端）。")).toBeVisible();
    await expect(settingsCloud).toBeChecked();

    await settingsLocal.check();
    await expect(statusGroup.getByRole("link", { name: "本机模型已就绪" })).toBeVisible();
  } finally {
    await app.stop();
    fixture.closeAllConnections();
    fixture.close();
  }
});
