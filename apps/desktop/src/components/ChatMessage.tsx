import { BookOpen, Check, Code2, Copy, Sparkles, X } from "lucide-react";
import { memo, useCallback, useRef, useState, useSyncExternalStore } from "react";
import { knowledge } from "../api/knowledge";
import { memoryApi } from "../api/memory";
import type { ChatStream } from "../chat-stream";
import type { Message, MessageImage } from "../studio";
import { AnswerContent } from "./AnswerContent";
import { AppDialog, openDialog } from "./AppDialog";
import { SelectionPopover } from "./SelectionPopover";
import { notify } from "./Toast";

const statusNames: Record<string, string> = {
  streaming: "生成中",
  completed: "已完成",
  cancelled: "已停止",
  failed: "生成失败",
  interrupted: "生成中断",
};

function deriveCardTitle(text: string): string {
  const firstLine =
    text
      .split("\n")
      .map((line) => line.replace(/^[#>*\-\d.\s]+/, "").trim())
      .find((line) => line.length > 0) ?? "对话摘录";
  const chars = Array.from(firstLine).slice(0, 48).join("").trim();
  return chars || "对话摘录知识卡";
}

function findVerbatimExcerpt(original: string, selected: string): string {
  if (original.includes(selected)) {
    return Array.from(selected).slice(0, 20000).join("");
  }
  const candidates = selected
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length >= 4)
    .sort((a, b) => b.length - a.length);
  for (const line of candidates) {
    if (original.includes(line)) {
      return Array.from(line).slice(0, 20000).join("");
    }
  }
  return Array.from(original).slice(0, 1200).join("");
}

export const ChatMessage = memo(function ChatMessage({
  message: stored,
  stream,
  assistantName,
  located,
  copied,
  copy,
  onSaveKnowledge,
  onQuoteSelection,
  onQuickSavedKnowledge,
  setSelectedRun,
  canRetry,
  reportError,
  retry,
}: {
  message: Message;
  stream: ChatStream;
  assistantName?: string;
  located: boolean;
  copied: boolean;
  copy: (id: string, text: string) => Promise<void>;
  onSaveKnowledge: (message: Message, excerpt: string) => void;
  onQuoteSelection?: (quoteBlock: string) => void;
  onQuickSavedKnowledge?: () => void;
  setSelectedRun: (id: string | null) => void;
  canRetry: boolean;
  reportError: (value: unknown) => void;
  retry: (runId: string | null) => void;
}) {
  const id = stored.id;
  const streaming = stored.status === "streaming";
  const subscribe = useCallback(
    (onStoreChange: () => void) => (streaming ? stream.subscribe(id, onStoreChange) : () => {}),
    [id, stream, streaming],
  );
  const snapshot = useCallback(() => (streaming ? stream.snapshot(id) : undefined), [id, stream, streaming]);
  const update = useSyncExternalStore(subscribe, snapshot);
  const message =
    update && update.runId === stored.modelRunId
      ? {
          ...stored,
          content:
            update.status === "running" && stored.content.startsWith(update.content)
              ? stored.content
              : update.content,
          status: update.status === "running" ? "streaming" : update.status,
        }
      : stored;
  const [raw, setRaw] = useState(false);
  const [previewImage, setPreviewImage] = useState<MessageImage | null>(null);
  const previewDialog = useRef<HTMLDialogElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  const openImagePreview = (img: MessageImage) => {
    setPreviewImage(img);
    requestAnimationFrame(() => openDialog(previewDialog.current));
  };

  return (
    <article
      className={`message ${message.role}`}
      id={`message-${message.id}`}
      tabIndex={-1}
      data-located={located || undefined}
      aria-label={message.role === "user" ? "你的问题" : "模型回答"}
    >
      <div className="message-heading">
        <strong>{message.role === "user" ? "你" : assistantName || "专属助手"}</strong>
        {message.role === "assistant" && (
          <span
            className={`message-status ${message.finishReason === "length" ? "limited" : message.status}`}
          >
            {message.finishReason === "length"
              ? "达到输出上限"
              : (statusNames[message.status] ?? message.status)}
          </span>
        )}
      </div>
      {message.images && message.images.length > 0 && (
        <div
          className="message-images"
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "8px",
            marginBottom: "8px",
          }}
        >
          {message.images.map((img, idx) => (
            <button
              key={idx}
              type="button"
              onClick={() => openImagePreview(img)}
              aria-label={`放大查看第 ${idx + 1} 张图片`}
              title="点击放大查看图片"
              style={{
                padding: 0,
                border: "1px solid var(--line)",
                borderRadius: "8px",
                overflow: "hidden",
                background: "var(--surface-inset, var(--surface))",
                cursor: "zoom-in",
                maxWidth: "220px",
                maxHeight: "152px",
                display: "inline-flex",
              }}
            >
              <img
                src={`data:${img.mimeType};base64,${img.dataBase64}`}
                alt={`消息附图 ${idx + 1}`}
                style={{
                  maxWidth: "220px",
                  maxHeight: "152px",
                  objectFit: "cover",
                  display: "block",
                }}
              />
            </button>
          ))}
        </div>
      )}
      {message.content ? (
        <div ref={contentRef} className="message-selection-scope">
          <AnswerContent message={message} raw={raw} />
          <SelectionPopover
            containerRef={contentRef}
            disabled={message.status === "streaming"}
            onAsk={(selectedText) => {
              const quoteBlock =
                selectedText
                  .split("\n")
                  .slice(0, 16)
                  .map((line) => `> ${line}`)
                  .join("\n") + "\n\n";
              onQuoteSelection?.(quoteBlock);
            }}
            onSaveCard={(selectedText) => {
              const isAssistant = message.role === "assistant";
              const sourceExcerpt = isAssistant
                ? findVerbatimExcerpt(message.content, selectedText)
                : null;
              void knowledge("save_knowledge", {
                input: {
                  workspaceId: message.workspaceId,
                  nodeId: null,
                  expectedRevision: null,
                  title: deriveCardTitle(selectedText),
                  kind: "knowledge",
                  content: Array.from(selectedText).slice(0, 20000).join(""),
                  changeReason: isAssistant ? "从回答划词存为知识卡" : "从对话划词存为知识卡",
                  sourceMessageId: isAssistant ? message.id : null,
                  sourceExcerpt,
                },
              })
                .then(() => {
                  onQuickSavedKnowledge?.();
                  notify(
                    isAssistant
                      ? "已存为知识卡（保留原回答溯源）"
                      : "已存为知识卡并纳入检索",
                  );
                })
                .catch(reportError);
            }}
          />
        </div>
      ) : (
        <div className="message-content">
          {message.status === "streaming" ? "正在准备回答…" : "本次未生成可用文字。"}
        </div>
      )}
      {message.role === "assistant" && message.finishReason === "length" && (
        <p className="output-limit-note">
          已达到长度上限，回答可能不完整。可在设置中增加“回答长度上限（Token）”后重试。
        </p>
      )}
      {message.role === "user" &&
        message.content.trim().length >= 4 &&
        message.content.trim().length <= 240 && (
          <div className="message-actions">
            <button
              className="button text-button"
              onClick={() => {
                void memoryApi("promote_rule_from_chat", { rule: message.content.trim() })
                  .then(() => notify("已沉淀为全局长期规则，后续对话生效"))
                  .catch(reportError);
              }}
            >
              <Sparkles size={14} aria-hidden="true" />
              沉淀为长期规则
            </button>
          </div>
        )}
      {message.role === "assistant" && (
        <div className="message-actions">
          <button
            className="button text-button"
            disabled={!message.content}
            onClick={() => {
              void copy(message.id, message.content);
            }}
          >
            {copied ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
            {copied ? "已复制" : "复制"}
          </button>
          <button
            className="button text-button"
            aria-pressed={raw}
            aria-label={raw ? "显示 Markdown 排版" : "查看 Markdown 原文"}
            onClick={() => setRaw(!raw)}
          >
            <Code2 size={16} aria-hidden="true" />
            {raw ? "排版" : "原文"}
          </button>
          <button
            className="button text-button"
            disabled={!message.content || message.status === "streaming"}
            onClick={(event) => {
              const selection = window.getSelection();
              const article = event.currentTarget.closest("article");
              const selected = selection?.toString() ?? "";
              const inside =
                selection?.anchorNode &&
                selection?.focusNode &&
                article?.querySelector(".message-content")?.contains(selection.anchorNode) &&
                article?.querySelector(".message-content")?.contains(selection.focusNode);
              if (inside && selected && !message.content.includes(selected)) {
                reportError(
                  "所选文字跨越了 Markdown 格式标记。请点击“原文”后重新选择，或直接使用划词悬浮气泡「存为知识卡」。",
                );
                return;
              }
              onSaveKnowledge(
                message,
                Array.from(
                  inside && selected && message.content.includes(selected)
                    ? selected
                    : message.content,
                )
                  .slice(0, 20000)
                  .join(""),
              );
            }}
          >
            <BookOpen size={14} aria-hidden="true" />
            保存为知识
          </button>
          {message.modelRunId && (
            <button className="button text-button" onClick={() => setSelectedRun(message.modelRunId)}>
              参考依据
            </button>
          )}
          {(["failed", "cancelled", "interrupted"].includes(message.status) ||
            message.finishReason === "length") && (
            <button
              className="button text-button"
              disabled={!canRetry}
              onClick={() => retry(message.modelRunId)}
            >
              重试
            </button>
          )}
        </div>
      )}
      <AppDialog
        ref={previewDialog}
        className="form-dialog"
        aria-label="查看图片大图"
        onClose={() => setPreviewImage(null)}
        style={{ maxWidth: "min(90vw, 960px)", padding: "12px" }}
      >
        {previewImage && (
          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <strong style={{ fontSize: "0.9rem" }}>图片预览</strong>
              <button
                type="button"
                className="icon-button"
                aria-label="关闭图片预览"
                onClick={() => previewDialog.current?.close()}
              >
                <X size={18} aria-hidden="true" />
              </button>
            </div>
            <img
              src={`data:${previewImage.mimeType};base64,${previewImage.dataBase64}`}
              alt="消息附图大图"
              style={{
                maxWidth: "100%",
                maxHeight: "78vh",
                objectFit: "contain",
                borderRadius: "8px",
                margin: "0 auto",
              }}
            />
          </div>
        )}
      </AppDialog>
    </article>
  );
});
