import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { createWorkspace, launchNative, nativeInvoke } from "./native";

test("published personal instructions reach local chat and rollback preserves prior evidence", async ({}, info) => {
  test.setTimeout(90_000);
  const prompts: string[] = [];
  const fixture = createServer(async (req, res) => {
    if (req.url === "/api/chat") {
      const chunks: Buffer[] = [];
      for await (const chunk of req) chunks.push(chunk);
      const { messages } = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      prompts.push(messages[0].content);
      res.writeHead(200, { "content-type": "application/x-ndjson" });
      res.end(JSON.stringify({ message: { role: "assistant", content: "固定回复 " + prompts.length }, done: true }) + "\n");
      return;
    }
    res.setHeader("content-type", "application/json");
    if (req.url === "/api/version") res.end('{"version":"assistant-fixture"}');
    else if (req.url === "/api/show") res.end('{"details":{"format":"gguf"}}');
    else if (req.url === "/api/tags") res.end('{"models":[{"name":"qwen3:4b-instruct","size":1}]}');
    else res.end("{}");
  });
  await new Promise<void>(done => fixture.listen(0, "127.0.0.1", done));
  const address = fixture.address();
  if (!address || typeof address === "string") throw Error("fixture address unavailable");
  const app = await launchNative(info, { endpoint: "http://127.0.0.1:" + address.port });
  try {
    const { page } = app;
    await createWorkspace(page, "助手验收");
    const workspaceId = await page.getByLabel("当前工作区", { exact: true }).inputValue();
    const first = "先列风险，再列进展。";
    const second = "结尾单列下一步。";
    const payload = (instructions: string) => ({ workspaceId, name: "周报助手", instructions });
    async function turn() {
      const sequence = prompts.length + 1;
      await page.getByRole("textbox", { name: "你的问题" }).fill("请整理本周进展 " + sequence);
      await page.getByRole("button", { name: "发送消息", exact: true }).click();
      await expect(page.getByText("固定回复 " + sequence, { exact: true })).toBeVisible();
      const [run] = await nativeInvoke<{id: string}[]>(page, "list_runs", { workspaceId });
      return run.id;
    }
    await page.getByRole("link", { name: "设置", exact: true }).click();
    await page.getByRole("button", { name: "我的助手", exact: true }).click();
    await page.getByLabel("助手名称").fill("周报助手");
    await page.getByLabel("回答指令").fill(first);
    await page.getByRole("button", {name:"发布并启用"}).click();
    await expect(page.getByText("使用中", {exact:true})).toBeVisible();
    await page.getByLabel("回答指令").fill(second);
    await page.getByRole("button", {name:"保存草稿"}).click();
    await expect(page.getByText("草稿已保存。", {exact:true})).toBeVisible();
    await page.getByRole("link", {name:"对话", exact:true}).click();
    const original = await turn();
    const detail = () => nativeInvoke<{contextManifest: Record<string, unknown>}>(page, "get_run", { workspaceId, runId: original });
    const historical = (await detail()).contextManifest;
    expect(historical.assistant).toEqual({kind:"workspace",name:"周报助手",version:1});
    await nativeInvoke(page, "publish_assistant", payload(second));
    await turn();
    await nativeInvoke(page, "activate_assistant", { workspaceId, version: 1 });
    await turn();
    await nativeInvoke(page, "activate_assistant", { workspaceId, version: null });
    await turn();
    expect(prompts).toHaveLength(4);
    expect(prompts[0]).toContain(first);
    expect(prompts[0]).not.toContain(second);
    expect(prompts[1]).toContain(second);
    expect(prompts[1]).not.toContain(first);
    expect(prompts[2]).toBe(prompts[0]);
    expect(prompts[3]).not.toContain(first);
    expect(prompts[3]).not.toContain(second);
    expect((await detail()).contextManifest).toEqual(historical);
    const other = await nativeInvoke<{id: string}>(page, "create_workspace", { name: "隔离验收" });
    expect(await nativeInvoke(page, "read_assistant", { workspaceId: other.id })).toMatchObject({ activeVersion: null, versions: [] });
    await expect(nativeInvoke(page, "activate_assistant", { workspaceId: other.id, version: 1 })).rejects.toThrow();
    await page.getByRole("link", { name: "设置", exact: true }).click();
    await page.getByRole("button", { name: "我的助手", exact: true }).click();
    await expect(page.getByText("周报助手", {exact:true}).first()).toBeVisible();
    await page.screenshot({ path: info.outputPath("assistant-versions.png"), animations: "disabled" });
  } finally {
    await app.stop();
    fixture.closeAllConnections();
    fixture.close();
  }
});
