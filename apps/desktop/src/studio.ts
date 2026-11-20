import { useStudioSettings } from "./studio-settings";
import { preference, remember } from "./preferences";
import { useChatWorkspace } from "./chat-workspace";
import { createChatStream, type StreamUpdate } from "./chat-stream";
import { errorText } from "./frontend-error";
import { draftStore } from "./draft-store";
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CloudConsent, ModelRoute } from "./runtime-profiles";

export interface ModelSettings {
  model: string;
  numCtx: number;
  temperature: number;
  numPredict: number;
  timeoutSeconds: number;
}
export interface Workspace {
  id: string;
  name: string;
  createdAt: string;
}
export interface Conversation {
  id: string;
  workspaceId: string;
  title: string;
  updatedAt: string;
  autoRetrieval: boolean;
}
export interface Message {
  id: string;
  workspaceId: string;
  conversationId: string;
  role: "user" | "assistant";
  content: string;
  status: string;
  modelRunId: string | null;
  finishReason: string | null;
  citations?: CitationReport | null;
  createdAt: string;
}
export interface RunSummary {
  id: string;
  workspaceId: string;
  conversationId: string;
  assistantMessageId: string;
  modelId: string;
  status: string;
  startedAt: string;
  finishedAt: string | null;
  errorMessage: string | null;
}
export interface ContextSelection {
  nodeId: string;
  version: number;
}
export type KnowledgeScope =
  | { kind: "snapshot"; snapshotId: string }
  | { kind: "branch"; snapshotId: string }
  | { kind: "factDate"; date: string };

export function localDay() {
  const now = new Date();
  return [now.getFullYear(), now.getMonth() + 1, now.getDate()]
    .map((part, index) => String(part).padStart(index === 0 ? 4 : 2, "0"))
    .join("-");
}
export interface KnowledgeReference extends ContextSelection {
  citationId?: string;
  title: string;
  kind: string;
  content: string;
  contentHash: string;
  reason: string;
  sourceChunk?: { expanded: boolean; matchStartLine: number; matchEndLine: number; sourcePath: string };
  versionLineage?: VersionLineage;
  sourceEvidence?: Omit<
    import("./SourceBrowser").SourceEvidence,
    "knowledgeVersion"
  >[];
}
export interface VersionLineage {
  replacesVersion: number | null;
  replacedByVersion: number | null;
}

