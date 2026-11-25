import { expect, test } from "@playwright/test";

test.use({ headless: true });

test("switch a workspace profile and confirm cloud transfer per message", async ({ page }, info) => {
  await page.addInitScript(() => {
    const profileId = "fixture-profile";
    const updatedAt = "2026-11-17T10:00:00Z";
    const settings = { model: "fixture-local", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 };
    const conversation = { id: "c", workspaceId: "w", title: "已有会话", autoRetrieval: true };
    const connection = { id: "provider", label: "测试连接", address: "https://api.deepseek.com", provider: "compatible",
      probes: [{ model: "fixture-cloud", capability: "generation", state: "verified" }] };
    const secondConnection = { id: "second-provider", label: "备用测试连接",
      address: "https://dashscope.aliyuncs.com/compatible-mode/v1", provider: "compatible",
      probes: [{ model: "fixture-bailian", capability: "generation", state: "verified" }] };
    const secondProfile = {
      id: "second-profile", name: "百炼问答", updatedAt, mode: "cloud",
      generation: { kind: "cloud", connectionId: secondConnection.id, model: "fixture-bailian" },
      embedding: { kind: "off" }, reranking: { kind: "off" },
    };
    let profiles: { id: string }[] = [secondProfile];
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
          content?: string; conversationId?: string; cloudConsent?: Record<string, unknown>; contextSelection?: unknown[]; runId?: string; knowledge?: boolean;
          onPreparing?: { onmessage: (message: { id: string; workspaceId: string; conversationId: string }) => void } }) => {
          if (command === "bootstrap") return { workspaces: [{ id: "w", name: "测试工作区" }], settings, activeRun: null };
          if (command === "check_ollama") return { version: "fixture", models: [] };
          if (command === "sync_workspace_watched_folders") return [];
          if (command === "read_assistant") return { activeVersion: null, versions: [] };
          if (command.startsWith("plugin:")) return 1;
          if (command === "get_run") return null;
          if (args.workspaceId !== "w") throw Error("Unscoped command " + command);
          if (command === "read_web_search_status") return { configured: false };
          if (command === "model_cloud_privacy_rules") return { syncMemoryAndHistory: false };
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
            return { profile: profiles.find(profile => profile.id === activeId) ?? null, decision: { task: "knowledge" }, localBackup: null };
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
          if (command === "list_runtime_cloud_connections") return [connection, secondConnection];
          if (command === "list_model_cloud_knowledge") return {
            grants: [{ nodeId: "approved", version: 3, title: "已授权资料", available: true, grantedAt: updatedAt }], candidates: [],
          };
          if (command === "save_model_runtime_profile") {
            if (args.input?.mode !== "cloud") throw Error("Wrong profile mode");
            const result = { ...args.input, id: profileId, updatedAt };
            profiles = [result, secondProfile];
            return result;
          }
          if (command === "activate_model_runtime_profile") {
            if (!profiles.some(profile => profile.id === args.profileId)) throw Error("Wrong profile");
            activeId = args.profileId!;
            return;
          }
          if (command === "start_chat") {
            const questions = ["只发送本题", "只发送本题", "同一会话的下一题", "切换方案后的问题"];
            const expectedProfile = requests < 3 ? profileId : secondProfile.id;
            if (args.content !== questions[requests] || args.conversationId !== conversation.id
              || args.cloudConsent?.profileId !== expectedProfile
              || args.cloudConsent.updatedAt !== updatedAt || args.cloudConsent.shareSelection !== false
              || JSON.stringify(args.cloudConsent.grantedVersions) !== JSON.stringify(requests < 2 ? [{ nodeId: "approved", version: 3 }] : [])
              || args.contextSelection?.length !== 0) throw Error("Missing exact consent");
            requests++;
            if (requests === 1) {
              args.onPreparing?.onmessage({ id: "prepared", workspaceId: "w", conversationId: "c" });
              await new Promise<void>(resolve => { finishPreparation = resolve; });
              throw Error("已停止查找");
            }
            return { id: "run-" + requests, workspaceId: "w", conversationId: "c", assistantMessageId: "m",
              modelId: requests < 4 ? "fixture-cloud" : "fixture-bailian", status: "running" };
          }
          if (command === "get_chat_run_status") return "completed";
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
  await expect(page.getByRole("combobox", { name: "回答模型", exact: true }).locator("option:checked")).toHaveText(/^自动 · fixture-cloud/);
  const draft = page.getByRole("textbox", { name: "你的问题" });
  await draft.fill("只发送本题");
  await page.getByRole("button", { name: "发送消息", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "确认发送到云端" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText(/DeepSeek 官方.*fixture-cloud.*https:\/\/api\.deepseek\.com/)).toBeVisible();
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
  await expect(dialog.getByText(/DeepSeek 官方.*fixture-cloud.*https:\/\/api\.deepseek\.com/)).toBeVisible();
  await grant.check();
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  await expect(page.getByRole("button", { name: "停止生成", exact: true })).toBeVisible();
  await expect(draft).toHaveValue("");
  const send = page.getByRole("button", { name: "发送消息", exact: true });
  await expect(send).toBeVisible();
  await draft.fill("同一会话的下一题");
  await send.click();
  await expect(dialog.getByText(/DeepSeek 官方.*fixture-cloud.*https:\/\/api\.deepseek\.com/)).toBeVisible();
  await expect(grant).not.toBeChecked();
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  await expect(draft).toHaveValue("");
  await expect(send).toBeVisible();

  await page.getByRole("link", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "运行方案", exact: true }).click();
  await page.getByLabel("选择方案", { exact: true }).selectOption("second-profile");
  await page.getByRole("button", { name: "启用方案", exact: true }).click();
  await expect(page.getByText(/当前方案.*百炼问答/)).toBeVisible();
  await page.getByRole("link", { name: "对话", exact: true }).click();
  await draft.fill("切换方案后的问题");
  await send.click();
  await expect(dialog.getByText(/阿里云百炼（北京）.*fixture-bailian.*https:\/\/dashscope\.aliyuncs\.com\/compatible-mode\/v1/)).toBeVisible();
  await expect(dialog.getByText(/DeepSeek 官方/)).toHaveCount(0);
  await page.screenshot({ path: info.outputPath("bailian-chat-consent.png") });
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  await expect(draft).toHaveValue("");
});

test("choose a verified cloud model from chat while Ollama is offline", async ({ page }, info) => {
  const target = { kind: "cloud", connectionId: "cloud", model: "verified-chat" };
  const updatedAt = "2026-11-24T00:00:00Z";
  const connectionName = "云端验收连接";
  const browserErrors: string[] = [];
  page.on("pageerror", error => browserErrors.push(error.message));
  await page.addInitScript(({ target, updatedAt, connectionName }) => {
    type Request = {
      workspaceId: string;
      conversationId: string;
      generationTarget?: typeof target;
      cloudConsent?: object;
      contextSelection?: unknown[];
      content?: string;
      runId?: string;
      onPreparing?: { onmessage: (message: object) => void };
      onUpdate?: { onmessage: (message: object) => void };
    };
    const conversations = [
      { id: "first", workspaceId: "w", title: "原会话", autoRetrieval: false, updatedAt },
      { id: "next", workspaceId: "w", title: "另一会话", autoRetrieval: false, updatedAt },
      { id: "isolated", workspaceId: "other", title: "隔离会话", autoRetrieval: false, updatedAt },
    ];
    const activity = { sends: [] as object[], cancels: [] as object[], unexpected: [] as string[], syncs: [] as string[] };
    let finishPreparation: (() => void) | undefined;
    const messages: object[] = [];
    Object.assign(window, {
      isTauri: true,
      modelChoiceActivity: activity,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: {
        metadata: { currentWindow: { label: "main" } },
        transformCallback: () => 1,
        unregisterCallback() {},
        invoke: async (command: string, args: Request) => {
          if (command === "sync_workspace_watched_folders") {
            activity.syncs.push(args.workspaceId);
            return [];
          }
          if (command === "read_assistant") return { activeVersion: null, versions: [] };
          if (command.startsWith("plugin:")) return 1;
          if (command === "restore_on_start") return { phase: "idle" };
          if (command === "bootstrap") return {
            workspaces: [{ id: "w", name: "选模验收" }, { id: "other", name: "隔离工作区" }],
            settings: { model: "offline-local", numCtx: 4096, numPredict: 512, temperature: .3, timeoutSeconds: 180 },
            activeRun: null,
          };
          if (command === "check_ollama") throw Error("模拟 Ollama 离线");
          if (command === "list_conversations") return conversations.filter(item => item.workspaceId === args.workspaceId);
          if (command === "load_messages") return args.workspaceId === "w" && args.conversationId === "next" ? messages : [];
          if (command === "list_memory_candidates") return [];
          if (command === "read_web_search_status") return { configured: false };
          if (command === "model_cloud_privacy_rules") return { syncMemoryAndHistory: false };
          if (command === "get_run") return null;
          if (command === "rag_index_status") return { index: null };
          if (command === "list_runtime_cloud_connections") return args.workspaceId === "w" ? [{
            id: target.connectionId, label: connectionName, provider: "compatible", address: "https://mock.example",
            credentialAvailable: true,
            probes: [
              { model: target.model, capability: "generation", state: "verified" },
              { model: "unverified-chat", capability: "generation", state: "failed" },
              { model: "embed-only", capability: "embedding", state: "verified" },
            ],
          }] : [];
          if (command === "list_model_cloud_knowledge") return { grants: [] };
          if (command === "chat_runtime_route") {
            const selected = args.workspaceId === "w" && args.generationTarget?.connectionId === target.connectionId &&
              args.generationTarget?.kind === target.kind && args.generationTarget?.model === target.model;
            if (args.generationTarget && !selected) activity.unexpected.push("Unexpected route target");
            return {
              profile: selected ? {
                id: "manual-generation", updatedAt, name: "手动选择回答模型", mode: "cloud",
                generation: target, embedding: { kind: "off" }, reranking: { kind: "off" },
              } : null,
              localBackup: null,
              decision: { task: "question", reason: selected ? "manual_generation" : "default" },
            };
          }
          if (command === "start_chat") {
            const { workspaceId, conversationId, generationTarget, cloudConsent, content, contextSelection } = args;
            activity.sends.push({ workspaceId, conversationId, generationTarget, cloudConsent, content, contextSelection });
            const run = { id: "fixture-run", workspaceId, conversationId, assistantMessageId: "fixture-answer", modelId: target.model, status: "running" };
            if (content === "准备后停止") {
              args.onPreparing?.onmessage({ id: run.id, workspaceId, conversationId });
              await new Promise<void>(resolve => { finishPreparation = resolve; });
              throw Error("已停止查找");
            }
            if (content === "准备时失败") throw Error("模拟准备失败");
            const answer = "模拟云端回答已完成。";
            messages.push({
              id: run.assistantMessageId, workspaceId, conversationId, role: "assistant", content: answer,
              status: "completed", modelRunId: run.id, finishReason: "stop", createdAt: updatedAt,
            });
            args.onUpdate?.onmessage({
              runId: run.id, messageId: run.assistantMessageId, workspaceId, conversationId, seq: 1,
              content: answer, status: "completed", errorMessage: null,
            });
            return run;
          }
          if (command === "cancel_run" && finishPreparation) {
            activity.cancels.push({ workspaceId: args.workspaceId, runId: args.runId });
            finishPreparation();
            finishPreparation = undefined;
            return;
          }
          activity.unexpected.push(command);
          throw Error("Unexpected mock command " + command);
        },
      },
    });
  }, { target, updatedAt, connectionName });

  const activity = () => page.evaluate(() => (window as unknown as {
    modelChoiceActivity: { sends: unknown[]; cancels: unknown[]; unexpected: string[]; syncs: string[] };
  }).modelChoiceActivity);
  const choice = page.getByRole("combobox", { name: "回答模型", exact: true });
  const send = page.getByRole("button", { name: "发送消息", exact: true });
  const draft = page.getByRole("textbox", { name: "你的问题", exact: true });
  const dialog = page.getByRole("dialog", { name: "确认发送到云端", exact: true });
  const pickCloud = () => choice.selectOption({ label: target.model + " · " + connectionName });
  const pickAutomatic = () => choice.selectOption({ index: 0 });
  const openConversation = (workspace: string, conversation: string) =>
    page.evaluate(([w, c]) => { location.hash = `/chat?workspace=${w}&conversation=${c}`; }, [workspace, conversation]);

  await page.goto("/#/chat?workspace=w&conversation=first");
  await expect(choice.locator("option:checked")).toHaveText(/自动/);
  await expect(choice.getByRole("option")).toHaveText([/自动/, target.model + " · " + connectionName]);
  await draft.fill("准备后停止");
  await expect(send).toBeDisabled();
  await pickCloud();
  await expect(send).toBeEnabled();
  for (const theme of ["light", "dark"]) {
    await page.getByRole("combobox", { name: "外观", exact: true }).selectOption(theme);
    await expect(page.locator("html")).toHaveAttribute("data-color-scheme", theme);
    for (const [width, height] of [[1366, 768], [1600, 900], [1920, 1080]]) {
      await page.setViewportSize({ width, height });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: info.outputPath(`chat-model-choice-${theme}-${width}.png`) });
      await send.click();
      await expect(dialog.getByText(/verified-chat.*https:\/\/mock\.example/)).toBeVisible();
      await page.screenshot({ path: info.outputPath(`chat-model-consent-${theme}-${width}.png`) });
      await dialog.getByRole("button", { name: "取消", exact: true }).click();
      await expect(dialog).toBeHidden();
    }
  }
  expect((await activity()).sends).toEqual([]);
  await expect(draft).toHaveValue("准备后停止");
  await send.click();
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  const stop = page.getByRole("button", { name: "停止查找", exact: true });
  await expect(stop).toBeVisible();
  await expect(choice).toBeDisabled();
  await page.screenshot({ path: info.outputPath("chat-model-preparing.png") });
  await stop.click();
  await expect(draft).toHaveValue("准备后停止");
  await expect(send).toBeEnabled();
  await pickAutomatic();
  await expect(send).toBeDisabled();

  await pickCloud();
  await draft.fill("准备时失败");
  await send.click();
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  await expect(page.getByText("模拟准备失败", { exact: true }).first()).toBeVisible();
  await expect(draft).toHaveValue("准备时失败");
  await page.screenshot({ path: info.outputPath("chat-model-failure.png") });
  await pickAutomatic();
  await expect(send).toBeDisabled();

  await pickCloud();
  await send.click();
  await expect(dialog).toBeVisible();
  await openConversation("w", "next");
  await expect(page.locator(".conversation-title")).toHaveText("另一会话");
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  await expect(dialog).toBeHidden();
  expect((await activity()).sends).toHaveLength(2);
  await expect(choice.locator("option:checked")).toHaveText(target.model + " · " + connectionName);
  await draft.fill("普通云端问答");
  await send.click();
  await dialog.getByRole("button", { name: "确认发送", exact: true }).click();
  await expect(page.getByText("模拟云端回答已完成。", { exact: true })).toBeVisible();
  await expect(draft).toHaveValue("");
  await page.screenshot({ path: info.outputPath("chat-model-cloud-answer.png") });

  await openConversation("other", "isolated");
  await expect(page.getByLabel("当前工作区", { exact: true })).toHaveValue("other");
  await expect(choice.getByRole("option")).toHaveText([/自动/]);
  await draft.fill("隔离工作区草稿");
  await expect(send).toBeDisabled();
  await page.setViewportSize({ width: 375, height: 812 });
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
  await page.screenshot({ path: info.outputPath("chat-model-offline-narrow.png") });
  await openConversation("w", "first");
  await expect(choice.locator("option:checked")).toHaveText(target.model + " · " + connectionName);
  await expect(draft).toHaveValue("准备时失败");
  await expect(send).toBeEnabled();
  await pickAutomatic();
  await expect(send).toBeDisabled();
  expect(await activity()).toEqual({
    sends: [
      ["first", "准备后停止"],
      ["first", "准备时失败"],
      ["next", "普通云端问答"],
    ].map(([conversationId, content]) => ({
      workspaceId: "w", conversationId, content, generationTarget: target, contextSelection: [],
      cloudConsent: {
        profileId: "manual-generation", updatedAt, shareSelection: false, grantedVersions: [], rankingVersions: [],
      },
    })),
    cancels: [{ workspaceId: "w", runId: "fixture-run" }],
    syncs: ["w", "other", "w"],
    unexpected: [],
  });
  expect(browserErrors).toEqual([]);
});
