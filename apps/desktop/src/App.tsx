import { closeDraftWindow, useDraftWindow } from "./draft-store";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { benchmarkReturn } from "./benchmark/reportNavigation";
import { AppDialog, openDialog } from "./components/AppDialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { X } from "lucide-react";
import { AppShell, pages } from "./components/AppShell";
import { lazyView } from "./components/lazyView";
import type { EditKnowledge } from "./knowledge-types";
import { ChatPage } from "./Chat";
import type { EditEdge } from "./RelationshipEditor";
import { errorText, useStudio, type Message } from "./studio";
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
const BenchmarkPage = lazyView(() => import("./benchmark/BenchmarkPage"), (module) => module.BenchmarkPage);

export default function App() {
  const studio = useStudio();
  const { desktop, workspaceId, openWorkspace, openConversation, reportError, view, loading } = studio;
  const graphStudio = useMemo(
    () => ({ desktop, workspaceId, openConversation, reportError }),
    [desktop, workspaceId, openConversation, reportError],
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
  const returnTarget=route.back?.startsWith("#/benchmark?")?benchmarkReturn(route.back,route.workspace,route.run):route.back;
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
  const workspaceDialog = useRef<HTMLDialogElement>(null);
  const volatileDialog = useRef<HTMLDialogElement>(null);
  useDraftWindow(() => openDialog(volatileDialog.current), reportError);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const openCreateWorkspace = () => {
    setWorkspaceName("");
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
            {returnTarget.startsWith("#/benchmark?")?"返回评测结果":returnTarget.startsWith("#/chat?") ? "返回回答" : "返回执行轨迹"}
          </a>
        )}
        <ErrorBoundary key={page} scope="page">
        {page === "chat" && (
          <ChatPage
            studio={studio}
            onCreateWorkspace={openCreateWorkspace}
            onSaveKnowledge={saveMessageAsKnowledge}
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
        {page === "benchmark" && <BenchmarkPage studio={studio} />}
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
              .createWorkspace(workspaceName)
              .then(() => {
                notify("工作区已创建");
                navigate(page);
                workspaceDialog.current?.close();
              })
              .catch((value) => setWorkspaceError(errorText(value)))
              .finally(() => setCreating(false));
          }}
        >
          <h2 id="workspace-title">创建工作区</h2>
          <p>为一个项目或一类工作保留独立的会话。</p>
          <div className="field">
            <label htmlFor="workspace-name">工作区名称</label>
            <input
              id="workspace-name"
              value={workspaceName}
              onChange={(event) => setWorkspaceName(event.target.value)}
              required
              maxLength={80}
              placeholder="例如：产品设计"
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
