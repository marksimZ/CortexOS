import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { createServer } from "node:http";
import { createWorkspace, launchNative, nativeInvoke } from "./native";
import type { ReminderList } from "../src/api/reminders";

test("scheduled news saves scoped editions across restart", async ({}, info) => {
  test.setTimeout(90_000);
  const queries: string[] = [];
  const fixture = createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://127.0.0.1");
    if (url.pathname !== "/news/search") { res.writeHead(404); res.end(); return; }
    const query = url.searchParams.get("q") ?? "";
    queries.push(query);
    if (query === "服务异常") { res.writeHead(503); res.end(); return; }
    res.setHeader("content-type","application/rss+xml");
    const story = "<item><title>简报验收条目</title><description>本期摘要</description><link>https://example.com/story</link></item>";
    res.end("<rss><channel>" + (query === "无新闻" ? "" : story) + "</channel></rss>");
  });
  await new Promise<void>(done => fixture.listen(0,"127.0.0.1",done));
  const address = fixture.address();
  if (!address || typeof address === "string") throw Error("fixture unavailable");
  const previous = process.env.CORTEXOS_TEST_BRAVE_URL;
  process.env.CORTEXOS_TEST_BRAVE_URL = "http://127.0.0.1:" + address.port;
  const options = {endpoint:"http://127.0.0.1:1", dataDirectory:info.outputPath("data")};
  let app: Awaited<ReturnType<typeof launchNative>> | undefined;
  try {
    app = await launchNative(info,options);
    const page = app.page;
    await createWorkspace(page,"定时资讯");
    const workspaceId = await page.getByLabel("当前工作区",{exact:true}).inputValue();
    const list = () => nativeInvoke<ReminderList>(app!.page,"list_reminders",{workspaceId});
    await page.getByRole("textbox",{name:"你的问题"}).fill("每个工作日早上九点整理 AI 新闻");
    await page.getByRole("button",{name:"预览新闻简报"}).click();
    await expect(page.getByLabel("新闻查询词")).toHaveValue("AI 新闻");
    expect((await list()).reminders).toHaveLength(0);
    await page.screenshot({path:info.outputPath("schedule.png")});
    await page.getByRole("button",{name:"确认启用"}).click();
    await expect.poll(async () => (await list()).reminders.length).toBe(1);
    const saved = (await list()).reminders[0];
    expect(saved).toMatchObject({kind:"weekdays",time:"09:00",newsQuery:"AI 新闻"});
    for (const query of ["无新闻","服务异常"]) {
      await nativeInvoke(page,"save_reminder",{workspaceId,id:null,input:{...saved,title:query,newsQuery:query}});
    }
    expect(queries).toHaveLength(0);
    await app.closeWindow(); await app.stop();
    // Move the three confirmed plans to the previous minute in the stopped test instance.
    execFileSync("py.exe",["-3","-c",[
      "import sqlite3, sys, datetime",
      "t = datetime.datetime.now() - datetime.timedelta(minutes=1)",
      "with sqlite3.connect(sys.argv[1]) as db:",
      "    db.execute(\"UPDATE reminders SET kind='daily', date=?, next_date=?, time=? WHERE workspace_id=?\",(str(t.date()),str(t.date()),t.strftime('%H:%M'),sys.argv[2]))",
    ].join("\n"),join(options.dataDirectory,"cortexos.sqlite3"),workspaceId],{windowsHide:true});
    app = await launchNative(info,options);
    await expect.poll(async () => (await list()).runs.map(r=>r.brief?.status).sort(),{timeout:30_000}).toEqual(["empty","failed","ready"]);
    const {runs} = await list();
    expect(runs.find(r=>r.brief?.status==="ready")?.brief?.results[0]).toMatchObject({title:"简报验收条目",url:"https://example.com/story",snippet:"本期摘要"});
    await app.page.getByRole("button",{name:"任务中心",exact:true}).click();
    const drawer = app.page.getByRole("dialog",{name:"任务中心"});
    await drawer.locator("li").filter({hasText:saved.title}).locator("summary").first().click();
    await expect(drawer.getByRole("link",{name:"简报验收条目"})).toBeVisible();
    await drawer.getByText("本期摘要", {exact:true}).scrollIntoViewIfNeeded();
    await app.page.screenshot({path:info.outputPath("saved-edition.png")});
    await app.page.keyboard.press("Escape");
    const other = await nativeInvoke<{id:string}>(app.page,"create_workspace",{name:"独立工作区"});
    expect(await nativeInvoke(app.page,"list_reminders",{workspaceId:other.id})).toEqual({reminders:[],runs:[]});
    await app.closeWindow(); await app.stop();
    // Recreate an interrupted fetch for the failure case only; the completed edition stays intact.
    const failedRun = runs.find(r => r.brief?.status === "failed")!;
    execFileSync("py.exe", ["-3","-c",[
      "import sqlite3, sys",
      "with sqlite3.connect(sys.argv[1]) as db:",
      "    db.execute(\"UPDATE reminder_runs SET brief_status='pending',brief_error=NULL WHERE id=?\",(sys.argv[2],))",
    ].join("\n"),join(options.dataDirectory,"cortexos.sqlite3"),failedRun.id],{windowsHide:true});
    app = await launchNative(info,options);
    await expect.poll(async () => (await list()).runs.find(r=>r.id===failedRun.id)?.brief?.status).toBe("failed");
    const restored = (await list()).runs;
    expect(restored.filter(r=>r.id!==failedRun.id)).toEqual(runs.filter(r=>r.id!==failedRun.id));
    expect(restored.find(r=>r.id===failedRun.id)?.brief?.error).toContain("退出");
    expect(queries.sort()).toEqual(["AI 新闻","无新闻","服务异常"].sort());
  } finally {
    if (previous === undefined) delete process.env.CORTEXOS_TEST_BRAVE_URL;
    else process.env.CORTEXOS_TEST_BRAVE_URL = previous;
    await app?.stop(); fixture.closeAllConnections(); fixture.close();
  }
});

