import { chat } from "./api/chat";
import { saveReminder, type ReminderInput } from "./api/reminders";
import { webSearch, type WebSearchProvider } from "./api/web-search";
import { parseReminder, reminderOccurrences } from "./reminder-draft";
import { AppDialog, openDialog } from "./components/AppDialog";
import { Fragment, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ArrowDown, ChevronUp, PanelRight, Pencil } from "lucide-react";
import { navigate, useNavigation } from "./navigation";
import { ChatMessage } from "./components/ChatMessage";
import { ChatComposer } from "./components/ChatComposer";
import { MemoryCandidatesBar } from "./components/MemoryCandidatesBar";
import { ChatWebSources, questionWithSources, safeSourceUrl, type SearchState } from "./components/ChatWebSources";
import { ReminderFields } from "./components/ReminderFields";
import { EmptyState } from "./components/EmptyState";
import { CognitiveInspector } from "./components/CognitiveInspector";
import { ChatEvidence } from "./components/ChatEvidence";
import { useChatScroll } from "./components/useChatScroll";
import { PaneDivider, splitStyle } from "./components/PaneDivider";
import { notify } from "./components/Toast";
import { errorText, preference, remember, type Message, type Studio } from "./studio";
import { cloudConnectionLabel, runtimeRoles, type CloudDisclosure, type CloudDraft, type CloudKnowledgeGrant, type Profile,
  type GenerationTarget, type RuntimeConnection, type RuntimeRole } from "./runtime-profiles";

const historyPageSize = 60;

const cloudNames: Record<string, string> = {
  openai: "OpenAI", anthropic: "Anthropic", gemini: "Google Gemini", compatible: "OpenAI 兼容服务",
};

function savedGeneration(workspace: string | null): GenerationTarget | undefined {
  if (!workspace) return;
  try {
    const choice: Partial<GenerationTarget> | null = JSON.parse(preference(`workspace:${workspace}:generation-model`) ?? "null");
    if (!choice || typeof choice.model !== "string" || !choice.model) return;
    if (choice.kind === "local") return { kind: "local", model: choice.model };
    if (choice.kind === "cloud" && typeof choice.connectionId === "string" && choice.connectionId) {
      return { kind: "cloud", connectionId: choice.connectionId, model: choice.model };
    }
  } catch {
    return undefined;
  }
}

