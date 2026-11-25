import { BrandMark } from "./BrandMark";
import { AppDialog, openDialog } from "./AppDialog";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { CircleHelp, ListChecks, PanelLeft, X, Plus } from "lucide-react";
import type { Studio } from "../studio";
import { navigate, type PageName } from "../navigation";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
import { ModelStatus } from "./ModelStatus";
import { ConversationList } from "./ConversationList";
import { NavigationIcon } from "./NavigationIcon";
import { WindowControls, WindowDragRegion, handleWindowDrag } from "./WindowControls";
import { AppearanceControl } from "./AppearanceControl";
import { notify, Toast } from "./Toast";
import { TaskCenter } from "./TaskCenter";
import { WorkspaceHelp } from "./WorkspaceHelp";
export const pages = [
  { id: "chat", label: "对话" }, { id: "knowledge", label: "知识" },
  { id: "graph", label: "图谱" }, { id: "settings", label: "设置" },
] as const;
function Brand() {
  return <a className="prototype-brand" href="#/chat" aria-label="CortexOS 首页" title="CortexOS · 专属个人 AI 工作台">
    <BrandMark />
    <span>CortexOS</span>
  </a>;
}
export function AppShell({ studio, page, openCreateWorkspace, children }: {
  studio: Studio; page: PageName; openCreateWorkspace: () => void; children: ReactNode;
}) {
  const [chatCollapsed, setChatCollapsed] = useState(false);
  const [toolsCollapsed, setToolsCollapsed] = useState(true);
  const [collapsed, setCollapsed] = page === "chat" ? [chatCollapsed, setChatCollapsed] as const : [toolsCollapsed, setToolsCollapsed] as const;
  const [mobileOpen, setMobileOpen] = useState(false);
  const [narrow, setNarrow] = useState(() => matchMedia("(max-width: 900px)").matches);
  const drawer = useRef<HTMLDialogElement>(null);
  const tasks = useRef<HTMLDialogElement>(null);
  const help = useRef<HTMLDialogElement>(null);
  const [tasksOpen, setTasksOpen] = useState(false);
  const { desktop, workspaceId, reportError } = studio;
  useEffect(() => {
    if (!desktop || !workspaceId) return;
    let current = true;
    const listener = listen<{ workspaceId: string; title: string; briefStatus?: "ready" | "empty" | "failed" }>("reminder-due", ({ payload }) => {
      if (!current || payload.workspaceId !== workspaceId) return;
      const briefLabels = { ready: "简报已保存", empty: "本期未找到新闻", failed: "简报获取失败" };
      const label = payload.briefStatus ? briefLabels[payload.briefStatus] : "提醒到期";
      notify(`${label}：${payload.title}`, workspaceId);
    }).catch(error => { if (current) reportError(error); });
    return () => {
      current = false;
      void listener.then(unlisten => unlisten?.()).catch(reportError);
    };
  }, [desktop, workspaceId, reportError]);
  useEffect(() => {
    const query = matchMedia("(max-width: 900px)");
    const update = () => { setNarrow(query.matches); if (!query.matches) drawer.current?.close(); };
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  const conversation = studio.conversations.find(item => item.id === studio.conversationId);
  const workspace = studio.data?.workspaces.find(item => item.id === studio.workspaceId);
  const navigation = (mobile: boolean) => (
    <nav className="primary-navigation" aria-label={mobile ? "移动主导航" : "主导航"}>
      {pages.map(({id, label}) => <a key={id} href={`#/${id}`} title={label} aria-label={label} aria-current={page === id ? "page" : undefined} onClick={() => drawer.current?.close()}><NavigationIcon page={id} /><span>{label}</span></a>)}
    </nav>
  );
  const sidebar = (mobile: boolean) => <>
    {mobile && <Brand />}
    <button className="button secondary new-thread" disabled={!studio.desktop || !studio.workspaceId || studio.loading || studio.creatingConversation || studio.sending} onClick={() => {
      void studio.newConversation().then(item => { navigate("chat", { workspace: item.workspaceId, conversation: item.id }); drawer.current?.close(); }).catch(studio.reportError);
    }}><Plus size={16} aria-hidden="true" />{studio.creatingConversation ? "正在创建…" : "新建对话"}</button>
    {mobile && navigation(true)}
    <div className="sidebar-threads"><ConversationList studio={studio} onSelect={() => drawer.current?.close()} /></div>
    <div className="sidebar-workspace">
      <WorkspaceSwitcher studio={studio} page={page} openCreateWorkspace={() => { drawer.current?.close(); openCreateWorkspace(); }} />
    </div>
  </>;
  return <div className={`app-shell prototype-shell reference-chat page-${page} ${collapsed ? "sidebar-collapsed" : ""}`}>
    <a className="skip-link" href="#main-content" onClick={event => {event.preventDefault();document.getElementById("main-content")?.focus();}}>跳到主要内容</a>
    <aside id="primary-sidebar" className="prototype-sidebar reference-sidebar">{!mobileOpen && sidebar(false)}</aside>
    <div className="workspace-stage" onMouseDown={event => {
      if (studio.desktop && event.target instanceof Element && event.target.closest(".prototype-topbar,.page-heading")) {
        handleWindowDrag(event, studio.reportError);
      }
    }}>
      <header className="prototype-topbar">
        {!narrow && <Brand />}
        <button className="icon-button navigation-toggle" aria-label={narrow ? "打开导航" : collapsed ? "展开会话侧栏" : "收起会话侧栏"} aria-expanded={narrow ? mobileOpen : !collapsed} aria-controls={narrow ? "mobile-navigation" : "primary-sidebar"} aria-haspopup={narrow ? "dialog" : undefined} title={narrow ? "打开导航" : collapsed ? "展开会话侧栏" : "收起会话侧栏"} onClick={() => {
          if (narrow) { openDialog(drawer.current); setMobileOpen(true); } else setCollapsed(!collapsed);
        }}><PanelLeft size={18} /></button>
        {!narrow && navigation(false)}
        {(narrow || page === "chat") && <div className="topbar-title" title={page === "chat" ? conversation?.title ?? "新的对话" : undefined}><strong>{page === "chat" ? conversation?.title ?? "新的对话" : pages.find(item => item.id === page)?.label}</strong></div>}
        {studio.desktop ? <WindowDragRegion onError={studio.reportError} /> : <span className="topbar-spacer" />}
        <ModelStatus studio={studio} />
        <button className="icon-button" type="button" aria-label="任务中心" title="任务中心" aria-haspopup="dialog" onClick={() => {
          if (tasks.current?.open) { tasks.current.close(); return; }
          help.current?.close();
          openDialog(tasks.current);
          setTasksOpen(true);
        }}><ListChecks size={18} aria-hidden="true" /></button>
        <button className="icon-button" type="button" aria-label="使用帮助" title="使用帮助" aria-haspopup="dialog" onClick={() => {
          if (help.current?.open) { help.current.close(); return; }
          tasks.current?.close();
          openDialog(help.current);
        }}><CircleHelp size={18} aria-hidden="true" /></button>
        <AppearanceControl />
        {studio.desktop && <WindowControls onError={studio.reportError} />}
      </header>
      {children}
    </div>
    <AppDialog id="mobile-navigation" ref={drawer} className="mobile-sidebar panel-dialog" aria-labelledby="mobile-navigation-title" onClose={() => setMobileOpen(false)}>
      <header className="panel-header"><h2 id="mobile-navigation-title">导航与会话</h2><button className="icon-button" aria-label="关闭导航" onClick={() => drawer.current?.close()}><X size={18} /></button></header>
      <div className="panel-body">{mobileOpen && sidebar(true)}</div>
    </AppDialog>
    <TaskCenter dialog={tasks} open={tasksOpen} onClose={() => setTasksOpen(false)} desktop={studio.desktop} workspaceId={studio.workspaceId} workspaceName={workspace?.name} />
    <WorkspaceHelp dialog={help} page={page} />
    <Toast workspaceId={workspaceId} />
  </div>;
}
