import { AppDialog, openDialog } from "./components/AppDialog";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowDown, PanelRight, Pencil } from "lucide-react";
import { navigate, useNavigation } from "./navigation";
import { ChatMessage } from "./components/ChatMessage";
import { ChatComposer } from "./components/ChatComposer";
import { EmptyState } from "./components/EmptyState";
import { CognitiveInspector } from "./components/CognitiveInspector";
import { ChatEvidence } from "./components/ChatEvidence";
import { useChatScroll } from "./components/useChatScroll";
import { PaneDivider, splitStyle } from "./components/PaneDivider";
import { notify } from "./components/Toast";
import { errorText, preference, remember, type Message, type Studio } from "./studio";
import { runtimeRoles, type ChatRuntimeRoute, type CloudDisclosure, type CloudDraft, type CloudKnowledgeGrant, type Profile,
  type RuntimeConnection, type RuntimeRole } from "./runtime-profiles";

const cloudNames: Record<string, string> = {
  openai: "OpenAI", anthropic: "Anthropic", gemini: "Google Gemini", compatible: "OpenAI 兼容服务",
};

export function ChatPage({
  studio,
  onCreateWorkspace,
  onSaveKnowledge,
}: {
  studio: Studio;
  onCreateWorkspace: () => void;
  onSaveKnowledge: (message: Message, excerpt: string) => void;
}) {
  const route = useNavigation();
  useEffect(() => {
    if (!route.message || studio.loadingMessages) return;
    const message = document.getElementById(`message-${route.message}`);
    const element = (route.citation === undefined ? null : message?.querySelector<HTMLElement>(`[data-answer-citation="${route.citation}"]`)) ?? message;
    element?.scrollIntoView({ block: "center" });
    element?.focus({ preventScroll: true });
  }, [route.message, route.citation, studio.messages.length, studio.loadingMessages]);
  const conversation = studio.conversations.find(
    (item) => item.id === studio.conversationId,
  );
  const preparing = studio.preparingRun?.conversationId === studio.conversationId && !studio.activeRun;
  const currentRunning = preparing || studio.activeRun?.conversationId === studio.conversationId;
  const knowledgeRoute = studio.autoRetrieval || !!studio.knowledgeScope || !!studio.contextSelection.length || studio.useGraph;
  const [runtime, setRuntime] = useState<{ workspace: string; knowledgeRoute: boolean; profile?: Profile; localBackup: string | null; disclosure: CloudDisclosure }>();
  const currentRuntime = runtime?.workspace === studio.workspaceId && runtime.knowledgeRoute === knowledgeRoute ? runtime : undefined;
  const target = currentRuntime?.profile?.generation;
  const cloudAnswer = target?.kind === "cloud";
  const cloudLookup = currentRuntime?.profile?.embedding.kind === "cloud" || currentRuntime?.profile?.reranking.kind === "cloud";
  const cloudTurn = cloudAnswer || cloudLookup;
  const modelName = (target?.kind !== "off" ? target?.model : null) ?? studio.settings.model;
  const modelReady = cloudAnswer
    ? currentRuntime?.disclosure.generation?.connection.probes.some(item => item.model === modelName && item.capability === "generation" && item.state === "verified")
    : studio.health?.models.some(model => model.name === modelName || model.name === currentRuntime?.localBackup);
  const cloudDialog = useRef<HTMLDialogElement>(null);
  const [cloudDraft, setCloudDraft] = useState<CloudDraft>();
  const [shareSelection, setShareSelection] = useState(false);
  const [shareGranted, setShareGranted] = useState(false);
  const [allowRanking, setAllowRanking] = useState(false);
  useEffect(() => {
    const workspace = studio.workspaceId;
    if (!studio.desktop || !workspace) return;
    let current = true;
    void invoke<ChatRuntimeRoute>("chat_runtime_route", { workspaceId: workspace, knowledge: knowledgeRoute }).then(async selected => {
      if (!current) return;
      const profile = selected.profile ?? undefined;
      const disclosure: CloudDisclosure = {};
      const roles = Object.keys(runtimeRoles) as RuntimeRole[];
      if (profile && roles.some(role => profile[role].kind === "cloud")) {
        const rows = await invoke<RuntimeConnection[]>("list_runtime_cloud_connections", { workspaceId: workspace });
        const grants = new Map<string, Promise<CloudKnowledgeGrant[]>>();
        await Promise.all(roles.map(async role => {
          const target = profile[role];
          if (target.kind !== "cloud") return;
          const connection = rows.find(item => item.id === target.connectionId);
          if (!connection) return;
          let allowed: CloudKnowledgeGrant[] = [];
          if (role !== "embedding") {
            if (!grants.has(target.connectionId)) {
              grants.set(target.connectionId, invoke<{ grants: CloudKnowledgeGrant[] }>(
                "list_model_cloud_knowledge", { workspaceId: workspace, connectionId: target.connectionId },
              ).then(result => result.grants.filter(item => item.available)).catch(() => []));
            }
            allowed = await grants.get(target.connectionId) ?? [];
          }
          disclosure[role] = { connection, grants: allowed };
        }));
      }
      if (current) setRuntime({ workspace, knowledgeRoute, profile, localBackup: selected.localBackup, disclosure });
    }).catch(() => {});
    return () => { current = false; };
  }, [studio.desktop, studio.workspaceId, knowledgeRoute, studio.settings]);
  useEffect(() => {
    if (cloudDraft) openDialog(cloudDialog.current);
  }, [cloudDraft]);
  const draftTooLong = useMemo(() => new TextEncoder().encode(studio.draft.trim()).length > 16_384, [studio.draft]);
  const canSend =
    studio.desktop &&
    !!studio.workspaceId &&
    !!modelReady &&
    !!studio.draft.trim() &&
    !draftTooLong &&
    (cloudAnswer || studio.knowledgeScopeReady) &&
    !studio.activeRun &&
    !studio.sending &&
    !studio.savingRetrieval &&
    !studio.loadingMessages;
  const scroll = useChatScroll(studio.conversationId, studio.messages.length, studio.messages, !!route.message);
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
  const [desktopEvidence, setDesktopEvidence] = useState(() => preference("evidence-open") !== "false");
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
  const latestRun = useMemo(() => [...studio.messages].reverse().find(message => message.role === "assistant" && message.modelRunId)?.modelRunId ?? null, [studio.messages]);
  const evidenceRun = studio.messages.find(message => message.role === "assistant" && message.id === route.message)?.modelRunId ?? latestRun;

  useEffect(() => {
    if (!currentRunning) setCancelling(false);
  }, [currentRunning]);
  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(null), 2500);
    return () => window.clearTimeout(timer);
  }, [copied]);

  const send = async (content?: string) => {
    if (cloudTurn && currentRuntime?.profile && studio.workspaceId) {
      const question = (content ?? studio.draft).trim();
      if (question) {
        setShareSelection(false);
        setShareGranted(false);
        setAllowRanking(false);
        setCloudDraft({
          question, retry: content !== undefined,
          workspace: studio.workspaceId, conversation: studio.conversationId,
          selection: studio.knowledgeScope ? [] : studio.contextSelection,
          profile: currentRuntime.profile, disclosure: currentRuntime.disclosure,
          knowledgeRoute,
        });
      }
      return;
    }
    navigate("chat", {
      workspace: studio.workspaceId,
      conversation: studio.conversationId,
    });
    return studio.send(content);
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
            <button ref={evidenceToggle} hidden={evidenceOpen} className="icon-button evidence-toggle" aria-label="打开回答依据" aria-expanded={evidenceOpen} aria-controls="chat-evidence" aria-haspopup={!wide ? "dialog" : undefined} title="打开回答依据" onClick={() => setEvidenceOpen(true)}><PanelRight size={17} /></button>
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
              {studio.messages.map((message) => (
                <ChatMessage
                  key={message.id}
                  message={message}
                  located={route.message === message.id}
                  copied={copied === message.id}
                  copy={copy}
                  onSaveKnowledge={onSaveKnowledge}
                  setSelectedRun={setSelectedRun}
                  canRetry={canRetry}
                  reportError={studio.reportError}
                  retry={retry}
                />
              ))}
            </div>
          ) : (
            <EmptyState
              studio={studio}
              modelReady={modelReady}
              onCreateWorkspace={onCreateWorkspace}
            />
          )}
        </div>
        {studio.messages.length > 0 && scroll.showJump && (
          <button
            className="jump-latest button secondary"
            onClick={scroll.jump}
          >
            <ArrowDown size={15} aria-hidden="true" />
            回到最新内容
          </button>
        )}
        <ChatComposer
          studio={studio}
          modelReady={modelReady}
          modelName={modelName}
          cloudAnswer={cloudAnswer}
          cloudLookup={cloudLookup}
          currentRunning={currentRunning}
          preparing={preparing}
          cancelling={cancelling}
          setCancelling={setCancelling}
          canSend={canSend}
          draftTooLong={draftTooLong}
          send={send}
        />
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
          });
        }}>
          <h2 id="cloud-send-title">确认发送到云端</h2>
          <ul className="cloud-runtime-capabilities">{(Object.keys(runtimeRoles) as RuntimeRole[]).map(role => {
            const target = cloudDraft.profile[role];
            if (target.kind !== "cloud") return null;
            const connection = cloudDraft.disclosure[role]?.connection;
            return <li key={role}><span><strong>{runtimeRoles[role]}</strong>　{connection && (cloudNames[connection.provider] ?? connection.label)} / {target.model}
              {connection?.address && <><br />{connection.address}</>}</span></li>;
          })}</ul>
          <p>云端回答发送本次问题。向量查找启用且索引可用时发送问题。历史对话留在本机，云端服务可能计费。</p>
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
