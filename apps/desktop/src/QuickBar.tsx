import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowUpRight,
  BookmarkPlus,
  Check,
  GripHorizontal,
  Send,
  Sparkles,
  Square,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { chat } from "./api/chat";
import { quickbarApi } from "./api/quickbar";
import type { StreamUpdate } from "./chat-stream";
import { BrandMark } from "./components/BrandMark";
import { MarkdownContent } from "./components/MarkdownContent";
import { handleWindowDrag } from "./components/WindowControls";
import { errorText } from "./frontend-error";
import { preference, remember } from "./preferences";
import type { CloudConsent, GenerationTarget } from "./runtime-profiles";
import type { RunSummary, Workspace } from "./studio";
import "./styles/tokens.css";
import "./styles/layout.css";
import "./styles/controls.css";
import "./styles/features.css";
import "./styles/motion.css";
import "./markdown.css";

const COMPACT_HEIGHT = 136;
const EXPANDED_HEIGHT = 416;

function readSavedGeneration(workspace: string | null): GenerationTarget | undefined {
  if (!workspace) return undefined;
  try {
    const raw = JSON.parse(preference(`workspace:${workspace}:generation-model`) ?? "null");
    if (!raw || typeof raw.model !== "string" || !raw.model) return undefined;
    if (raw.kind === "local") return { kind: "local", model: raw.model };
    if (raw.kind === "cloud" && typeof raw.connectionId === "string" && raw.connectionId) {
      return { kind: "cloud", connectionId: raw.connectionId, model: raw.model };
    }
  } catch {
    return undefined;
  }
  return undefined;
}