interface ConflictSide extends ContextSelection {
  title: string;
  citationId: string | null;
}
export interface RetrievalGraphNode extends ContextSelection {
  title: string;
  source?: { sourceId: string; version: number; name: string; startLine: number; endLine: number };
}
export interface KnowledgeClusters {
  groups: {
    id: string;
    name: string;
    nodeCount: number;
    edgeCount: number;
    fileCount: number;
    relationCounts: Record<string, number>;
    examples: { node: RetrievalGraphNode; excerpt: string; excerptTruncated: boolean }[];
  }[];
  truncated: boolean;
}
export interface CitationOccurrence {
  start: number;
  end: number;
  label: string;
  referenceId: string | null;
  startLine: number | null;
  endLine: number | null;
  error: string | null;
}
export interface CitationReport {
  version: number;
  scope: "reference_locations_only";
  references: KnowledgeReference[];
  occurrences: CitationOccurrence[];
  availableCount: number;
}
type RetrievalMethod = "none" | "fts5" | "fts5_rerank" | "hybrid" | "hybrid_rerank" | "version_aware";
export interface RetrievalReport {
  enabled: boolean;
  status: "disabled" | "no_search_terms" | "no_match" | "matched";
  algorithm?: string;
  includedCount: number;
  candidateLimit?: number;
  candidatesTruncated?: boolean;
  queryTruncated?: boolean;
  semanticStatus?: "completed" | "unavailable" | "index_changed";
  semanticNote?: string;
  embedding?: { model: string; provider?: string; elapsedMicros: number; cloudTransfer?: {
    status: string; attempted: boolean; inputTokens?: number | null;
  } };
  reranker?: string;
  rerankerTarget?: { provider: string; model: string };
  cloudRerank?: { status: string; reason?: string; attempted: boolean; appliedCount: number };
  elapsedMicros?: number;
  stages?: { kind: string; elapsedMicros: number; available?: boolean }[];
  sourceChunksUsed?: number;
  route?: {
    policyVersion: number;
    plannedMethod: RetrievalMethod;
    usedMethod: RetrievalMethod;
    reason: "conversation_disabled" | "no_searchable_text" | "standalone_greeting" | "version_scope" | "identifier" | "embedding_index" | "no_embedding_index" | "runtime_no_matching_index";
    graphReason: "user_enabled" | "relationship_question" | null;
    fallbackReason: "preview" | "unavailable" | "index_changed" | "no_query_vector" | null;
  };
  conflicts?: {
    edgeId: string;
    edgeVersion: number;
    recordedIn: "current_graph" | "saved_snapshot";
    from: ConflictSide;
    to: ConflictSide;
  }[];
  conflictsTruncated?: boolean;
  graph?: {
    status: "no_seeds" | "no_paths" | "matched";
    maxHops: number;
    truncated: boolean;
    paths: {
      nodes: RetrievalGraphNode[];
      steps: {
        edgeId: string;
        edgeVersion: number;
        recordedIn: "current_graph" | "saved_snapshot";
        relation: string;
        from: RetrievalGraphNode;
        to: RetrievalGraphNode;
        reversed: boolean;
      }[];
      citationId: string | null;
      crossDocument: boolean;
    }[];
  };
  candidates: {
    nodeId: string;
    version: number;
    title: string;
    content: string;
    score: number;
    included: boolean;
    reason: string;
    costUnits: number;
    lexicalRank?: number;
    semanticRank?: number;
    fusionRank?: number;
    rerankScore?: number;
    sourceLocation?: import("./SourceBrowser").SourceRef & {
      name: string;
      match: { startLine: number; endLine: number } | null;
    };
    recallSource?: "explicit_conflict" | "graph_relation";
  }[];
}
export interface ContextManifest {
  purpose?: "chat" | "benchmark";
  batchId?: string;
  benchmarkScope?: "standard" | "personalized";
  strategy: string;
  estimatedUnits: number;
  budgetUnits: number;
  estimateNote: string;
  messageRefs: { messageId: string; contentHash: string }[];
  retrievalUsed: boolean;
  retrieval?: RetrievalReport;
  knowledgeRefs?: KnowledgeReference[];
  modelRoute?: ModelRoute;
  modelFallback?: { primaryModel: string; backupModel: string; reason: string };
  knowledgeScope?:
    | { kind: "snapshot" | "branch"; name: string; factDate: string; eligibleCount: number; unavailableCount: number }
    | { kind: "factDate"; factDate: string; eligibleCount: number; unknownDateCount: number };
}
export interface ContextSnapshot {
  request: {
    model: string;
    messages: { role: string; content: string }[];
    options: Record<string, number>;
    think: boolean;
    keep_alive: string;
  };
  requestHash: string;
  manifest: ContextManifest;
}
export interface RunDetail {
  run: RunSummary;
  settings: ModelSettings;
  contextManifest: ContextManifest;
  snapshot: ContextSnapshot | null;
  primarySnapshot?: ContextSnapshot;
  events: { seq: number; kind: string; occurredAt: string; payload: unknown }[];
}
export interface Bootstrap {
  workspaces: Workspace[];
  settings: ModelSettings;
  activeRun: RunSummary | null;
  activeTask?: {
    id: string;
    workspaceId: string;
    kind: "chat" | "benchmark" | "harness_validation" | "harness_workspace" | "harness_session" | "harness_experiment" | "rag_index" | "local_maintenance";
    executionRunId: string | null;
  } | null;
  dataPath: string;
}
export interface ModelHealth {
  version: string;
  models: {
    name: string;
    size: number;
    parameterSize: string | null;
    quantization: string | null;
  }[];
}
export const desktop = isTauri();
export { errorText };
export { preference, remember };
export { defaultSettings } from "./studio-settings";

const chatDrafts = draftStore<string>("chat", value => typeof value === "string");
const emptyContext: ContextSelection[] = [];
function useStudioError() {
  const [error, setError] = useState<string | null>(null);
  const reportError = useCallback((value: unknown) => setError(errorText(value)), []);
  const clearError = useCallback(() => setError(null), []);
  return { error, reportError, clearError };
}

