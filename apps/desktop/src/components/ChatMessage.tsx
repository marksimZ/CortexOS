import { BookOpen, Check, Code2, Copy, GitBranch } from "lucide-react";
import { memo, useCallback, useState, useSyncExternalStore } from "react";
import type { ChatStream } from "../chat-stream";
import { navigate } from "../navigation";
import type { Message } from "../studio";
import { AnswerContent } from "./AnswerContent";
const statusNames: Record<string, string> = {
  streaming: "生成中",
  completed: "已完成",
  cancelled: "已停止",
  failed: "生成失败",
  interrupted: "生成中断",
};
export const ChatMessage = memo(function ChatMessage({
  message: stored,
  stream,
  located,
  copied,
  copy,
  onSaveKnowledge,
  setSelectedRun,
  canRetry,
  reportError,
  retry,
}: {
  message: Message;
  stream: ChatStream;
  located: boolean;
  copied: boolean;
  copy: (id: string, text: string) => Promise<void>;
  onSaveKnowledge: (message: Message, excerpt: string) => void;
  setSelectedRun: (id: string | null) => void;
  canRetry: boolean;
  reportError: (value: unknown) => void;
  retry: (runId: string | null) => void;
}) {
  const id = stored.id;
  const streaming = stored.status === "streaming";
  const subscribe = useCallback((notify: () => void) => streaming ? stream.subscribe(id, notify) : () => {}, [id, stream, streaming]);
  const snapshot = useCallback(() => streaming ? stream.snapshot(id) : undefined, [id, stream, streaming]);
  const update = useSyncExternalStore(subscribe, snapshot);
  const message = update && update.runId === stored.modelRunId ? {
    ...stored,
    content: update.status === "running" && stored.content.startsWith(update.content)
      ? stored.content : update.content,
    status: update.status === "running" ? "streaming" : update.status,
  } : stored;
  const [raw, setRaw] = useState(false);
  return (
    <article
      className={`message ${message.role}`}
      id={`message-${message.id}`}
      tabIndex={-1}
      data-located={located || undefined}
      aria-label={message.role === "user" ? "你的问题" : "模型回答"}
    >
      <div className="message-heading">
        <strong>{message.role === "user" ? "你" : "本地助手"}</strong>
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
      {message.content ? (
        <AnswerContent message={message} raw={raw} />
      ) : (
        <div className="message-content">
          {message.status === "streaming"
            ? "正在准备回答…"
            : "本次未生成可用文字。"}
        </div>
      )}
      {message.role === "assistant" && message.finishReason === "length" && (
        <p className="output-limit-note">
          已达到长度上限，回答可能不完整。可在设置中增加“回答长度上限（Token）”后重试。
        </p>
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
            {copied ? (
              <Check size={14} aria-hidden="true" />
            ) : (
              <Copy size={14} aria-hidden="true" />
            )}
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
                article
                  ?.querySelector(".message-content")
                  ?.contains(selection.anchorNode) &&
                article
                  ?.querySelector(".message-content")
                  ?.contains(selection.focusNode);
              if (inside && selected && !message.content.includes(selected)) {
                reportError(
                  "所选文字跨越了 Markdown 格式标记。请点击“原文”后重新选择，或取消文字选择以保存整条回答。",
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
            <>
              <button
                className="button text-button"
                onClick={() => setSelectedRun(message.modelRunId)}
              >
                <GitBranch size={14} aria-hidden="true" />
                执行过程
              </button>
              <button
                className="button text-button"
                onClick={() =>
                  navigate("graph", {
                    workspace: message.workspaceId,
                    mode: "execution",
                    run: message.modelRunId,
                  })
                }
              >
                3D 执行轨迹
              </button>
            </>
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
    </article>
  );
});
