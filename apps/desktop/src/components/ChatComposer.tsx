import {
  Box,
  ChevronDown,
  Cloud,
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
import { SelectMenu } from "./SelectMenu";
import type { Studio } from "../studio";
export function ChatComposer({
  studio,
  modelReady,
  modelName,
  cloudAnswer,
  cloudLookup,
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
  modelReady: boolean | undefined;
  modelName: string;
  cloudAnswer: boolean;
  cloudLookup: boolean;
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
  const taskPreview = taskDraft === "briefing" ? "预览新闻简报" : taskDraft === "reminder" ? "预览提醒" : undefined;
  const [searchChoice, setSearchChoice] = useState<{ workspace: string; provider: WebSearchProvider }>();
  const provider = searchConfigured
    ? (searchChoice?.workspace === studio.workspaceId ? searchChoice?.provider : undefined) ?? "brave"
    : "news";
  const scope = `${studio.workspaceId}:${studio.conversationId}`;
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
          <a
            className="composer-model"
            href={navigationHash("settings", { workspace: studio.workspaceId, section: cloudAnswer || cloudLookup ? "profiles" : "model" })}
            title={modelReady ? "打开模型设置" : "模型尚未就绪，打开设置"}
          >
            {cloudAnswer || cloudLookup ? <Cloud size={17} aria-hidden="true" /> : <Box size={17} aria-hidden="true" />}
            <span>{(cloudAnswer || cloudLookup) && <><strong>{cloudAnswer ? "云端回答" : "云端查找"}</strong>{" "}</>}{modelName}</span>
            <ChevronDown size={14} aria-hidden="true" />
          </a>
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
                title={taskPreview}
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
        {studio.desktop && studio.workspaceId && !cloudAnswer && (
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
        <ContextPicker
          key={`${studio.workspaceId}:${studio.conversationId}`}
          studio={studio}
          cloudAnswer={cloudAnswer}
        />
      </div>
      {studio.desktop && studio.workspaceId && !studio.draft && !currentRunning && !studio.sending &&
        !studio.loadingMessages && !studio.creatingConversation && (
          <button className="button secondary reminder-example" type="button" onClick={() => {
            studio.setDraft("十分钟后提醒我喝水");
            textarea.current?.focus();
          }}>试试「十分钟后提醒我喝水」</button>
        )}
      {draftTooLong && <small className="form-error" id="message-length-error">问题超过输入上限。请缩短后再发送。</small>}
      {searchConfigured !== undefined && !!studio.draft.trim() && <small className="inline-help" id="web-query-disclosure">
        联网查询会将本题发送给{webSearchProviderLabel[provider]}。
      </small>}
      {searchConfigured === null && <div className="composer-context">
        <small className="form-error" role="status">无法读取查询服务配置，可显式选择查询公开新闻。</small>
        <button className="button text-button" type="button" onClick={retrySearchStatus}>重新读取配置</button>
      </div>}
      {!!studio.draftPersistence && <small className="inline-help">{studio.draftPersistence === "local" ? "草稿保存在本机" : "草稿只保留在当前窗口"}</small>}
      <details className="composer-help">
        <summary>对话说明与快捷键</summary>
        <p>
          Enter 发送，Shift+Enter 换行。支持 Markdown 标题、列表、代码和表格，点击眼睛图标预览，返回编辑保留原文。
          可输入“十分钟后提醒我喝水”来创建提醒。
          可输入“每个工作日早上九点整理 AI 新闻”来创建定时简报。
          {cloudAnswer ? "发送前可确认要传给云端的内容。" : "系统会在容量允许的范围内选取最近的完整对话，并查找当前工作区已确认的知识；不会直接搜索文件全文。请结合依据判断回答是否可靠。"}
        </p>
        {!studio.desktop && <p>浏览器预览不会读取或保存本地数据。</p>}
      </details>
    </div>
  );
}