export function useStudio() {
  const [data, setData] = useState<Bootstrap | null>(null);
  const { settings, settingsDraft, setSettingsDraft, applySettings, saveSettings } = useStudioSettings();
  const {
    workspaceId, conversationId, conversations, messages: storedMessages, loadingMessages, creatingConversation,
    view, refreshMessages, refreshConversations, openConversation, openWorkspace, newConversation, renameConversation,
  } = useChatWorkspace();
  const [chatStream] = useState(createChatStream);
  const [activeRun, setActiveRun] = useState<RunSummary | null>(null);
  const [preparingRun, setPreparingRun] = useState<Pick<RunSummary, "id" | "workspaceId" | "conversationId"> | null>(null);
  const [contexts, setContexts] = useState<Record<string, ContextSelection[]>>(
    {},
  );
  const [knowledgeScopes, setKnowledgeScopes] = useState<Record<string, KnowledgeScope | null>>({});
  const [graphLookups, setGraphLookups] = useState<Record<string, boolean>>({});
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [health, setHealth] = useState<ModelHealth | null>(null);
  const [checking, setChecking] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const { error, reportError, clearError } = useStudioError();
  const [loading, setLoading] = useState(desktop);
  const [sending, setSending] = useState(false);
  const [savingRetrieval, setSavingRetrieval] = useState(false);
  const [retrievalChange, setRetrievalChange] = useState<{
    workspaceId: string;
    conversationId: string | null;
    value: boolean;
  } | null>(null);
  const pendingSend = useRef(false);
  const finishedRuns = useRef(new Set<string>());
  const streamedRun = useRef<string | null>(null);
  const channel = useRef<Channel<StreamUpdate> | null>(null);
  useEffect(() => () => {
    channel.current = null;
    chatStream.cancelPending();
  }, [chatStream]);
  useEffect(() => chatStream.reconcile(storedMessages), [chatStream, storedMessages]);
  const reload = useCallback(async () => {
    if (!desktop) return;
    setLoading(true);
    clearError();
    try {
      const result = await invoke<Bootstrap>("bootstrap");
      setData(result);
      applySettings(result.settings);
      setActiveRun(result.activeRun);
      const saved = preference("workspace");
      const first =
        result.workspaces.find((item) => item.id === saved) ??
        result.workspaces[0];
      if (first) await openWorkspace(first.id);
    } catch (value) {
      reportError(value);
    } finally {
      setLoading(false);
    }
  }, [openWorkspace, reportError, clearError, applySettings]);

  const checkConnection = useCallback(async () => {
    if (!desktop) return;
    setChecking(true);
    setConnectionError(null);
    try {
      setHealth(await invoke<ModelHealth>("check_ollama"));
    } catch (value) {
      setHealth(null);
      setConnectionError(errorText(value));
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    void reload();
    void checkConnection();
  }, [reload, checkConnection]);

  useEffect(() => {
    if (!activeRun) return;
    const run = activeRun;
    let current = true;
    let timer: number;
    async function poll() {
      if (!current || finishedRuns.current.has(run.id)) return;
      try {
        const status = await invoke<string>("get_chat_run_status", {
          workspaceId: run.workspaceId,
          runId: run.id,
        });
        if (!current || finishedRuns.current.has(run.id)) return;
        if (status !== "running") {
          finishedRuns.current.add(run.id);
          setActiveRun((value) => value?.id === run.id ? null : value);
          await Promise.all([
            refreshMessages(run.workspaceId, run.conversationId),
            refreshConversations(run.workspaceId),
          ]);
          return;
        }
        if (streamedRun.current !== run.id && view.current.workspaceId === run.workspaceId && view.current.conversationId === run.conversationId)
          await refreshMessages(run.workspaceId, run.conversationId);
      } catch (value) {
        if (current) reportError(value);
      }
      if (current && !finishedRuns.current.has(run.id)) timer = window.setTimeout(poll, 1000);
    }
    timer = window.setTimeout(poll, 1000);
    return () => { current = false; window.clearTimeout(timer); };
  }, [activeRun, refreshMessages, refreshConversations, reportError]);

  const createWorkspace = async (name: string) => {
    const workspace = await invoke<Workspace>("create_workspace", { name });
    setData((current) =>
      current
        ? { ...current, workspaces: [...current.workspaces, workspace] }
        : current,
    );
    await openWorkspace(workspace.id);
  };

  const draftKey = `${workspaceId}:${conversationId ?? "new"}`;
  const contextSelection = contexts[draftKey] ?? emptyContext;
  const knowledgeScope = knowledgeScopes[draftKey] ?? null;
  const useGraph = graphLookups[draftKey] ?? false;
  const setUseGraph = (value: boolean) =>
    setGraphLookups((previous) => ({ ...previous, [draftKey]: value }));
  const knowledgeScopeReady = !knowledgeScope || !!(knowledgeScope.kind === "factDate" ? knowledgeScope.date : knowledgeScope.snapshotId);
  const setKnowledgeScope = (value: KnowledgeScope | null) =>
    setKnowledgeScopes((previous) => ({ ...previous, [draftKey]: value }));
  const setContextSelection = (items: ContextSelection[]) =>
    setContexts((previous) => ({ ...previous, [draftKey]: items }));
  const cachedDraft = useMemo(() => chatDrafts.get(draftKey), [draftKey]);
  const draft = drafts[draftKey] ?? cachedDraft ?? "";
  const writeDraft = (key: string, text: string) => {
    const persisted = text ? chatDrafts.set(key, text) : (chatDrafts.delete(key), true);
    setDrafts(current => ({ ...current, [key]: text }));
    return persisted;
  };
  const setDraft = (text: string) => writeDraft(draftKey, text);

  const autoRetrieval =
    retrievalChange?.workspaceId === workspaceId &&
    retrievalChange?.conversationId === conversationId
      ? retrievalChange.value
      : (conversations.find((item) => item.id === conversationId)
          ?.autoRetrieval ?? true);
  const setAutoRetrieval = async (enabled: boolean) => {
    const workspace = view.current.workspaceId;
    if (!workspace || savingRetrieval) return;
    setSavingRetrieval(true);
    setRetrievalChange({
      workspaceId: workspace,
      conversationId: view.current.conversationId,
      value: enabled,
    });
    const savedDraft = draft;
    const savedSelection = contextSelection;
    try {
      let conversation = view.current.conversationId;
      if (!conversation) {
        conversation = (await newConversation()).id;
        setRetrievalChange({
          workspaceId: workspace,
          conversationId: conversation,
          value: enabled,
        });
        if (writeDraft(`${workspace}:${conversation}`, savedDraft) && draftKey !== `${workspace}:${conversation}`) writeDraft(draftKey, "");
        setContexts((previous) => ({
          ...previous,
          [`${workspace}:${conversation}`]: savedSelection,
        }));
        setKnowledgeScopes((previous) => ({ ...previous, [draftKey]: null, [`${workspace}:${conversation}`]: knowledgeScope }));
        setGraphLookups((previous) => ({ ...previous, [draftKey]: false, [`${workspace}:${conversation}`]: useGraph }));
      }
      await invoke("set_conversation_retrieval", {
        workspaceId: workspace,
        conversationId: conversation,
        enabled,
      });
      await refreshConversations(workspace);
    } catch (value) {
      reportError(value);
    } finally {
      setSavingRetrieval(false);
      setRetrievalChange(null);
    }
  };

  const send = async (override?: string, cloud?: { question: string; selection: ContextSelection[]; consent: CloudConsent; knowledgeRoute: boolean }) => {
    const workspace = view.current.workspaceId;
    const content = (cloud?.question ?? override ?? draft).trim();
    const selectedContext = cloud?.selection ?? contextSelection;
    if (
      !workspace ||
      !content ||
      (!cloud && !knowledgeScopeReady) ||
      activeRun ||
      pendingSend.current ||
      savingRetrieval
    )
      return;
    pendingSend.current = true;
    setSending(true);
    clearError();
    let conversation = view.current.conversationId;
    let receivingPreparation = true;
    try {
      conversation = conversation ?? (await newConversation()).id;
      setContexts((previous) => ({
        ...previous,
        [`${workspace}:${conversation}`]: contextSelection,
      }));
      setKnowledgeScopes((previous) => ({ ...previous, [`${workspace}:${conversation}`]: knowledgeScope }));
      setGraphLookups((previous) => ({ ...previous, [`${workspace}:${conversation}`]: useGraph }));
      if (override === undefined && draftKey !== `${workspace}:${conversation}`) {
        if (writeDraft(`${workspace}:${conversation}`, draft)) writeDraft(draftKey, "");
      }
      const stream = new Channel<StreamUpdate>();
      channel.current = stream;
      let receivedTerminal = false;
      stream.onmessage = (update) => {
        if (channel.current !== stream || receivedTerminal ||
            update.workspaceId !== workspace || update.conversationId !== conversation || !chatStream.push(update)) return;
        if (update.status !== "running") {
          receivedTerminal = true;
          const visible = view.current.workspaceId === update.workspaceId && view.current.conversationId === update.conversationId;
          if (!visible) chatStream.forget(update.messageId);
          const alreadyRefreshed = finishedRuns.current.has(update.runId);
          finishedRuns.current.add(update.runId);
          setActiveRun((current) =>
            current?.id === update.runId ? null : current,
          );
          if (update.errorMessage) reportError(update.errorMessage);
          if (!alreadyRefreshed) {
            void refreshMessages(update.workspaceId, update.conversationId).catch(reportError);
            void refreshConversations(update.workspaceId).catch(reportError);
          }
        }
      };
      const prepareStarted = performance.now();
      const preparation = new Channel<Pick<RunSummary, "id" | "workspaceId" | "conversationId">>();
      preparation.onmessage = value => { if (receivingPreparation) setPreparingRun(value); };
      const run = await invoke<RunSummary>("start_chat", {
        workspaceId: workspace,
        conversationId: conversation,
        content,
        contextSelection: cloud ? selectedContext : knowledgeScope ? [] : selectedContext,
        ...(cloud ? { cloudConsent: cloud.consent, useKnowledgeRoute: cloud.knowledgeRoute } : {}),
        knowledgeScope,
        useGraph,
        onPreparing: preparation,
        onUpdate: stream,
      });
      if (performance.getEntriesByName("cortex.profile.enabled").length) {
        performance.clearMeasures("cortex.prepare-ipc");
        performance.measure("cortex.prepare-ipc", {
          start: prepareStarted,
          end: performance.now(),
        });
      }
      streamedRun.current = run.id;
      if (!finishedRuns.current.has(run.id)) setActiveRun(run);
      if (override === undefined) {
        writeDraft(draftKey, "");
        if (draftKey !== `${workspace}:${conversation}`) writeDraft(`${workspace}:${conversation}`, "");
      }
      if (draftKey !== `${workspace}:${conversation}`) {
        setKnowledgeScopes((previous) => ({ ...previous, [draftKey]: null }));
        setGraphLookups((previous) => ({ ...previous, [draftKey]: false }));
      }
      if (!finishedRuns.current.has(run.id)) {
        await refreshMessages(workspace, conversation);
        if (!finishedRuns.current.has(run.id)) await refreshConversations(workspace);
      }
    } catch (value) {
      reportError(value);
    } finally {
      receivingPreparation = false;
      pendingSend.current = false;
      setSending(false);
      setPreparingRun(null);
    }
  };

  const cancel = async () => {
    const run = activeRun ?? preparingRun;
    if (!run) return;
    await invoke("cancel_run", {
      workspaceId: run.workspaceId,
      runId: run.id,
    });
  };
  const getRun = useCallback(
    (runId: string) =>
      invoke<RunDetail>("get_run", {
        workspaceId: view.current.workspaceId,
        runId,
      }),
    [],
  );

  return {
    desktop,
    data,
    settings,
    settingsDraft,
    setSettingsDraft,
    workspaceId,
    conversationId,
    conversations,
    messages: storedMessages,
    chatStream,
    activeRun,
    preparingRun,
    draft,
    draftPersistence: draft ? chatDrafts.persisted(draftKey) ? "local" : "temporary" : null,
    setDraft,
    contextSelection,
    setContextSelection,
    knowledgeScope,
    setKnowledgeScope,
    knowledgeScopeReady,
    useGraph,
    setUseGraph,
    autoRetrieval,
    setAutoRetrieval,
    savingRetrieval,
    health,
    checking,
    connectionError,
    error,
    loading,
    loadingMessages,
    sending,
    creatingConversation,
    reload,
    createWorkspace,
    openWorkspace,
    openConversation,
    newConversation,
    renameConversation,
    checkConnection,
    send,
    cancel,
    saveSettings,
    reportError,
    clearError,
    getRun,
  };
}

export type Studio = ReturnType<typeof useStudio>;
export type GraphStudio = Pick<
  Studio,
  "desktop" | "workspaceId" | "openConversation" | "reportError"
>;
