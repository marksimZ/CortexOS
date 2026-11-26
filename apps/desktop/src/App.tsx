import { closeDraftWindow, useDraftWindow } from "./draft-store";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { AppDialog, openDialog } from "./components/AppDialog";
import { DropImportOverlay } from "./components/DropImportOverlay";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { X } from "lucide-react";
import { formatFolderSync, knowledge, type FolderSyncReport } from "./api/knowledge";
import { memoryApi } from "./api/memory";
import { AppShell, pages } from "./components/AppShell";
import { lazyView } from "./components/lazyView";
import type { EditKnowledge } from "./knowledge-types";
import { ChatPage } from "./Chat";
import type { EditEdge } from "./RelationshipEditor";
import { errorText, preference, remember, useStudio, type Message } from "./studio";
import { WORKSPACE_TEMPLATES, type WorkspaceTemplateId } from "./workspace-templates";
import { notify } from "./components/Toast";
import "./styles/tokens.css";
import "./styles/layout.css";
import "./styles/controls.css";
import "./styles/features.css";
import "./styles/motion.css";
import "./markdown.css";
import { useNavigation, navigate } from "./navigation";

const KnowledgePage = lazyView(() => import("./Knowledge"), (module) => module.KnowledgePage);
const KnowledgeEditor = lazyView(() => import("./KnowledgeEditor"), (module) => module.KnowledgeEditor);
const GraphPage = lazyView(() => import("./Graph"), (module) => module.GraphPage);
const RelationshipEditor = lazyView(() => import("./RelationshipEditor"), (module) => module.RelationshipEditor);
const SettingsPage = lazyView(() => import("./Settings"), (module) => module.SettingsPage);