test("chat reminder survives restart, stays scoped and delivers once", async ({}, info) => {
  test.setTimeout(180_000);
  const options = { endpoint: "http://127.0.0.1:1", dataDirectory: info.outputPath("data") };
  let app = await launchNative(info, options);
  try {
    const { page } = app;
    await createWorkspace(page, "提醒验收");
    const workspaceId = await page.getByLabel("当前工作区", { exact: true }).inputValue();
    const list = () => nativeInvoke<ReminderList>(app.page, "list_reminders", { workspaceId });
    await page.getByRole("textbox", { name: "你的问题" }).fill("如何整理项目资料？");
    await expect(page.getByRole("button", { name: "发送消息", exact: true })).toBeDisabled();
    await page.getByRole("textbox", { name: "你的问题" }).fill("每个工作日下午六点提醒我写日志");
    await page.getByRole("textbox", { name: "你的问题" }).press("Enter");
    await expect(page.getByLabel("提醒内容")).toHaveValue("写日志");
    await expect(page.getByLabel("本地时间")).toHaveValue("18:00");
    expect((await list()).reminders).toHaveLength(0);
    await page.screenshot({ path: info.outputPath("chat-draft.png") });
    await page.getByRole("button", { name: "确认保存", exact: true }).click();
    await expect.poll(async () => (await list()).reminders.length).toBe(1);
    const reminder = (await list()).reminders[0];
    expect(reminder.kind).toBe("weekdays");
    await page.getByRole("button", { name: "任务中心", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "任务中心" });
    await dialog.getByRole("button", { name: "暂停", exact: true }).click();
    await expect(dialog.getByText("已暂停", { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "编辑", exact: true }).click();
    await dialog.getByLabel("本地时间").fill("17:30");
    await dialog.getByRole("button", { name: "保存变更" }).click();
    await expect.poll(async () => (await list()).reminders[0].time).toBe("17:30");
    await dialog.getByRole("button", { name: "恢复", exact: true }).click();
    await expect(dialog.getByText("启用中", { exact: true })).toBeVisible();
    await page.screenshot({ path: info.outputPath("task-center.png") });
    await dialog.getByRole("button", { name: "取消提醒" }).click();
    await dialog.getByRole("button", { name: "确认取消" }).click();
    await expect.poll(async () => (await list()).reminders[0].state).toBe("cancelled");
    const second = await nativeInvoke<{ id: string }>(page, "create_workspace", { name: "另一个工作区" });
    await expect(nativeInvoke(page, "set_reminder_state", {
      workspaceId: second.id, id: reminder.id, state: "active",
    })).rejects.toThrow();
    expect(await nativeInvoke(page, "list_reminders", { workspaceId: second.id })).toEqual({ reminders: [], runs: [] });
    const start = new Date(Date.now() + 60_000);
    const date = [start.getFullYear(), start.getMonth() + 1, start.getDate()].map(n => String(n).padStart(2, "0")).join("-");
    const time = [start.getHours(), start.getMinutes()].map(n => String(n).padStart(2, "0")).join(":");
    await nativeInvoke(page, "save_reminder", { workspaceId, id: null, input: {
      title: "仅触发一次", kind: "once", date, time, weekday: start.getDay() || 7,
    } });
    await app.closeWindow();
    await app.stop();
    // Seed a past occurrence in the stopped, isolated database to exercise resume after a long absence.
    execFileSync("py.exe", ["-3", "-c", [
      "import sqlite3, sys",
      "with sqlite3.connect(sys.argv[1]) as db:",
      "    db.execute(\"INSERT INTO reminders (id,workspace_id,title,kind,date,time,weekday,state,next_date) VALUES ('missed-sample',?,'错过的提醒','once','2020-01-06','10:00',1,'active','2020-01-06')\", (sys.argv[2],))",
    ].join("\n"), join(options.dataDirectory, "cortexos.sqlite3"), workspaceId], { windowsHide: true });
    app = await launchNative(info, options);
    await expect.poll(async () => (await list()).runs.length, { timeout: 85_000 }).toBe(2);
    const result = await list();
    expect(result.runs[0]).toMatchObject({ title: "仅触发一次", status: "delivered" });
    expect(result.runs[1]).toMatchObject({ title: "错过的提醒", status: "missed" });
    expect(result.reminders.find(row => row.kind === "once")?.state).toBe("completed");
    await app.closeWindow();
    await app.stop();
    app = await launchNative(info, options);
    expect((await list()).runs).toEqual(result.runs);
  } finally {
    await app.stop();
  }
});
