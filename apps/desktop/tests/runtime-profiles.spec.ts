import { expect, test } from "@playwright/test";

test.use({ headless: true });

test("switch a workspace profile and confirm cloud transfer per message", async ({ page }, info) => {
  await page.addInitScript(() => {
    const profileId = "fixture-profile";
    const updatedAt = "2026-11-17T10:00:00Z";
    const settings = { model: "fixture-local", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 };
    const conversation = { id: "c", workspaceId: "w", title: "已有会话", autoRetrieval: true };
    const connection = { id: "provider", label: "测试连接", address: null, provider: "openai",
      probes: [{ model: "fixture-cloud", capability: "generation", state: "verified" }] };
    let profiles: unknown[] = [];
    let activeId: string | null = null;
    let routing = { preference:"preferred", question:{preferred:null,alternative:null}, knowledge:{preferred:null,alternative:null}, localBackup:null } as Record<string, unknown>;
    let requests = 0;
    let finishPreparation: (() => void) | undefined;
    let policy = { singleRunMicros: null as number | null, rollingDayMicros: null as number | null, requestsPerMinute: null as number | null, retryLimit: 0 };
    Object.assign(window, {
      isTauri: true,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: { metadata: { currentWindow: { label: "main" } }, transformCallback: () => 1, unregisterCallback() {},
        invoke: async (command: string, args: { workspaceId?: string; profileId?: string; input?: Record<string, unknown>;
          content?: string; cloudConsent?: Record<string, unknown>; contextSelection?: unknown[]; runId?: string; knowledge?: boolean;
          onPreparing?: { onmessage: (message: { id: string; workspaceId: string; conversationId: string }) => void } }) => {
          if (command === "bootstrap") return { workspaces: [{ id: "w", name: "测试工作区" }], settings, activeRun: null };
          if (command === "check_ollama") return { version: "fixture", models: [] };
          if (command.startsWith("plugin:")) return 1;
          if (command === "get_run") return null;
          if (args.workspaceId !== "w") throw Error("Unscoped command " + command);
          if (command === "list_conversations") return [conversation];
          if (command === "list_knowledge" || command === "load_messages") return [];
          if (command === "rag_index_status") return { index: null };
          if (command === "list_model_runtime_profiles") return { activeId, profiles };
          if (command === "model_chat_routing") {
            if (args.input) {
              if (args.input.preference !== "lower_cost" || (args.input.knowledge as { preferred?: string })?.preferred !== profileId) {
                throw Error("Wrong routing profile");
              }
              routing = args.input;
            }
            return routing;
          }
          if (command === "chat_runtime_route") {
            if (args.knowledge !== true) throw Error("Wrong task classification");
            return { profile: activeId ? profiles[0] : null, decision: { task: "knowledge" }, localBackup: null };
          }
          if (command === "model_runtime_policy") {
            if (args.input) {
              if (args.input.singleRunMicros !== 125_000 || args.input.requestsPerMinute !== 8 || args.input.retryLimit !== 1) {
                throw Error("Incorrect request limits");
              }
              policy = args.input as typeof policy;
            }
            return policy;
          }
          if (command === "list_runtime_cloud_connections") return [connection];
          if (command === "list_model_cloud_knowledge") return {
            grants: [{ nodeId: "approved", version: 3, title: "已授权资料", available: true, grantedAt: updatedAt }], candidates: [],
          };
          if (command === "save_model_runtime_profile") {
            if (args.input?.mode !== "cloud") throw Error("Wrong profile mode");
            const result = { ...args.input, id: profileId, updatedAt };
            profiles = [result];
            return result;
          }
          if (command === "activate_model_runtime_profile") {
            if (args.profileId !== profileId) throw Error("Wrong profile");
            activeId = profileId;
            return;
          }
          if (command === "start_chat") {
            if (args.content !== "只发送本题" || args.cloudConsent?.profileId !== profileId
              || args.cloudConsent.updatedAt !== updatedAt || args.cloudConsent.shareSelection !== false
              || JSON.stringify(args.cloudConsent.grantedVersions) !== JSON.stringify([{ nodeId: "approved", version: 3 }])
              || args.contextSelection?.length !== 0 || ++requests > 2) throw Error("Missing exact consent");
            if (requests === 1) {
              args.onPreparing?.onmessage({ id: "prepared", workspaceId: "w", conversationId: "c" });
              await new Promise<void>(resolve => { finishPreparation = resolve; });
              throw Error("已停止查找");
            }
            return { id: "run", workspaceId: "w", conversationId: "c", assistantMessageId: "m", modelId: "fixture-cloud", status: "running" };
          }
          if (command === "cancel_run") {
            if (args.runId !== "prepared" || !finishPreparation) throw Error("Wrong cancellation scope");
            finishPreparation();
            return;
          }
          throw Error("Unexpected " + command);
        },
      },
    });
  });
  await page.goto("/#/settings?workspace=w");
  await page.getByRole("button", { name: "运行方案", exact: true }).click();
  await page.getByText("云端预算与请求限制", { exact: true }).click();
  await page.getByLabel("单次任务费用上限（美元）", { exact: true }).fill("0.125");
  await page.getByLabel("每个连接每分钟请求上限", { exact: true }).fill("8");
  await page.getByLabel("云端限流后重试次数", { exact: true }).fill("1");
  await page.getByRole("button", { name: "保存请求限制", exact: true }).click();
  await expect(page.getByText("已保存请求限制。", { exact: true })).toBeVisible();
  await page.screenshot({ path: info.outputPath("runtime-budget-settings.png") });
  await page.getByText("云端预算与请求限制", { exact: true }).click();
  await page.getByLabel("选择方案", { exact: true }).selectOption("new");
  await page.getByLabel("方案名称", { exact: true }).fill("项目问答");
  await page.getByLabel("运行模式", { exact: true }).selectOption("cloud");
  await page.getByRole("button", { name: "保存方案", exact: true }).click();
  await page.getByRole("button", { name: "启用方案", exact: true }).click();
  await expect(page.getByText(/当前方案.*项目问答/)).toBeVisible();
  await page.getByText("任务路由与备用模型", { exact: true }).click();
  await page.getByLabel("路由偏好", { exact: true }).selectOption("lower_cost");
  await page.getByLabel("知识问答首选方案", { exact: true }).selectOption("fixture-profile");
  await page.getByRole("button", { name: "保存任务路由", exact: true }).click();
  await expect(page.getByText("已保存任务路由。", { exact: true })).toBeVisible();
  await page.setViewportSize({ width: 1366, height: 768 });
  await page.screenshot({ path: info.outputPath("profiles-1366.png") });

  await page.getByRole("link", { name: "对话", exact: true }).click();
  await expect(page.getByText("云端回答", { exact: true })).toBeVisible();
  const draft = page.getByRole("textbox", { name: "你的问题" });
  await draft.fill("只发送本题");
  await page.getByRole("button", { name: "发送消息", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "确认发送到云端" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText(/OpenAI.*fixture-cloud/)).toBeVisible();
  await dialog.getByRole("checkbox", { name: "查找并发送此连接已获授权的相关知识" }).check();
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(draft).toHaveValue("只发送本题");
  await page.getByRole("button", { name: "发送消息", exact: true }).click();
  const grant = dialog.getByRole("checkbox", { name: "查找并发送此连接已获授权的相关知识" });
  await expect(grant).not.toBeChecked();
  await grant.check();
  await page.getByRole("combobox", { name: "外观", exact: true }).selectOption("dark");
  await page.setViewportSize({ width: 1920, height: 1080 });
  await page.screenshot({ path: info.outputPath("cloud-consent-dark-1920.png") });
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  await page.getByRole("button", { name: "停止查找", exact: true }).click();
  await expect(draft).toHaveValue("只发送本题");
  await page.getByRole("button", { name: "发送消息", exact: true }).click();
  await grant.check();
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  await expect(page.getByRole("button", { name: "停止生成", exact: true })).toBeVisible();
  await expect(draft).toHaveValue("");
});