export default function App() {
  const studio = useStudio();
  const { desktop, workspaceId, openWorkspace, openConversation, startTopicChat, reportError, view, loading } = studio;
  const graphStudio = useMemo(
    () => ({ desktop, workspaceId, openConversation, startTopicChat, reportError }),
    [desktop, workspaceId, openConversation, startTopicChat, reportError],
  );
  const [edgeEdit, setEdgeEdit] = useState<EditEdge | null>(null);
  const [knowledgeEdit, setKnowledgeEdit] = useState<EditKnowledge | null>(
    null,
  );
  const saveMessageAsKnowledge = useCallback((source: Message, excerpt: string) => {
    setKnowledgeEdit({ workspaceId: source.workspaceId, source, excerpt });
  }, []);
  const [knowledgeSelection, setKnowledgeSelection] = useState<{
    workspace: string;
    id: string;
  } | null>(null);
  const [knowledgeRevision, setKnowledgeRevision] = useState(0);
  const route = useNavigation();
  const page = route.page;
  const returnTarget = route.back;
  const { conversation: routeConversation, workspace: routeWorkspace } = route;
  const knownWorkspace = studio.data?.workspaces.some(item => item.id === routeWorkspace);
  useEffect(() => {
    if (loading || knownWorkspace === undefined || !routeWorkspace) return;
    if (!knownWorkspace) {
      reportError("目标工作区不存在。请选择可用工作区。");
      return;
    }
    const request = new AbortController();
    const apply = async () => {
      if (view.current.workspaceId !== routeWorkspace) {
        await openWorkspace(routeWorkspace, routeConversation, request.signal);
      } else if (routeConversation && view.current.conversationId !== routeConversation) {
        await openConversation(routeWorkspace, routeConversation);
      }
    };
    void apply().catch(error => { if (!request.signal.aborted) reportError(error); });
    return () => request.abort();
  }, [routeWorkspace, routeConversation, loading, knownWorkspace, view, openWorkspace, openConversation, reportError]);
  const mainRef = useRef<HTMLElement>(null);
  const previousPage = useRef(page);
  const folderSyncs = useRef(new Map<string, Promise<FolderSyncReport[]>>());
  useEffect(() => {
    if (!studio.desktop) return;
    let active = true;
    const navSub = listen<{
      workspaceId: string;
      conversationId?: string | null;
      nodeId?: string | null;
    }>("quickbar-navigate", ({ payload }) => {
      if (!active || !payload.workspaceId) return;
      if (payload.nodeId) {
        setKnowledgeSelection({ workspace: payload.workspaceId, id: payload.nodeId });
        setKnowledgeRevision((n) => n + 1);
        navigate("knowledge", { workspace: payload.workspaceId, node: payload.nodeId });
        return;
      }
      void studio
        .openWorkspace(payload.workspaceId, payload.conversationId ?? undefined)
        .then(async () => {
          await studio.refreshConversations(payload.workspaceId);
          if (payload.conversationId) {
            await studio.openConversation(payload.workspaceId, payload.conversationId);
          }
          navigate("chat", {
            workspace: payload.workspaceId,
            conversation: payload.conversationId ?? undefined,
          });
        })
        .catch(reportError);
    }).catch(() => undefined);

    const savedSub = listen<{ workspaceId: string; nodeId: string }>(
      "quickbar-knowledge-saved",
      ({ payload }) => {
        if (!active) return;
        setKnowledgeRevision((n) => n + 1);
        if (payload.workspaceId === studio.view.current.workspaceId) {
          notify("随手记已存入当前工作区知识库", payload.workspaceId);
        }
      },
    ).catch(() => undefined);

    return () => {
      active = false;
      void navSub.then((unlisten) => unlisten?.()).catch(() => {});
      void savedSub.then((unlisten) => unlisten?.()).catch(() => {});
    };
  }, [studio.desktop, studio.openWorkspace, studio.openConversation, studio.refreshConversations, studio.view, reportError]);
  useEffect(() => {
    const ws = studio.workspaceId;
    if (!studio.desktop || !ws) return;
    let active = true;
    let sync = folderSyncs.current.get(ws);
    if (!sync) {
      sync = knowledge("sync_workspace_watched_folders", { workspaceId: ws })
        .finally(() => folderSyncs.current.delete(ws));
      folderSyncs.current.set(ws, sync);
    }
    void sync.then(reports => {
      if (!active) return;
      const totals = { imported: 0, updated: 0, unchanged: 0, skipped: 0, remaining: 0 };
      const errors = [];
      for (const report of reports) {
        totals.imported += report.imported;
        totals.updated += report.updated;
        totals.unchanged += report.unchanged;
        totals.skipped += report.skipped;
        totals.remaining += report.remaining;
        if (report.error) errors.push(`${report.folder}：${report.error}`);
      }
      if (totals.imported || totals.updated) setKnowledgeRevision(n => n + 1);
      if (totals.imported || totals.updated || totals.skipped || totals.remaining)
        notify(`本地文件夹同步：${formatFolderSync(totals)}`, ws);
      if (errors.length) reportError(`文件夹同步未完成：${errors.join("；")}`);
    }).catch(error => { if (active) reportError(`文件夹同步失败：${errorText(error)}`); });
    return () => { active = false; };
  }, [studio.desktop, studio.workspaceId, reportError]);
  const workspaceDialog = useRef<HTMLDialogElement>(null);
  const afterWorkspace = useRef<"chat" | "cloud" | null>(null);
  const volatileDialog = useRef<HTMLDialogElement>(null);
  const namingDialog = useRef<HTMLDialogElement>(null);
  const namingPromptedRef = useRef(false);
  const [assistantNameDraft, setAssistantNameDraft] = useState("");
  const [savingAssistantName, setSavingAssistantName] = useState(false);
  const [assistantNameError, setAssistantNameError] = useState<string | null>(null);
  useEffect(() => {
    if (
      !studio.desktop ||
      studio.loading ||
      !studio.personaLoaded ||
      namingPromptedRef.current
    ) {
      return;
    }
    if (studio.assistantName || preference("assistant-naming-prompted") === "true") {
      namingPromptedRef.current = true;
      return;
    }
    namingPromptedRef.current = true;
    openDialog(namingDialog.current);
  }, [studio.desktop, studio.loading, studio.personaLoaded, studio.assistantName]);
  useDraftWindow(() => openDialog(volatileDialog.current), reportError);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspaceTemplate, setWorkspaceTemplate] = useState<WorkspaceTemplateId>("blank");
  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const openCreateWorkspace = (next?: "chat" | "cloud") => {
    afterWorkspace.current = next ?? null;
    setWorkspaceName("");
    setWorkspaceTemplate("blank");
    setWorkspaceError(null);
    openDialog(workspaceDialog.current);
  };
  useEffect(() => {
    document.title = `${pages.find((item) => item.id === page)?.label} · CortexOS`;
    if (previousPage.current !== page && !(page === "chat" && route.message))
      mainRef.current?.focus();
    previousPage.current = page;
  }, [page, route.message]);
  return (
    <AppShell
      studio={studio}
      page={page}
      openCreateWorkspace={openCreateWorkspace}
    >
      <main
        id="main-content"
        className="main-content"
        ref={mainRef}
        tabIndex={-1}
      >
        {studio.loading && (
          <p className="startup-status" role="status">
            正在读取本地工作区…
          </p>
        )}
        {studio.error && (
          <div className="error-banner" role="alert">
            <div>
              <strong>操作未完成</strong>
              <p>{studio.error}</p>
              {!studio.data && studio.desktop && (
                <button
                  className="button text-button"
                  onClick={() => {
                    void studio.reload();
                  }}
                >
                  重试读取
                </button>
              )}
            </div>
            <button
              className="icon-button"
              aria-label="关闭错误提示"
              onClick={studio.clearError}
            >
              <X size={18} aria-hidden="true" />
            </button>
          </div>
        )}
        {returnTarget && (
          <a className="button text-button return-to-trace" href={returnTarget}>
            {returnTarget.startsWith("#/chat?") ? "返回回答" : "返回知识图谱"}
          </a>
        )}
        <ErrorBoundary key={page} scope="page">
        {page === "chat" && (
          <ChatPage
            studio={studio}
            onCreateWorkspace={openCreateWorkspace}
            onSaveKnowledge={saveMessageAsKnowledge}
            onQuickSavedKnowledge={() => setKnowledgeRevision((n) => n + 1)}
          />
        )}
        {page === "knowledge" && (
          <KnowledgePage
            key={studio.workspaceId ?? "preview"}
            studio={studio}
            selected={
              route.node ??
              (knowledgeSelection?.workspace === studio.workspaceId
                ? knowledgeSelection.id
                : null)
            }
            requestedVersion={route.version}
            onSelect={(id) => {
              setKnowledgeSelection(
                id && studio.workspaceId
                  ? { workspace: studio.workspaceId, id }
                  : null,
              );
              navigate("knowledge", {
                workspace: studio.workspaceId,
                node: id,
              });
            }}
            revision={knowledgeRevision}
            onEdit={setKnowledgeEdit}
          />
        )}
        {page === "graph" && (
          <GraphPage
            studio={graphStudio}
            revision={knowledgeRevision}
            onEdit={setKnowledgeEdit}
            onEditEdge={setEdgeEdit}
          />
        )}
        {page === "settings" && <SettingsPage studio={studio} />}
        </ErrorBoundary>
      </main>
      {edgeEdit && (
        <RelationshipEditor
          request={edgeEdit}
          onClose={() => setEdgeEdit(null)}
          onSaved={() => {
            notify("关系已保存");
            setEdgeEdit(null);
            setKnowledgeRevision((n) => n + 1);
          }}
        />
      )}
      {knowledgeEdit && (
        <KnowledgeEditor
          request={knowledgeEdit}
          onClose={() => setKnowledgeEdit(null)}
          onSaved={(workspace, id) => {
            notify("知识草稿已保存");
            setKnowledgeEdit(null);
            setKnowledgeSelection({ workspace, id });
            setKnowledgeRevision((n) => n + 1);
            navigate(
              knowledgeEdit.returnTo === "graph" ? "graph" : "knowledge",
              {
                workspace,
                node: id,
                mode:
                  knowledgeEdit.returnTo === "graph" ? "knowledge" : undefined,
              },
            );
          }}
        />
      )}
      <DropImportOverlay
        studio={studio}
        page={page}
        onDocumentsImported={(ws, docs) => {
          setKnowledgeRevision((n) => n + 1);
          if (page === "knowledge" && docs[0]) {
            setKnowledgeSelection({ workspace: ws, id: docs[0].nodeId });
            navigate("knowledge", { workspace: ws, node: docs[0].nodeId });
          }
        }}
      />
      <AppDialog
        ref={namingDialog}
        className="form-dialog"
        aria-labelledby="naming-assistant-title"
        onCancel={() => {
          remember("assistant-naming-prompted", "true");
        }}
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            const trimmed = assistantNameDraft.trim();
            if (!trimmed || savingAssistantName) return;
            setSavingAssistantName(true);
            setAssistantNameError(null);
            void memoryApi("get_global_persona", undefined)
              .then((persona) =>
                memoryApi("save_global_persona", {
                  input: { ...persona, assistantName: trimmed },
                }),
              )
              .then((saved) => {
                remember("assistant-naming-prompted", "true");
                studio.setAssistantName(saved.assistantName?.trim() || trimmed);
                notify(`你好，我是 ${trimmed}！随时在「设置 → 我的助手」可修改称呼`);
                namingDialog.current?.close();
              })
              .catch((value) => setAssistantNameError(errorText(value)))
              .finally(() => setSavingAssistantName(false));
          }}
        >
          <h2 id="naming-assistant-title">想给你的专属 AI 起个名字吗？</h2>
          <p>
            给它起一个顺口的名字，后续对话、今日聚焦与全局随手记都会用这个称呼陪伴你（随时可在「设置 → 我的助手」修改）。
          </p>
          <div className="field">
            <label htmlFor="first-run-assistant-name">助手称呼（选填）</label>
            <input
              id="first-run-assistant-name"
              data-initial-focus
              value={assistantNameDraft}
              onChange={(event) => setAssistantNameDraft(event.target.value)}
              maxLength={40}
              placeholder="例如：Jarvis / 星期五 / 小脑"
              readOnly={savingAssistantName}
            />
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "6px", marginBottom: "10px" }}>
            {["小脑", "星期五", "Jarvis", "小星"].map((sample) => (
              <button
                key={sample}
                type="button"
                className="button secondary"
                style={{ padding: "3px 10px", fontSize: "12px" }}
                disabled={savingAssistantName}
                onClick={() => setAssistantNameDraft(sample)}
              >
                {sample}
              </button>
            ))}
          </div>
          {assistantNameError && (
            <p className="form-error" role="alert">
              {assistantNameError}
            </p>
          )}
          <div className="dialog-actions">
            <button
              className="button secondary"
              type="button"
              disabled={savingAssistantName}
              onClick={() => {
                remember("assistant-naming-prompted", "true");
                namingDialog.current?.close();
              }}
            >
              暂不起名
            </button>
            <button
              className="button primary"
              disabled={savingAssistantName || !assistantNameDraft.trim()}
            >
              {savingAssistantName ? "正在保存…" : "就叫这个名字"}
            </button>
          </div>
        </form>
      </AppDialog>
      <AppDialog ref={volatileDialog} aria-labelledby="volatile-title" aria-describedby="volatile-detail" className="form-dialog">
        <h2 id="volatile-title">仍有临时草稿</h2>
        <p id="volatile-detail">有修改只保留在当前窗口。关闭窗口后无法恢复这些内容。</p>
        <div className="dialog-actions">
          <button type="button" className="button secondary" data-initial-focus onClick={() => volatileDialog.current?.close()}>继续编辑</button>
          <button type="button" className="button primary" onClick={() => void closeDraftWindow().catch(reportError)}>仍然关闭窗口</button>
        </div>
      </AppDialog>
      <AppDialog
        ref={workspaceDialog}
        className="form-dialog"
        aria-labelledby="workspace-title"
        onCancel={(event) => {
          if (creating) event.preventDefault();
        }}
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            setCreating(true);
            setWorkspaceError(null);
            void studio
              .createWorkspace(workspaceName, workspaceTemplate)
              .then((workspace) => {
                const chosen = WORKSPACE_TEMPLATES.find((t) => t.id === workspaceTemplate);
                notify(
                  chosen?.assistantLabel
                    ? `工作区已创建，并自动启用「${chosen.assistantLabel}」`
                    : "工作区已创建",
                );
                if (afterWorkspace.current === "cloud") {
                  navigate("settings", { section: "cloud", workspace: workspace.id });
                } else if (afterWorkspace.current === "chat") {
                  navigate("chat", { workspace: workspace.id });
                } else if (page === "settings") {
                  navigate("settings", { section: route.params.get("section"), workspace: workspace.id });
                } else {
                  navigate(page);
                }
                workspaceDialog.current?.close();
              })
              .catch((value) => setWorkspaceError(errorText(value)))
              .finally(() => setCreating(false));
          }}
        >
          <h2 id="workspace-title">创建工作区</h2>
          <p>为一个项目、一门课程或一类生活事务保留独立的资料、记忆与会话。</p>
          <div className="field">
            <label>选择使用场景（可选）</label>
            <div
              role="radiogroup"
              aria-label="工作区场景模板"
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
                gap: "8px",
              }}
            >
              {WORKSPACE_TEMPLATES.map((tpl) => {
                const selected = workspaceTemplate === tpl.id;
                return (
                  <button
                    key={tpl.id}
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    disabled={creating}
                    onClick={() => setWorkspaceTemplate(tpl.id)}
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "flex-start",
                      gap: "3px",
                      padding: "8px 10px",
                      borderRadius: "var(--radius-control)",
                      border: selected
                        ? "1.5px solid var(--accent, #2563eb)"
                        : "1px solid var(--line)",
                      background: selected
                        ? "var(--accent-soft, rgba(37, 99, 235, 0.08))"
                        : "var(--surface)",
                      textAlign: "left",
                      cursor: "pointer",
                      color: "inherit",
                    }}
                  >
                    <strong style={{ fontSize: "0.85rem" }}>{tpl.label}</strong>
                    <small style={{ color: "var(--muted)", fontSize: "0.75rem", lineHeight: 1.35 }}>
                      {tpl.tagline}
                    </small>
                  </button>
                );
              })}
            </div>
          </div>
          <div className="field">
            <label htmlFor="workspace-name">工作区名称</label>
            <input
              id="workspace-name"
              value={workspaceName}
              onChange={(event) => setWorkspaceName(event.target.value)}
              required
              maxLength={80}
              placeholder={
                WORKSPACE_TEMPLATES.find((t) => t.id === workspaceTemplate)?.placeholder ??
                "例如：产品设计"
              }
              readOnly={creating}
              aria-describedby={workspaceError ? "workspace-error" : undefined}
            />
          </div>
          {workspaceError && (
            <p id="workspace-error" className="form-error" role="alert">
              {workspaceError}
            </p>
          )}
          <div className="dialog-actions">
            <button
              className="button secondary"
              type="button"
              disabled={creating}
              onClick={() => workspaceDialog.current?.close()}
            >
              取消
            </button>
            <button className="button primary" disabled={creating}>
              {creating ? "正在创建…" : "创建工作区"}
            </button>
          </div>
        </form>
      </AppDialog>
    </AppShell>
  );
}
