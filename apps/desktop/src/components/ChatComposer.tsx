import {
  Eye,
  Globe,
  HardDrive,
  Pencil,
  Send,
  Square,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { MarkdownContent } from "./MarkdownContent";
import { ContextPicker } from "../ContextPicker";
import { navigationHash } from "../navigation";
import { webSearchProviderLabel, type WebSearchProvider } from "../api/web-search";
import { assistant, type AssistantSettings } from "../api/assistant";
import { notify } from "./Toast";
import { cloudPresetForAddress } from "../cloud-presets";
import { SelectMenu } from "./SelectMenu";
import type { Studio } from "../studio";
import { cloudConnectionLabel, type GenerationTarget, type RuntimeConnection } from "../runtime-profiles";
export function ChatComposer({
  studio,
  modelName,
  generationTarget,
  chooseGeneration,
  connections,
  modelError,
  retryModels,
  cloudAnswer,
  cloudLookup,
  cloudSyncDefault,
  currentRunning,
  preparing,
  cancelling,
  setCancelling,
  canSend,
  taskDraft,
  draftTooLong,
  searchConfigured,
  retrySearchStatus,
  searching,
  searchWeb,
  send,
}: {
  studio: Studio;
  modelName: string;
  generationTarget?: GenerationTarget;
  chooseGeneration: (target?: GenerationTarget) => void;
  connections: RuntimeConnection[];
  modelError?: string;
  retryModels: () => void;
  cloudAnswer: boolean;
  cloudLookup: boolean;
  cloudSyncDefault: boolean;
  currentRunning: boolean;
  preparing: boolean;
  cancelling: boolean;
  setCancelling: (value: boolean) => void;
  canSend: boolean;
  taskDraft: "briefing" | "reminder" | null;
  draftTooLong: boolean;
  searchConfigured: boolean | null | undefined;
  retrySearchStatus: () => void;
  searching: boolean;
  searchWeb: (provider: WebSearchProvider) => void;
  send: (content?: string) => Promise<void>;
}) {
  const [previewKey, setPreviewKey] = useState<string | null>(null);
  const [assistantState, setAssistantState] = useState<AssistantSettings | null>(null);
  const [switchingAssistant, setSwitchingAssistant] = useState(false);
  const taskPreview = taskDraft === "briefing" ? "预览新闻简报" : taskDraft === "reminder" ? "预览提醒" : undefined;
  const [searchChoice, setSearchChoice] = useState<{ workspace: string; provider: WebSearchProvider }>();
  const provider = searchConfigured
    ? (searchChoice?.workspace === studio.workspaceId ? searchChoice?.provider : undefined) ?? "brave"
    : "news";
  const scope = `${studio.workspaceId}:${studio.conversationId}`;
  const selectedModel = generationTarget ? JSON.stringify(generationTarget) : "";
  const localModels = studio.health?.models.map(({ name }) => {
    const target: GenerationTarget = { kind: "local", model: name };
    return { target, label: name, key: JSON.stringify(target) };
  }) ?? [];
  const cloudModels = connections.flatMap(connection => {
    if (connection.credentialAvailable === false || connection.catalogState === "invalid_credential") return [];
    const seen = new Set<string>();
    const items: { target: GenerationTarget; label: string; key: string }[] = [];
    const connLabel = cloudConnectionLabel(connection);
    const pushModel = (model: string) => {
      if (!model || seen.has(model)) return;
      seen.add(model);
      const target: GenerationTarget = { kind: "cloud", connectionId: connection.id, model };
      items.push({ target, label: model + " · " + connLabel, key: JSON.stringify(target) });
    };
    for (const probe of connection.probes) {
      if (probe.capability === "generation" && probe.state === "verified") pushModel(probe.model);
    }
    for (const item of connection.models ?? []) {
      if (item.generation !== "unsupported") pushModel(item.id);
    }
    if (items.length === 0) {
      const preset = connection.provider === "compatible" ? cloudPresetForAddress(connection.address) : undefined;
      for (const fallback of preset?.defaultModels ?? []) pushModel(fallback);
    }
    return items;
  });
  const options = [...localModels, ...cloudModels];
  const preview = previewKey === scope;
  const textarea = useRef<HTMLTextAreaElement>(null);
  const previewArea = useRef<HTMLDivElement>(null);
  const previousPreview = useRef(false);
  const composing = useRef(false);
  const compositionEndedAt = useRef(-Infinity);
  useEffect(() => {
    if (previousPreview.current !== preview) {
      (preview ? previewArea.current : textarea.current)?.focus();
      previousPreview.current = preview;
    }
  }, [preview]);
  useEffect(() => {
    const ws = studio.workspaceId;
    if (!studio.desktop || !ws) {
      setAssistantState(null);
      return;
    }
    let active = true;
    void assistant("read_assistant", { workspaceId: ws })
      .then((res) => { if (active) setAssistantState(res); })
      .catch(() => { if (active) setAssistantState(null); });
    return () => { active = false; };
  }, [studio.desktop, studio.workspaceId]);
  return (
    <div className="composer-area">
      <div className="composer">
        <label className="sr-only" htmlFor="message">
          你的问题
        </label>
        <textarea
          ref={textarea}
          hidden={preview}
          id="message"
          rows={1}
          aria-invalid={draftTooLong || undefined}
          aria-describedby={draftTooLong ? "message-length-error" : undefined}
          value={studio.draft}
          onChange={(event) => studio.setDraft(event.target.value)}
          onCompositionStart={() => { composing.current = true; }}
          onCompositionEnd={() => {
            composing.current = false;
            compositionEndedAt.current = performance.now();
          }}
          placeholder={
            studio.workspaceId ? "输入问题…" : "创建工作区后，在这里输入问题…"
          }
          disabled={!studio.desktop || !studio.workspaceId}
          readOnly={studio.sending || studio.creatingConversation || studio.loadingMessages}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              // Let the IME consume its own Enter; suppress only the subsequent send.
              if (composing.current || event.nativeEvent.isComposing || event.keyCode === 229) return;
              event.preventDefault();
              if (!event.repeat && performance.now() - compositionEndedAt.current >= 500 && canSend) void send();
            }
          }}
        />
        {preview && (
          <div
            ref={previewArea}
            className="composer-preview markdown-content"
            tabIndex={0}
            role="region"
            aria-label="Markdown 预览"
          >
            {studio.draft ? (
              <MarkdownContent content={studio.draft} />
            ) : (
              <p>输入内容后可预览排版。</p>
            )}
          </div>
        )}
        <div className="composer-toolbar">
          <div className="composer-model">
            <SelectMenu
              aria-label="回答模型"
              title={modelName}
              value={selectedModel}
              disabled={!studio.desktop || !studio.workspaceId || !!studio.activeRun || currentRunning || studio.sending}
              onChange={event => chooseGeneration(options.find(item => item.key === event.target.value)?.target)}
            >
              <option value="">{generationTarget ? "自动" : `自动 · ${modelName}`}</option>
              {selectedModel && !options.some(item => item.key === selectedModel) &&
                <option value={selectedModel} disabled>{generationTarget?.model}（不可用）</option>}
              {cloudModels.length > 0 && <optgroup label="云端模型">
                {cloudModels.map(item => <option key={item.key} value={item.key}>{item.label}</option>)}
              </optgroup>}
              {localModels.length > 0 && <optgroup label="本机模型">
                {localModels.map(item => <option key={item.key} value={item.key}>{item.label}</option>)}
              </optgroup>}
            </SelectMenu>
            {studio.desktop && studio.workspaceId && assistantState && assistantState.versions.length > 0 && (
              <SelectMenu
                aria-label="助手分身"
                title="快速切换当前工作区的助手分身"
                value={assistantState.activeVersion === null ? "" : String(assistantState.activeVersion)}
                disabled={switchingAssistant || !!studio.activeRun || currentRunning || studio.sending}
                onChange={event => {
                  const ws = studio.workspaceId;
                  if (!ws) return;
                  const nextVersion = event.target.value === "" ? null : Number(event.target.value);
                  setSwitchingAssistant(true);
                  void assistant("activate_assistant", { workspaceId: ws, version: nextVersion })
                    .then(() => {
                      setAssistantState(prev => prev ? { ...prev, activeVersion: nextVersion } : prev);
                      const label = nextVersion === null
                        ? "内置默认助手"
                        : assistantState.versions.find(v => v.version === nextVersion)?.name || `分身 v${nextVersion}`;
                      notify(`已切换助手分身：${label}`);
                    })
                    .catch(studio.reportError)
                    .finally(() => setSwitchingAssistant(false));
                }}
              >
                <option value="">默认助手</option>
                {assistantState.versions.map(v => (
                  <option key={v.version} value={String(v.version)}>
                    {v.name || `分身 v${v.version}`}
                  </option>
                ))}
              </SelectMenu>
            )}
          </div>
          <div className="composer-actions">
            {studio.desktop && studio.workspaceId && <button
              type="button"
              className="button text-button"
              disabled={searchConfigured === undefined || !studio.draft.trim() || currentRunning || studio.sending || studio.loadingMessages || searching}
              aria-describedby={studio.draft.trim() ? "web-query-disclosure" : undefined}
              title={`将本题发送给${webSearchProviderLabel[provider]}`}
              onClick={() => { setPreviewKey(null); searchWeb(provider); }}
            ><Globe size={16} aria-hidden="true" />{searchConfigured === null ? "查询公开新闻" : "联网查询"}</button>}
            <button
              type="button"
              className="icon-button"
              aria-label={preview ? "返回 Markdown 编辑" : "预览 Markdown"}
              title={preview ? "返回编辑" : "预览 Markdown"}
              aria-pressed={preview}
              disabled={
                !studio.desktop ||
                !studio.workspaceId ||
                studio.sending ||
                studio.loadingMessages
              }
              onClick={() => setPreviewKey(preview ? null : scope)}
            >
              {preview ? (
                <Pencil size={17} aria-hidden="true" />
              ) : (
                <Eye size={17} aria-hidden="true" />
              )}
            </button>
            {currentRunning ? (
              <button
                className="button secondary stop-button"
                disabled={cancelling}
                onClick={() => {
                  setCancelling(true);
                  void studio.cancel().catch((value) => {
                    setCancelling(false);
                    studio.reportError(value);
                  });
                }}
              >
                <Square size={13} aria-hidden="true" />
                {cancelling ? "正在停止…" : preparing ? "停止查找" : "停止生成"}
              </button>
            ) : (
              <button
                className="send-button"
                disabled={!canSend}
                onClick={() => {
                  if (!canSend) return;
                  setPreviewKey(null);
                  if (!preview) textarea.current?.focus({ preventScroll: true });
                  void send();
                }}
                aria-label={studio.sending ? "正在发送" : taskPreview ?? "发送消息"}
                title={canSend ? (taskPreview ?? "发送消息（Enter）") : "请先选择可用模型并输入问题"}
              >
                <Send size={19} aria-hidden="true" />
              </button>
            )}
          </div>
        </div>
      </div>
      <div className="composer-context">
        {searchConfigured && studio.workspaceId && <SelectMenu aria-label="查询服务" value={provider} disabled={searching}
          onChange={event => {
            if (studio.workspaceId) setSearchChoice({
              workspace: studio.workspaceId, provider: event.target.value === "brave" ? "brave" : "news",
            });
          }}>
          <option value="news">{webSearchProviderLabel.news}</option>
          <option value="brave">{webSearchProviderLabel.brave}</option>
        </SelectMenu>}
        {studio.desktop && studio.workspaceId && (!cloudAnswer || cloudSyncDefault) && (
          <label className="retrieval-switch">
            <input
              type="checkbox"
              checked={studio.autoRetrieval}
              disabled={
                studio.savingRetrieval ||
                studio.sending ||
                !!studio.activeRun ||
                studio.loadingMessages ||
                studio.creatingConversation
              }
              onChange={(event) => {
                void studio.setAutoRetrieval(event.target.checked);
              }}
            />
            {studio.savingRetrieval ? "正在保存…" : "自动查找知识"}
          </label>
        )}
        {!cloudAnswer && <span className="local-label"><HardDrive size={15} aria-hidden="true" />本地</span>}
        {(cloudAnswer || cloudLookup) && <span>{cloudAnswer ? "云端回答" : "云端查找"}</span>}
        <ContextPicker
          key={`${scope}:${selectedModel}`}
          studio={studio}
          cloudAnswer={cloudAnswer}
          generationTarget={generationTarget}
        />
      </div>
      {draftTooLong && <small className="form-error" id="message-length-error">问题超过输入上限。请缩短后再发送。</small>}
      {modelError && <div className="composer-context">
        <small className="form-error" role="status">{modelError}</small>
        <button className="button text-button" type="button" onClick={retryModels}>重新读取模型</button>
      </div>}
      {searchConfigured !== undefined && !!studio.draft.trim() && <small className="inline-help" id="web-query-disclosure">
        联网查询会将本题发送给{webSearchProviderLabel[provider]}。
      </small>}
      {searchConfigured === null && <div className="composer-context">
        <small className="form-error" role="status">无法读取查询服务配置，可显式选择查询公开新闻。</small>
        <button className="button text-button" type="button" onClick={retrySearchStatus}>重新读取配置</button>
      </div>}
      {!!studio.draftPersistence && <small className="inline-help">{studio.draftPersistence === "local" ? "草稿保存在本机" : "草稿只保留在当前窗口"}</small>}
      <details className="composer-help">
        <summary title="展开查看对话说明与常用快捷键">对话说明与快捷键</summary>
        <p>
          Enter 发送，Shift+Enter 换行。支持 Markdown 标题、列表、代码和表格，点击眼睛图标预览，返回编辑保留原文。
          可输入“十分钟后提醒我喝水”来创建提醒。
          可输入“每个工作日早上九点整理 AI 新闻”来创建定时简报。
          {cloudAnswer
            ? cloudSyncDefault ? "云端回答会按当前同步设置使用对话与记忆。自动查找知识可在此关闭。" : "发送前可确认要传给云端的内容。"
            : "系统会在容量允许的范围内选取最近的完整对话，并查找当前工作区已确认的知识；不会直接搜索文件全文。请结合依据判断回答是否可靠。"}
        </p>
        {studio.workspaceId && <a href={navigationHash("settings", { workspace: studio.workspaceId, section: cloudAnswer ? "profiles" : "model" })}>模型设置</a>}
        {!studio.desktop && <p>浏览器预览不会读取或保存本地数据。</p>}
      </details>
    </div>
  );
}