export function QuickBar() {
  const desktop = isTauri();
  const [mode, setMode] = useState<"ask" | "note">("ask");
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [workspaceId, setWorkspaceId] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [savingNote, setSavingNote] = useState(false);
  const [savedFeedback, setSavedFeedback] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const [answer, setAnswer] = useState("");
  const [activeRun, setActiveRun] = useState<Pick<RunSummary, "id" | "workspaceId" | "conversationId"> | null>(null);
  const [lastConversationId, setLastConversationId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const composing = useRef(false);
  const busyRef = useRef(false);
  busyRef.current = asking || savingNote;

  const syncWorkspaces = useCallback(async () => {
    if (!desktop) return;
    try {
      const boot = await chat("bootstrap");
      setWorkspaces(boot.workspaces);
      const preferred = preference("workspace");
      const chosen =
        boot.workspaces.find((w) => w.id === preferred) ?? boot.workspaces[0] ?? null;
      setWorkspaceId(chosen?.id ?? null);
    } catch (err) {
      setError(errorText(err));
    }
  }, [desktop]);

  const setHeight = useCallback(
    (height: number) => {
      if (!desktop) return;
      void quickbarApi("resize_quickbar", { height }).catch(() => {});
    },
    [desktop],
  );

  const hideWindow = useCallback(() => {
    if (!desktop) return;
    void quickbarApi("hide_quickbar").catch(() => {});
  }, [desktop]);

  useEffect(() => {
    document.title = "CortexOS 随手记与秒问";
    void syncWorkspaces();
    requestAnimationFrame(() => inputRef.current?.focus());

    if (!desktop) return;
    const shownPromise = listen("quickbar-shown", () => {
      void syncWorkspaces();
      setSavedFeedback(null);
      setError(null);
      requestAnimationFrame(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      });
    }).catch(() => undefined);

    const onBlur = () => {
      // Auto-hide when losing focus while idle (Spotlight behavior).
      window.setTimeout(() => {
        if (!document.hasFocus() && !busyRef.current && !answer && !error) {
          hideWindow();
        }
      }, 140);
    };
    window.addEventListener("blur", onBlur);

    return () => {
      window.removeEventListener("blur", onBlur);
      void shownPromise.then((unlisten) => unlisten?.()).catch(() => {});
    };
  }, [desktop, syncWorkspaces, hideWindow, answer, error]);

  useEffect(() => {
    const wantExpanded = Boolean(asking || answer || error);
    setHeight(wantExpanded ? EXPANDED_HEIGHT : COMPACT_HEIGHT);
  }, [asking, answer, error, setHeight]);

  const resetConversationState = () => {
    setAnswer("");
    setError(null);
    setLastConversationId(null);
    setActiveRun(null);
  };

  const submitNote = async () => {
    if (!desktop || !workspaceId || savingNote) return;
    const trimmed = input.trim().replace(/^\/+\s*/, "").trim();
    if (!trimmed) return;
    setSavingNote(true);
    setError(null);
    try {
      await quickbarApi("quickbar_save_note", {
        workspaceId,
        content: trimmed,
      });
      setInput("");
      setSavedFeedback("已存入当前工作区知识库");
      window.setTimeout(() => {
        setSavedFeedback(null);
        hideWindow();
      }, 420);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setSavingNote(false);
    }
  };

  const submitAsk = async () => {
    if (!desktop || !workspaceId || asking) return;
    const question = input.trim();
    if (!question) return;
    setAsking(true);
    setError(null);
    setAnswer("");
    setSavedFeedback(null);
    try {
      const conv = await chat("create_conversation", { workspaceId });
      setLastConversationId(conv.id);
      const generationTarget = readSavedGeneration(workspaceId);
      const route = await chat("chat_runtime_route", {
        workspaceId,
        knowledge: true,
        ...(generationTarget ? { generationTarget } : {}),
      });

      let cloudConsent: CloudConsent | undefined;
      if (
        route.profile &&
        (route.profile.generation.kind === "cloud" ||
          route.profile.embedding.kind === "cloud" ||
          route.profile.reranking.kind === "cloud")
      ) {
        const rules = await invoke<{ syncMemoryAndHistory?: boolean }>(
          "model_cloud_privacy_rules",
          { workspaceId, input: null },
        );
        if (!rules.syncMemoryAndHistory) {
          // If the workspace requires manual per-turn cloud consent, hand off cleanly to main window.
          setAsking(false);
          await quickbarApi("open_in_main_window", {
            workspaceId,
            conversationId: conv.id,
          });
          return;
        }
        cloudConsent = {
          profileId: route.profile.id,
          updatedAt: route.profile.updatedAt,
          shareSelection: false,
          grantedVersions: [],
          rankingVersions: [],
        };
      }

      const prepChannel = new Channel<Pick<RunSummary, "id" | "workspaceId" | "conversationId">>();
      prepChannel.onmessage = (prep) => setActiveRun(prep);

      const updateChannel = new Channel<StreamUpdate>();
      updateChannel.onmessage = (update) => {
        if (update.content) setAnswer(update.content);
        if (update.status !== "running") {
          setAsking(false);
          setActiveRun(null);
          if (update.errorMessage) setError(update.errorMessage);
        }
      };

      const started = await chat("start_chat", {
        workspaceId,
        conversationId: conv.id,
        content: question,
        contextSelection: [],
        knowledgeScope: null,
        useGraph: false,
        ...(cloudConsent ? { cloudConsent, useKnowledgeRoute: true } : {}),
        ...(generationTarget ? { generationTarget } : {}),
        onPreparing: prepChannel,
        onUpdate: updateChannel,
      });
      setActiveRun(started);
      setInput("");
    } catch (err) {
      setAsking(false);
      setActiveRun(null);
      setError(errorText(err));
    }
  };

  const effectiveMode = input.startsWith("/") ? "note" : mode;
  const currentWorkspace = workspaces.find((w) => w.id === workspaceId);

  return (
    <div
      className="quickbar-shell"
      style={{
        height: "100vh",
        width: "100vw",
        display: "flex",
        flexDirection: "column",
        background: "var(--surface-elevated, var(--surface))",
        color: "var(--ink, var(--text))",
        border: "1px solid var(--line-strong, var(--line))",
        borderRadius: "12px",
        overflow: "hidden",
        boxSizing: "border-box",
      }}
    >
      <header
        data-tauri-drag-region
        onMouseDown={(event) => {
          if (desktop && !(event.target instanceof Element && event.target.closest("button, select"))) {
            handleWindowDrag(event, () => {});
          }
        }}
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "8px",
          padding: "7px 12px 5px",
          borderBottom: "1px solid var(--line)",
          fontSize: "12px",
          userSelect: "none",
          cursor: "grab",
        }}
      >
        <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", minWidth: 0 }}>
          <BrandMark size={18} />
          <div
            role="tablist"
            aria-label="浮窗模式"
            style={{
              display: "inline-flex",
              gap: "3px",
              padding: "2px",
              borderRadius: "999px",
              background: "var(--surface-inset, rgba(127,127,127,0.1))",
            }}
          >
            <button
              type="button"
              role="tab"
              aria-selected={effectiveMode === "ask"}
              className="button text-button"
              style={{
                padding: "2px 9px",
                fontSize: "11.5px",
                borderRadius: "999px",
                background: effectiveMode === "ask" ? "var(--surface)" : "transparent",
                fontWeight: effectiveMode === "ask" ? 600 : 400,
              }}
              onClick={() => {
                if (input.startsWith("/")) setInput(input.replace(/^\/+\s*/, ""));
                setMode("ask");
                inputRef.current?.focus();
              }}
            >
              <Sparkles size={12} aria-hidden="true" />
              秒问 AI
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={effectiveMode === "note"}
              className="button text-button"
              style={{
                padding: "2px 9px",
                fontSize: "11.5px",
                borderRadius: "999px",
                background: effectiveMode === "note" ? "var(--surface)" : "transparent",
                fontWeight: effectiveMode === "note" ? 600 : 400,
              }}
              onClick={() => {
                setMode("note");
                resetConversationState();
                inputRef.current?.focus();
              }}
            >
              <BookmarkPlus size={12} aria-hidden="true" />
              随手记
            </button>
          </div>
          {workspaces.length > 0 && (
            <select
              aria-label="目标工作区"
              value={workspaceId ?? ""}
              onChange={(event) => {
                const next = event.target.value;
                setWorkspaceId(next);
                remember("workspace", next);
              }}
              style={{
                fontSize: "11.5px",
                padding: "2px 6px",
                borderRadius: "6px",
                border: "1px solid var(--line)",
                background: "var(--surface)",
                color: "inherit",
                maxWidth: "150px",
              }}
            >
              {workspaces.map((ws) => (
                <option key={ws.id} value={ws.id}>
                  {ws.name}
                </option>
              ))}
            </select>
          )}
        </div>
        <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", color: "var(--muted)" }}>
          <span style={{ fontSize: "11px" }}>Tab 切换 · Esc 收起</span>
          <GripHorizontal size={13} aria-hidden="true" />
          <button
            type="button"
            className="icon-button"
            aria-label="收起悬浮窗"
            title="收起（Esc）"
            style={{ width: "22px", height: "22px" }}
            onClick={hideWindow}
          >
            <X size={14} aria-hidden="true" />
          </button>
        </div>
      </header>

      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (effectiveMode === "note") void submitNote();
          else void submitAsk();
        }}
        style={{
          display: "flex",
          alignItems: "center",
          gap: "8px",
          padding: "10px 14px",
        }}
      >
        <input
          ref={inputRef}
          value={input}
          onChange={(event) => {
            setInput(event.target.value);
            if (savedFeedback) setSavedFeedback(null);
          }}
          onCompositionStart={() => {
            composing.current = true;
          }}
          onCompositionEnd={() => {
            composing.current = false;
          }}
          onKeyDown={(event) => {
            if (composing.current || event.nativeEvent.isComposing || event.keyCode === 229) return;
            if (event.key === "Escape") {
              event.preventDefault();
              if (asking && activeRun) {
                void chat("cancel_run", {
                  workspaceId: activeRun.workspaceId,
                  runId: activeRun.id,
                }).catch(() => {});
              } else if (answer || error) {
                resetConversationState();
              } else {
                hideWindow();
              }
              return;
            }
            if (event.key === "Tab") {
              event.preventDefault();
              if (effectiveMode === "ask") {
                setMode("note");
                resetConversationState();
              } else {
                if (input.startsWith("/")) setInput(input.replace(/^\/+\s*/, ""));
                setMode("ask");
              }
            }
          }}
          disabled={!desktop || !workspaceId || savingNote}
          placeholder={
            !workspaceId
              ? "请先在主窗口创建一个工作区…"
              : effectiveMode === "note"
                ? `记录灵感、结论或待办，按 Enter 直接存入「${currentWorkspace?.name ?? "知识库"}」…`
                : `向「${currentWorkspace?.name ?? "专属助手"}」提问（输入 / 快速切换随手记）…`
          }
          style={{
            flex: 1,
            minWidth: 0,
            fontSize: "14px",
            padding: "8px 10px",
            borderRadius: "8px",
            border: "1px solid var(--line)",
            background: "var(--surface)",
          }}
        />
        {asking ? (
          <button
            type="button"
            className="button secondary"
            onClick={() => {
              if (activeRun) {
                void chat("cancel_run", {
                  workspaceId: activeRun.workspaceId,
                  runId: activeRun.id,
                }).catch(() => {});
              }
            }}
          >
            <Square size={13} aria-hidden="true" />
            停止
          </button>
        ) : (
          <button
            type="submit"
            className="button primary"
            disabled={!desktop || !workspaceId || !input.trim().replace(/^\/+\s*/, "") || savingNote}
          >
            {effectiveMode === "note" ? (
              <>
                <BookmarkPlus size={14} aria-hidden="true" />
                {savingNote ? "保存中…" : "存知识"}
              </>
            ) : (
              <>
                <Send size={14} aria-hidden="true" />
                提问
              </>
            )}
          </button>
        )}
      </form>

      {savedFeedback && (
        <div
          role="status"
          style={{
            padding: "0 14px 8px",
            fontSize: "12.5px",
            color: "var(--success, #16a34a)",
            display: "inline-flex",
            alignItems: "center",
            gap: "5px",
          }}
        >
          <Check size={14} aria-hidden="true" />
          {savedFeedback}
        </div>
      )}

      {(asking || answer || error) && (
        <div
          style={{
            flex: 1,
            minHeight: 0,
            display: "flex",
            flexDirection: "column",
            borderTop: "1px solid var(--line)",
            background: "var(--surface)",
          }}
        >
          <div
            className="markdown-content"
            style={{
              flex: 1,
              overflowY: "auto",
              padding: "10px 14px",
              fontSize: "13.5px",
              lineHeight: 1.55,
            }}
          >
            {error && (
              <p className="form-error" role="alert" style={{ margin: "0 0 8px" }}>
                {error}
              </p>
            )}
            {answer ? (
              <MarkdownContent content={answer} />
            ) : asking ? (
              <p style={{ color: "var(--muted)", margin: 0 }}>正在结合当前工作区记忆思考…</p>
            ) : null}
          </div>
          <footer
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "6px 14px",
              borderTop: "1px solid var(--line)",
              fontSize: "12px",
            }}
          >
            <button
              type="button"
              className="button text-button"
              onClick={() => {
                resetConversationState();
                inputRef.current?.focus();
              }}
            >
              清空结果
            </button>
            {workspaceId && lastConversationId && (
              <button
                type="button"
                className="button secondary"
                style={{ padding: "4px 10px", fontSize: "12px" }}
                onClick={() => {
                  void quickbarApi("open_in_main_window", {
                    workspaceId,
                    conversationId: lastConversationId,
                  });
                }}
              >
                在主窗口继续深聊
                <ArrowUpRight size={13} aria-hidden="true" />
              </button>
            )}
          </footer>
        </div>
      )}
    </div>
  );
}