export function ChatPage({
  studio,
  onCreateWorkspace,
  onSaveKnowledge,
}: {
  studio: Studio;
  onCreateWorkspace: (next?: "chat" | "cloud") => void;
  onSaveKnowledge: (message: Message, excerpt: string) => void;
}) {
  const route = useNavigation();
  const conversation = studio.conversations.find(
    (item) => item.id === studio.conversationId,
  );
  const preparing = studio.preparingRun?.conversationId === studio.conversationId && !studio.activeRun;
  const currentRunning = preparing || studio.activeRun?.conversationId === studio.conversationId;
  const knowledgeRoute = studio.autoRetrieval || !!studio.knowledgeScope || !!studio.contextSelection.length || studio.useGraph;
  const [modelChoice, setModelChoice] = useState<{ workspace: string; target?: GenerationTarget }>();
  const savedTarget = useMemo(() => savedGeneration(studio.workspaceId), [studio.workspaceId]);
  const generationTarget = modelChoice?.workspace === studio.workspaceId ? modelChoice?.target : savedTarget;
  const chooseGeneration = (target?: GenerationTarget) => {
    const workspace = studio.workspaceId;
    if (!workspace) return;
    setModelChoice({ workspace, target });
    remember(`workspace:${workspace}:generation-model`, target ? JSON.stringify(target) : "");
  };
  const [runtimeAttempt, setRuntimeAttempt] = useState(0);
  const [runtime, setRuntime] = useState<{
    workspace: string; knowledgeRoute: boolean; generationTarget?: GenerationTarget; attempt: number;
    profile?: Profile; localBackup?: string | null; disclosure: CloudDisclosure; syncMemory: boolean; error?: string;
  }>();
  const currentRuntime = runtime?.workspace === studio.workspaceId && runtime.knowledgeRoute === knowledgeRoute
    && runtime.generationTarget === generationTarget && runtime.attempt === runtimeAttempt ? runtime : undefined;
  const [connections, setConnections] = useState<{ workspace: string; rows?: RuntimeConnection[]; error?: string }>();
  const cloudConnections = connections?.workspace === studio.workspaceId ? connections : undefined;
  const target = currentRuntime?.profile?.generation;
  const cloudAnswer = (target ?? generationTarget)?.kind === "cloud";
  const cloudLookup = currentRuntime?.profile?.embedding.kind === "cloud" || currentRuntime?.profile?.reranking.kind === "cloud";
  const cloudTurn = cloudAnswer || cloudLookup;
  const modelName = (target?.kind !== "off" ? target?.model : null) ?? generationTarget?.model ?? studio.settings.model;
  const answerConnection = currentRuntime?.disclosure.generation?.connection;
  const modelReady = currentRuntime && !currentRuntime.error && (cloudAnswer
    ? answerConnection?.credentialAvailable !== false && answerConnection?.catalogState !== "invalid_credential" && !!answerConnection
    : studio.health?.models.some(model => model.name === modelName || model.name === currentRuntime?.localBackup));
  const modelUnavailable = currentRuntime && !modelReady && (cloudAnswer || !!generationTarget)
    ? `所选${cloudAnswer ? "云端" : "本机"}模型当前不可用，请检查连接或重新选择。`
    : undefined;
  const cloudDialog = useRef<HTMLDialogElement>(null);
  const [cloudDraft, setCloudDraft] = useState<CloudDraft>();
  const [shareSelection, setShareSelection] = useState(false);
  const [shareGranted, setShareGranted] = useState(false);
  const [allowRanking, setAllowRanking] = useState(false);
  const cloudSyncDefault = currentRuntime?.syncMemory === true;
  useEffect(() => {
    const workspace = studio.workspaceId;
    if (!studio.desktop || !workspace) return;
    let current = true;
    void chat("list_runtime_cloud_connections", { workspaceId: workspace }).then(
      rows => { if (current) setConnections({ workspace, rows }); },
      error => { if (current) setConnections({ workspace, error: errorText(error) }); },
    );
    return () => { current = false; };
  }, [studio.desktop, studio.workspaceId, runtimeAttempt]);
  useEffect(() => {
    const workspace = studio.workspaceId;
    if (!studio.desktop || !workspace) return;
    let current = true;
    const scope = { workspace, knowledgeRoute, generationTarget, attempt: runtimeAttempt };
    void chat("chat_runtime_route", { workspaceId: workspace, knowledge: knowledgeRoute,
      ...(generationTarget ? { generationTarget } : {}),
    }).then(async selected => {
      if (!current) return;
      const profile = selected.profile ?? undefined;
      const disclosure: CloudDisclosure = {};
      const roles = Object.keys(runtimeRoles) as RuntimeRole[];
      let syncMemory = false;
      if (profile && roles.some(role => profile[role].kind === "cloud")) {
        const { invoke } = await import("@tauri-apps/api/core");
        const [rows, rules] = await Promise.all([
          chat("list_runtime_cloud_connections", { workspaceId: workspace }),
          invoke<{ syncMemoryAndHistory?: boolean }>("model_cloud_privacy_rules", { workspaceId: workspace, input: null }),
        ]);
        syncMemory = rules.syncMemoryAndHistory === true;
        const grants = new Map<string, Promise<CloudKnowledgeGrant[]>>();
        await Promise.all(roles.map(async role => {
          const target = profile[role];
          if (target.kind !== "cloud") return;
          const connection = rows.find(item => item.id === target.connectionId);
          if (!connection) return;
          let allowed: CloudKnowledgeGrant[] = [];
          if (role !== "embedding") {
            if (!grants.has(target.connectionId)) {
              grants.set(target.connectionId, chat(
                "list_model_cloud_knowledge", { workspaceId: workspace, connectionId: target.connectionId },
              ).then(result => result.grants.filter(item => item.available)).catch(() => []));
            }
            allowed = await grants.get(target.connectionId) ?? [];
          }
          disclosure[role] = { connection, grants: allowed };
        }));
      }
      if (current) setRuntime({ ...scope, profile, localBackup: selected.localBackup, disclosure, syncMemory });
    }).catch(error => {
      if (current) setRuntime({ ...scope, disclosure: {}, syncMemory: false, error: errorText(error) });
    });
    return () => { current = false; };
  }, [studio.desktop, studio.workspaceId, knowledgeRoute, studio.settings, generationTarget, runtimeAttempt]);
  useEffect(() => {
    if (cloudDraft) openDialog(cloudDialog.current);
  }, [cloudDraft]);
  const reminderHeading = useRef<HTMLHeadingElement>(null);
  const [reminderDraft, setReminderDraft] = useState<ReminderInput | null>(null);
  const [reminderSaving, setReminderSaving] = useState(false);
  const [reminderError, setReminderError] = useState<string | null>(null);
  const [reminderContext, setReminderContext] = useState<{ workspace: string; conversation: string | null; text: string } | null>(null);
  const latestComposer = useRef({ workspace: studio.workspaceId, conversation: studio.conversationId, text: studio.draft });
  useLayoutEffect(() => {
    latestComposer.current = { workspace: studio.workspaceId, conversation: studio.conversationId, text: studio.draft };
  }, [studio.workspaceId, studio.conversationId, studio.draft]);
  const currentReminder = reminderContext?.workspace === studio.workspaceId && reminderContext?.conversation === studio.conversationId
    ? reminderDraft : null;
  const hasReminderDraft = currentReminder !== null;
  useLayoutEffect(() => {
    if (hasReminderDraft) reminderHeading.current?.focus();
  }, [hasReminderDraft]);
  const taskDraft = useMemo(() => parseReminder(studio.draft), [studio.draft]);
  const isReminder = taskDraft !== null;
  const isBriefing = currentReminder?.newsQuery != null;
  const canSaveReminder = currentReminder !== null && !!currentReminder.title.trim() &&
    (!isBriefing || !!currentReminder.newsQuery?.trim()) && reminderOccurrences(currentReminder).length > 0;
  const draftTooLong = useMemo(() => new TextEncoder().encode(studio.draft.trim()).length > 16_384, [studio.draft]);
  const composerReady =
    studio.desktop &&
    !!studio.workspaceId &&
    !!studio.draft.trim() &&
    !draftTooLong &&
    !studio.sending &&
    !studio.savingRetrieval &&
    !studio.loadingMessages;
  const canAnswer = composerReady && !!modelReady && (cloudAnswer || studio.knowledgeScopeReady) && !studio.activeRun && !currentRunning;
  const canSend = isReminder ? composerReady && !currentRunning && !reminderSaving : canAnswer;
  const { messages } = studio;
  const conversationKey = `${studio.workspaceId}:${studio.conversationId}`;
  const scroll = useChatScroll(conversationKey, messages, !!route.message);
  const [searchConfig, setSearchConfig] = useState<{ workspace: string; configured: boolean | null } | null>(null);
  const [searchStatusAttempt, setSearchStatusAttempt] = useState(0);
  const [webResults, setWebResults] = useState<SearchState | null>(null);
  const searchPanel = useRef<HTMLDivElement>(null);
  const query = studio.draft.trim();
  const visibleResults = webResults?.scope === conversationKey && webResults.query === query ? webResults : null;
  const configured = searchConfig?.workspace === studio.workspaceId ? searchConfig?.configured : undefined;
  const selectedWebSources = visibleResults?.status === "ready"
    ? visibleResults.response.results.filter(source => !visibleResults.excludedUrls.includes(source.url)) : [];
  // The backend's context calculation reserves 256 bytes and adds its system instructions.
  const maxQuestionBytes = Math.min(16_384, studio.settings.numCtx - studio.settings.numPredict - 256 - 768);
  const webInput = visibleResults?.status === "ready" && selectedWebSources.length
    ? questionWithSources(query, { ...visibleResults.response, results: selectedWebSources }, visibleResults.provider, maxQuestionBytes) : null;
  const showWebResults = visibleResults !== null && !currentReminder;
  useEffect(() => {
    const workspace = studio.workspaceId;
    if (!workspace || !studio.desktop) return;
    let current = true;
    void webSearch("read_web_search_status", { workspaceId: workspace }).then(status => {
      if (current) setSearchConfig({ workspace, configured: status.configured });
    }).catch(() => {
      if (current) setSearchConfig({ workspace, configured: null });
    });
    return () => { current = false; };
  }, [studio.workspaceId, studio.desktop, searchStatusAttempt]);
  useEffect(() => {
    if (!showWebResults) return;
    searchPanel.current?.scrollIntoView({ block: "start" });
    searchPanel.current?.querySelector("h2")?.focus({ preventScroll: true });
  }, [showWebResults]);
  const searchWeb = (provider: WebSearchProvider) => {
    const workspaceId = studio.workspaceId;
    if (!workspaceId || (provider === "brave" && !configured) || !query || visibleResults?.status === "loading") return;
    const pending: SearchState = { scope: conversationKey, query, provider, status: "loading" };
    setWebResults(pending);
    void webSearch("search_web", { workspaceId, query, provider }).then(response => {
      const results = response.results.filter(source => safeSourceUrl(source.url));
      setWebResults(current => current === pending
        ? { ...pending, status: "ready", response: { ...response, results }, excludedUrls: [] } : current);
    }).catch(value => {
      setWebResults(current => current === pending ? { ...pending, status: "error", error: errorText(value) } : current);
    });
  };
  const historyKey = `${conversationKey}:${route.message ?? "latest"}`;
  const [historyPage, setHistoryPage] = useState({ historyKey, limit: historyPageSize, newer: 0, initialCount: null as number | null });
  const currentPage = historyPage.historyKey === historyKey
    ? historyPage : { historyKey, limit: historyPageSize, newer: 0, initialCount: null };
  const { limit, newer, initialCount } = currentPage;
  useLayoutEffect(() => {
    if (initialCount === null && messages.length && !studio.loadingMessages) {
      setHistoryPage({ historyKey, limit: historyPageSize, newer: 0, initialCount: messages.length });
    }
  }, [historyKey, initialCount, messages.length, studio.loadingMessages]);
  const targetIndex = useMemo(() => route.message ? messages.findIndex(item => item.id === route.message) : -1, [messages, route.message]);
  const aroundTarget = targetIndex >= 0 && targetIndex < messages.length - historyPageSize;
  const baseEnd = aroundTarget ? Math.max(historyPageSize, targetIndex + historyPageSize / 2) : messages.length;
  const end = Math.min(messages.length, baseEnd + newer);
  // Keep rows already being read in place if a new turn arrives in this visit.
  const appended = aroundTarget || initialCount === null ? 0 : Math.max(0, messages.length - initialCount);
  const start = Math.max(0, baseEnd - limit - appended);
  const focusNewer = useRef<string | null>(null);
  useLayoutEffect(() => {
    if (!focusNewer.current) return;
    const target = document.getElementById(focusNewer.current);
    focusNewer.current = null;
    target?.scrollIntoView({ block: "start" });
    target?.focus({ preventScroll: true });
  }, [end]);
  const restoreAnchor = useRef<{ id: string; top: number; focus: boolean } | null>(null);
  const loadEarlier = () => {
    const id = `message-${messages[start].id}`;
    const anchor = document.getElementById(id);
    if (anchor) restoreAnchor.current = {
      id, top: anchor.getBoundingClientRect().top,
      focus: document.activeElement?.classList.contains("chat-history-older") ?? false,
    };
    scroll.pause();
    setHistoryPage({ ...currentPage, limit: limit + historyPageSize });
  };
  useLayoutEffect(() => {
    const anchor = restoreAnchor.current;
    if (!anchor) return;
    restoreAnchor.current = null;
    const element = document.getElementById(anchor.id);
    if (element && scroll.scroller.current) {
      scroll.scroller.current.scrollTop += element.getBoundingClientRect().top - anchor.top;
      if (anchor.focus) element.focus({ preventScroll: true });
    }
  }, [start, scroll.scroller]);
  useEffect(() => {
    if (!route.message || studio.loadingMessages) return;
    const message = document.getElementById(`message-${route.message}`);
    const element = (route.citation === undefined ? null : message?.querySelector<HTMLElement>(`[data-answer-citation="${route.citation}"]`)) ?? message;
    element?.scrollIntoView({ block: "center" });
    element?.focus({ preventScroll: true });
  }, [route.message, route.citation, studio.messages.length, studio.loadingMessages]);

  const splitRoot = useRef<HTMLDivElement>(null);
  const [splitRatio, setSplitRatio] = useState<number | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const renameDialog = useRef<HTMLDialogElement>(null);
  const [renameTitle, setRenameTitle] = useState("");
  const [renameError, setRenameError] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const evidenceToggle = useRef<HTMLButtonElement>(null);
  const [wide, setWide] = useState(() => window.innerWidth > 1100);
  const [desktopEvidence, setDesktopEvidence] = useState(() => preference("evidence-open") === "true");
  const [overlayEvidence, setOverlayEvidence] = useState(false);
  const evidenceOpen = wide ? desktopEvidence : overlayEvidence;
  const setEvidenceOpen = (open: boolean) => {
    if (wide) { setDesktopEvidence(open); remember("evidence-open", String(open)); }
    else setOverlayEvidence(open);
    requestAnimationFrame(() => {
      if (!open) evidenceToggle.current?.focus({ preventScroll: true });
      else if (wide) document.getElementById("evidence-collapse")?.focus({ preventScroll: true });
    });
  };
  useEffect(() => {
    const query = window.matchMedia("(max-width: 1100px)");
    const update = () => { setWide(!query.matches); setOverlayEvidence(false); };
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  const latestRun = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i--) {
      const message = messages[i];
      if (message.role === "assistant" && message.modelRunId) return message.modelRunId;
    }
    return null;
  }, [messages]);
  const evidenceRun = studio.messages.find(message => message.role === "assistant" && message.id === route.message)?.modelRunId ?? latestRun;

  const hasCitations = !!messages.find(message => message.role === "assistant" && message.modelRunId === evidenceRun)?.citations?.occurrences.length;

  useEffect(() => {
    if (!currentRunning) setCancelling(false);
  }, [currentRunning]);
  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(null), 2500);
    return () => window.clearTimeout(timer);
  }, [copied]);

  const focusComposer = () => requestAnimationFrame(() => document.getElementById("message")?.focus());
  const send = async (content?: string, withWebSources = false) => {
    if (content === undefined && studio.workspaceId && !reminderSaving) {
      const reminder = parseReminder(studio.draft);
      if (reminder) {
        setReminderContext({ workspace: studio.workspaceId, conversation: studio.conversationId, text: studio.draft });
        setReminderError(null);
        setReminderDraft(reminder);
        return;
      }
    }
    if (!modelReady || currentRunning) return;
    if (cloudTurn && currentRuntime?.profile && studio.workspaceId) {
      const question = (content ?? studio.draft).trim();
      if (question) {
        if (cloudSyncDefault) {
          navigate("chat", { workspace: studio.workspaceId, conversation: studio.conversationId });
          return studio.send(content !== undefined && !withWebSources ? question : undefined, {
            question,
            knowledgeRoute,
            selection: studio.contextSelection,
            consent: {
              profileId: currentRuntime.profile.id,
              updatedAt: currentRuntime.profile.updatedAt,
              shareSelection: true,
              grantedVersions: currentRuntime.disclosure.generation?.grants.map(({ nodeId, version }) => ({ nodeId, version })) ?? [],
              rankingVersions: currentRuntime.disclosure.reranking?.grants.map(({ nodeId, version }) => ({ nodeId, version })) ?? [],
            },
          }, withWebSources ? {
            question, workspaceId: studio.workspaceId, conversationId: studio.conversationId,
          } : undefined, generationTarget);
        }
        setShareSelection(false);
        setShareGranted(false);
        setAllowRanking(false);
        setCloudDraft({
          question, retry: content !== undefined && !withWebSources, webSources: withWebSources,
          workspace: studio.workspaceId, conversation: studio.conversationId,
          selection: studio.knowledgeScope ? [] : studio.contextSelection,
          profile: currentRuntime.profile, disclosure: currentRuntime.disclosure,
          knowledgeRoute, generationTarget,
        });
      }
      return;
    }
    navigate("chat", {
      workspace: studio.workspaceId,
      conversation: studio.conversationId,
    });
    if (withWebSources && content && studio.workspaceId) {
      return studio.send(undefined, undefined, { question: content, workspaceId: studio.workspaceId, conversationId: studio.conversationId }, generationTarget);
    }
    return studio.send(content, undefined, undefined, generationTarget);
  };
  const copy = useCallback(async (id: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(id);
      notify("回答已复制");
    } catch {
      studio.reportError("无法自动复制，请选中回答文字后复制。");
    }
  }, [studio.reportError]);
  const retryAction = useRef<(runId: string | null) => void>(() => {});
  useLayoutEffect(() => {
    retryAction.current = (runId) => {
      const question = studio.messages.find((message) => message.role === "user" && message.modelRunId === runId);
      if (question) void send(question.content);
    };
  }, [send, studio.messages]);
  const retry = useCallback((runId: string | null) => retryAction.current(runId), []);
  const canRetry = !!modelReady && !studio.activeRun && !studio.sending;

  return (
    <div
      ref={splitRoot}
      style={wide && evidenceOpen ? splitStyle(splitRatio, 16) : undefined}
      className={`chat-layout live-chat-layout ${studio.messages.length ? "has-messages" : "is-empty"} ${evidenceOpen ? "evidence-open" : "evidence-closed"}`}
    >
      <section className="chat-main" aria-labelledby="page-title">
        <header className="page-heading">
          <div>
            <h1 id="page-title">对话</h1>
            {conversation && (
              <p className="conversation-title">{conversation.title}</p>
            )}
          </div>
          <div className="heading-actions">
            <button ref={evidenceToggle} hidden={evidenceOpen} className="icon-button evidence-toggle" aria-label="打开回答依据" aria-expanded={evidenceOpen} aria-controls="chat-evidence" aria-haspopup={!wide ? "dialog" : undefined} title="打开回答依据" onClick={() => setEvidenceOpen(true)}><PanelRight size={17} />{hasCitations && <span>查看引用</span>}</button>
            {conversation && (
              <button
                className="icon-button"
                aria-label="重命名会话"
                aria-haspopup="dialog"
                aria-controls="rename-conversation"
                onClick={() => {
                  setRenameTitle(conversation.title);
                  setRenameError(null);
                  openDialog(renameDialog.current);
                }}
              >
                <Pencil size={17} aria-hidden="true" />
              </button>
            )}
          </div>
        </header>
        {studio.activeRun && !currentRunning && (
          <div className="active-run-notice" role="status">
            另一个会话正在生成回答。
            <button
              className="button text-button"
              onClick={() => {
                const run = studio.activeRun!;
                void studio
                  .openWorkspace(run.workspaceId)
                  .then(() =>
                    studio.openConversation(
                      run.workspaceId,
                      run.conversationId,
                    ),
                  )
                  .catch(studio.reportError);
              }}
            >
              前往生成中的会话
            </button>
          </div>
        )}
        <div
          className="message-scroll"
          role="region"
          aria-label="对话消息"
          tabIndex={0}
          ref={scroll.scroller}
          onScroll={scroll.onScroll}
          onWheel={scroll.onWheel}
          onPointerDown={scroll.onPointerDown}
          onTouchStart={scroll.onTouchStart}
          onTouchMove={scroll.onTouchMove}
          onKeyDown={scroll.onKeyDown}
        >
          {studio.loadingMessages ? (
            <p className="inline-loading" role="status">
              正在读取会话…
            </p>
          ) : studio.messages.length ? (
            <div className="messages">
              {start > 0 && <button type="button" className="button secondary chat-history-older" onClick={loadEarlier}>
                <ChevronUp size={16} aria-hidden="true" />
                加载更早的消息（还有 {start} 条）
              </button>}
              {messages.slice(start, end).map((message, offset) => {
                const index = start + offset;
                let switchedFrom: string | null = null;
                if (message.role === "user" && message.modelId) {
                  for (let prev = index - 1; prev >= 0; prev--) {
                    const prevModel = messages[prev].modelId;
                    if (prevModel) {
                      if (prevModel !== message.modelId) switchedFrom = prevModel;
                      break;
                    }
                  }
                }
                return (
                  <Fragment key={message.id}>
                    {switchedFrom && (
                      <div
                        className="model-switch-notice"
                        role="status"
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: "10px",
                          margin: "12px 0 4px",
                          color: "var(--text-muted, var(--muted))",
                          fontSize: "0.8rem",
                        }}
                      >
                        <span style={{ flex: 1, height: "1px", background: "var(--border-subtle, var(--border))" }} />
                        <span>模型已从【{switchedFrom}】切换至【{message.modelId}】</span>
                        <span style={{ flex: 1, height: "1px", background: "var(--border-subtle, var(--border))" }} />
                      </div>
                    )}
                    <ChatMessage
                      message={message}
                      stream={studio.chatStream}
                      located={route.message === message.id}
                      copied={copied === message.id}
                      copy={copy}
                      onSaveKnowledge={onSaveKnowledge}
                      setSelectedRun={setSelectedRun}
                      canRetry={canRetry}
                      reportError={studio.reportError}
                      retry={retry}
                    />
                  </Fragment>
                );
              })}

            </div>
          ) : !currentReminder && !showWebResults ? (
            <EmptyState
              studio={studio}
              modelReady={modelReady}
              cloudAnswer={cloudAnswer}
              onCreateWorkspace={onCreateWorkspace}
            />
          ) : null}
          {showWebResults && <div ref={searchPanel}>
            <ChatWebSources state={visibleResults}
              workspaceId={studio.workspaceId}
              canAnswer={canAnswer && !!webInput?.question}
              includedCount={webInput?.count ?? 0}
              onDismiss={() => { setWebResults(null); focusComposer(); }}
              onToggle={url => setWebResults(current => current?.status === "ready" ? {
                ...current,
                excludedUrls: current.excludedUrls.includes(url)
                  ? current.excludedUrls.filter(value => value !== url) : [...current.excludedUrls, url],
              } : current)}
              onAnswer={() => {
                if (canAnswer && webInput?.question) void send(webInput.question, true);
              }} />
          </div>}
          {currentReminder && <form className="chat-reminder" aria-labelledby="chat-reminder-title" onSubmit={event => {
            event.preventDefault();
            const context = reminderContext;
            if (reminderSaving || !context) return;
            if (context.workspace !== studio.workspaceId || context.conversation !== studio.conversationId) {
              setReminderError("当前会话已变化，请返回会话重新确认。");
              return;
            }
            if (!canSaveReminder || !reminderOccurrences(currentReminder).length) {
              setReminderError(isBriefing ? "请填写简报名称和查询词，并选择可触发的未来时间。" : "请填写提醒内容并选择可触发的未来时间。");
              return;
            }
            setReminderSaving(true);
            setReminderError(null);
            void saveReminder(context.workspace, null, currentReminder).then(() => {
              const composer = latestComposer.current;
              if (context.workspace === composer.workspace && context.conversation === composer.conversation &&
                  context.text === composer.text) studio.setDraft("");
              setReminderDraft(null);
              if (context.workspace === composer.workspace && context.conversation === composer.conversation) focusComposer();
              notify(isBriefing ? "新闻简报已启用，可在任务中心查看" : "提醒已保存，可在任务中心查看");
            }).catch(value => setReminderError(errorText(value))).finally(() => setReminderSaving(false));
          }}>
            <h2 id="chat-reminder-title" ref={reminderHeading} tabIndex={-1}>{isBriefing ? "确认新闻简报" : "确认提醒"}</h2>
            {!isBriefing && <p>提醒在应用运行时送达。恢复运行时会补发最近一次迟到不超过 5 分钟的提醒，较早的提醒记为错过。</p>}
            <div inert={reminderSaving}>
              <ReminderFields value={currentReminder} onChange={value => {
                setReminderDraft(value);
                setReminderError(null);
              }} />
            </div>
            {reminderError && <p className="form-error" role="alert">{reminderError}</p>}
            <div className="dialog-actions">
              <button className="button secondary" type="button" disabled={reminderSaving}
                onClick={() => { setReminderDraft(null); focusComposer(); }}>取消</button>
              <button className="button primary" disabled={reminderSaving || !canSaveReminder}>
                {reminderSaving ? "正在保存…" : isBriefing ? "确认启用" : "确认保存"}
              </button>
            </div>
          </form>}
        </div>
        {!currentReminder && messages.length > 0 && (scroll.showJump || end < messages.length) && (
          <div className="jump-latest" role="group" aria-label="历史消息导航">
            {end < messages.length && <button type="button" className="button secondary" onClick={() => {
              focusNewer.current = `message-${messages[end].id}`;
              setHistoryPage({ ...currentPage, newer: newer + historyPageSize });
            }}>
              加载更新的消息（还有 {messages.length - end} 条）
            </button>}
            <button type="button" className="button secondary" onClick={() => {
              if (route.message) navigate("chat", { workspace: studio.workspaceId, conversation: studio.conversationId, back: route.back }, { replace: true });
              scroll.jump();
            }}>
              <ArrowDown size={15} aria-hidden="true" />
              回到最新内容
            </button>
          </div>
        )}
        {!currentReminder && studio.desktop && (
          <MemoryCandidatesBar
            workspaceId={studio.workspaceId}
            conversationId={studio.conversationId}
            refreshKey={messages.filter(m => m.role === "assistant" && m.status === "completed").length}
          />
        )}
        {!currentReminder && <ChatComposer
          studio={studio}
          modelName={cloudAnswer && answerConnection ? modelName + " · " + cloudConnectionLabel(answerConnection) : modelName}
          generationTarget={generationTarget}
          chooseGeneration={chooseGeneration}
          connections={cloudConnections?.rows ?? []}
          modelError={currentRuntime?.error ?? modelUnavailable ?? cloudConnections?.error}
          retryModels={() => {
            setRuntimeAttempt(attempt => attempt + 1);
            void studio.checkConnection();
          }}
          cloudAnswer={cloudAnswer}
          cloudLookup={cloudLookup}
          cloudSyncDefault={cloudSyncDefault}
          currentRunning={currentRunning}
          preparing={preparing}
          cancelling={cancelling}
          setCancelling={setCancelling}
          canSend={canSend}
          taskDraft={taskDraft ? (taskDraft.newsQuery != null ? "briefing" : "reminder") : null}
          draftTooLong={draftTooLong}
          searchConfigured={configured}
          retrySearchStatus={() => {
            setSearchConfig(null);
            setSearchStatusAttempt(attempt => attempt + 1);
          }}
          searching={visibleResults?.status === "loading"}
          searchWeb={searchWeb}
          send={send}
        />}
        <p className="sr-only" role="status">
          {preparing ? "正在准备回答" : currentRunning ? "正在生成回答" : "当前没有生成任务"}
          {copied ? "，回答已复制" : ""}
        </p>
      </section>
      {evidenceOpen && <ChatEvidence studio={studio} overlay={!wide} runId={evidenceRun} onInspect={() => setSelectedRun(evidenceRun)} onClose={() => setEvidenceOpen(false)} />}
      {wide && evidenceOpen && <PaneDivider root={splitRoot} ratio={splitRatio} onChange={setSplitRatio} onCollapse={() => setEvidenceOpen(false)} label="调整回答依据宽度" />}
      <AppDialog className="form-dialog" ref={cloudDialog} aria-labelledby="cloud-send-title" onClose={() => setCloudDraft(undefined)}>
        {cloudDraft && <form onSubmit={event => {
          event.preventDefault();
          if (cloudDraft.workspace !== studio.workspaceId || cloudDraft.conversation !== studio.conversationId) {
            cloudDialog.current?.close();
            studio.reportError("当前会话已变化，请重新确认要发送的内容。");
            return;
          }
          cloudDialog.current?.close();
          navigate("chat", { workspace: studio.workspaceId, conversation: studio.conversationId });
          void studio.send(cloudDraft.retry ? cloudDraft.question : undefined, {
            question: cloudDraft.question,
            knowledgeRoute: cloudDraft.knowledgeRoute,
            selection: cloudDraft.profile.generation.kind === "cloud"
              ? (shareSelection ? cloudDraft.selection : []) : cloudDraft.selection,
            consent: {
              profileId: cloudDraft.profile.id, updatedAt: cloudDraft.profile.updatedAt, shareSelection,
              grantedVersions: shareGranted
                ? cloudDraft.disclosure.generation?.grants.map(({ nodeId, version }) => ({ nodeId, version })) ?? [] : [],
              rankingVersions: allowRanking
                ? cloudDraft.disclosure.reranking?.grants.map(({ nodeId, version }) => ({ nodeId, version })) ?? [] : [],
            },
          }, cloudDraft.webSources ? {
            question: cloudDraft.question, workspaceId: cloudDraft.workspace, conversationId: cloudDraft.conversation,
          } : undefined, cloudDraft.generationTarget);
        }}>
          <h2 id="cloud-send-title">确认发送到云端</h2>
          <ul className="cloud-runtime-capabilities">{(Object.keys(runtimeRoles) as RuntimeRole[]).map(role => {
            const target = cloudDraft.profile[role];
            if (target.kind !== "cloud") return null;
            const connection = cloudDraft.disclosure[role]?.connection;
            return <li key={role}><span><strong>{runtimeRoles[role]}</strong>　{connection && (connection.provider === "compatible" ? cloudConnectionLabel(connection) : cloudNames[connection.provider] ?? connection.label)} / {target.model}
              {connection?.address && <><br />{connection.address}</>}</span></li>;
          })}</ul>
          <p>云端回答发送本次问题。向量查找启用且索引可用时发送问题。历史对话留在本机，云端服务可能计费。</p>
          {cloudDraft.profile.generation.kind === "cloud" && <p>云端回答使用内置默认助手。个人助手指令留在本机。</p>}
          {cloudDraft.webSources && <p>本次问题包含你选用的网页标题、链接和摘要，适用的云端服务会收到这些内容。</p>}
          {cloudDraft.profile.generation.kind === "cloud" && cloudDraft.selection.length > 0 && <label className="retrieval-switch">
            <input type="checkbox" checked={shareSelection} onChange={event => setShareSelection(event.target.checked)} />
            同时发送手动选中的 {cloudDraft.selection.length} 条知识
          </label>}
          {!!cloudDraft.disclosure.generation?.grants.length && <>
            <label className="retrieval-switch">
              <input type="checkbox" checked={shareGranted} onChange={event => setShareGranted(event.target.checked)} />
              查找并发送此连接已获授权的相关知识
            </label>
            <details className="field-help">
              <summary>可参与查找的 {cloudDraft.disclosure.generation.grants.length} 个知识版本</summary>
              <ul>{cloudDraft.disclosure.generation.grants.map(grant => <li key={grant.nodeId + "/" + grant.version}>{grant.title}（版本 {grant.version}）</li>)}</ul>
            </details>
          </>}
          {!!cloudDraft.disclosure.reranking?.grants.length && <>
            <label className="retrieval-switch">
              <input type="checkbox" checked={allowRanking} onChange={event => setAllowRanking(event.target.checked)} />
              将问题和授权候选送到重排模型
            </label>
            <details className="field-help">
              <summary>可参与重排的 {cloudDraft.disclosure.reranking.grants.length} 个知识版本</summary>
              <ul>{cloudDraft.disclosure.reranking.grants.map(grant => <li key={grant.nodeId + "/" + grant.version}>{grant.title}（版本 {grant.version}）</li>)}</ul>
            </details>
          </>}
          <div className="dialog-actions">
            <button type="button" className="button secondary" onClick={() => cloudDialog.current?.close()}>取消</button>
            <button className="button primary">确认发送</button>
          </div>
        </form>}
      </AppDialog>
      <AppDialog
        className="form-dialog"
        ref={renameDialog}
        id="rename-conversation"
        aria-labelledby="rename-title"
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            setRenaming(true);
            setRenameError(null);
            void studio
              .renameConversation(renameTitle)
              .then(() => renameDialog.current?.close())
              .catch((value) => setRenameError(errorText(value)))
              .finally(() => setRenaming(false));
          }}
        >
          <h2 id="rename-title">重命名会话</h2>
          <div className="field">
            <label htmlFor="conversation-title">会话名称</label>
            <input
              id="conversation-title"
              value={renameTitle}
              onChange={(event) => setRenameTitle(event.target.value)}
              required
              maxLength={80}
            />
          </div>
          {renameError && (
            <p className="form-error" role="alert">
              {renameError}
            </p>
          )}
          <div className="dialog-actions">
            <button
              className="button secondary"
              type="button"
              onClick={() => renameDialog.current?.close()}
            >
              取消
            </button>
            <button className="button primary" disabled={renaming}>
              {renaming ? "正在保存…" : "保存名称"}
            </button>
          </div>
        </form>
      </AppDialog>
      <CognitiveInspector
        runId={selectedRun}
        studio={studio}
        onClose={() => setSelectedRun(null)}
      />
    </div>
  );
}
