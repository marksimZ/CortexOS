import { useEffect, useRef, useState } from "react";
import { Archive, ArchiveRestore, HardDrive, MessageSquare, Pencil, Plus, Search, Trash2, X } from "lucide-react";
import { chat, type ChatSearchHit } from "../api/chat";
import { errorText, type Conversation, type Studio } from "../studio";
import { navigate } from "../navigation";
import { AppDialog, openDialog } from "./AppDialog";
import { notify } from "./Toast";

type ContextMenuState = {
  conversation: Conversation;
  x: number;
  y: number;
};

export function ConversationList({
  studio,
  onSelect,
}: {
  studio: Studio;
  onSelect?: () => void;
}) {
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const [renameTarget, setRenameTarget] = useState<Conversation | null>(null);
  const [renameTitle, setRenameTitle] = useState("");
  const [renameError, setRenameError] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Conversation | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const renameDialog = useRef<HTMLDialogElement>(null);
  const deleteDialog = useRef<HTMLDialogElement>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchHits, setSearchHits] = useState<ChatSearchHit[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  useEffect(() => {
    setSearchQuery("");
    setSearchHits(null);
    setSearchError(null);
  }, [studio.workspaceId]);

  useEffect(() => {
    const workspaceId = studio.workspaceId;
    const trimmed = searchQuery.trim();
    if (!studio.desktop || !workspaceId || !trimmed) {
      setSearchHits(null);
      setSearchError(null);
      setSearching(false);
      return;
    }
    let active = true;
    const timer = window.setTimeout(() => {
      setSearching(true);
      setSearchError(null);
      void chat("search_chat_messages", { workspaceId, query: trimmed, limit: 20 })
        .then((hits) => {
          if (active) setSearchHits(hits);
        })
        .catch((value) => {
          if (active) setSearchError(errorText(value));
        })
        .finally(() => {
          if (active) setSearching(false);
        });
    }, 180);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [studio.desktop, studio.workspaceId, searchQuery]);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("click", close);
    window.addEventListener("contextmenu", close);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("contextmenu", close);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [menu]);

  const activeConversations = studio.conversations.filter((c) => !c.archived);
  const archivedConversations = studio.conversations.filter((c) => c.archived);

  const openContextMenu = (event: React.MouseEvent, conversation: Conversation) => {
    if (!studio.desktop) return;
    event.preventDefault();
    event.stopPropagation();
    const menuWidth = 156;
    const menuHeight = 128;
    const x = Math.min(event.clientX, Math.max(8, window.innerWidth - menuWidth - 8));
    const y = Math.min(event.clientY, Math.max(8, window.innerHeight - menuHeight - 8));
    setMenu({ conversation, x, y });
  };

  const renderItem = (conversation: Conversation) => (
    <li key={conversation.id}>
      <button
        className="conversation-item"
        aria-current={conversation.id === studio.conversationId ? "true" : undefined}
        title={`${conversation.title}（右键管理）`}
        onContextMenu={(event) => openContextMenu(event, conversation)}
        onClick={() => {
          void studio
            .openConversation(conversation.workspaceId, conversation.id)
            .then(() => {
              navigate("chat", {
                workspace: conversation.workspaceId,
                conversation: conversation.id,
              });
              onSelect?.();
            })
            .catch(studio.reportError);
        }}
      >
        <MessageSquare size={15} aria-hidden="true" />
        <span>{conversation.title}</span>
        {studio.activeRun?.conversationId === conversation.id && (
          <span className="running-mark" aria-label="生成中" />
        )}
      </button>
    </li>
  );

  return (
    <>
      <div className="sidebar-heading">
        <h2>会话</h2>
        <span className="count">{activeConversations.length}</span>
      </div>
      {studio.desktop && studio.workspaceId && (
        <div className="conversation-search-box">
          <Search size={14} aria-hidden="true" className="conversation-search-icon" />
          <input
            type="search"
            aria-label="搜索历史对话"
            placeholder="搜索历史对话…"
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
          />
          {searchQuery && (
            <button
              type="button"
              className="icon-button conversation-search-clear"
              aria-label="清空对话搜索"
              onClick={() => setSearchQuery("")}
            >
              <X size={13} aria-hidden="true" />
            </button>
          )}
        </div>
      )}
      <button
        className="button secondary new-conversation"
        disabled={
          !studio.desktop ||
          !studio.workspaceId ||
          studio.loading ||
          studio.creatingConversation ||
          studio.sending
        }
        onClick={() => {
          void studio
            .newConversation()
            .then((conversation) => {
              navigate("chat", {
                workspace: conversation.workspaceId,
                conversation: conversation.id,
              });
              onSelect?.();
            })
            .catch(studio.reportError);
        }}
      >
        <Plus size={16} aria-hidden="true" />
        {studio.creatingConversation ? "正在创建…" : "新建对话"}
      </button>
      {searchQuery.trim() ? (
        <div className="conversation-search-results" role="region" aria-label="对话搜索结果">
          {searching && <p className="conversation-search-state" role="status">正在搜索…</p>}
          {searchError && (
            <p className="form-error" role="alert">
              {searchError}
            </p>
          )}
          {!searching && !searchError && searchHits && searchHits.length === 0 && (
            <p className="conversation-search-state">未找到匹配的聊天记录</p>
          )}
          {searchHits && searchHits.length > 0 && (
            <ul className="conversation-list">
              {searchHits.map((hit) => (
                <li key={`${hit.conversationId}:${hit.messageId}`}>
                  <button
                    type="button"
                    className="conversation-item conversation-search-hit"
                    onClick={() => {
                      const workspaceId = studio.workspaceId;
                      if (!workspaceId) return;
                      void studio
                        .openConversation(workspaceId, hit.conversationId)
                        .then(() => {
                          navigate("chat", {
                            workspace: workspaceId,
                            conversation: hit.conversationId,
                            message: hit.messageId,
                          });
                          onSelect?.();
                        })
                        .catch(studio.reportError);
                    }}
                  >
                    <div className="conversation-search-meta">
                      <strong>{hit.conversationTitle}</strong>
                      <span>
                        {hit.role === "user" ? "你" : "助手"}
                        {hit.archived ? " · 已归档" : ""}
                      </span>
                    </div>
                    <span className="conversation-search-snippet">{hit.snippet}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : (
        <>
          {activeConversations.length ? (
            <ul className="conversation-list">
              {activeConversations.map(renderItem)}
            </ul>
          ) : (
            <div className="sidebar-empty">
              <MessageSquare size={22} aria-hidden="true" />
              <p>{archivedConversations.length ? "当前无进行中会话" : "还没有会话"}</p>
            </div>
          )}
          {archivedConversations.length > 0 && (
            <details className="archived-conversations">
              <summary>已归档（{archivedConversations.length}）</summary>
              <ul className="conversation-list">
                {archivedConversations.map(renderItem)}
              </ul>
            </details>
          )}
        </>
      )}
      <p className="sidebar-footer">
        <HardDrive size={14} aria-hidden="true" />
        {studio.desktop ? "会话自动保存在本机（右键可管理）" : "浏览器仅供界面预览"}
      </p>

      {menu && (
        <div
          className="conversation-context-menu"
          role="menu"
          aria-label="会话操作菜单"
          style={{ left: menu.x, top: menu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              const target = menu.conversation;
              setMenu(null);
              setRenameTarget(target);
              setRenameTitle(target.title);
              setRenameError(null);
              openDialog(renameDialog.current);
            }}
          >
            <Pencil size={14} aria-hidden="true" />
            修改标题
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              const target = menu.conversation;
              const nextArchived = !target.archived;
              setMenu(null);
              void studio
                .archiveConversation(target.id, nextArchived)
                .then(() => notify(nextArchived ? `已归档「${target.title}」` : `已取消归档「${target.title}」`))
                .catch(studio.reportError);
            }}
          >
            {menu.conversation.archived ? (
              <ArchiveRestore size={14} aria-hidden="true" />
            ) : (
              <Archive size={14} aria-hidden="true" />
            )}
            {menu.conversation.archived ? "取消归档" : "归档"}
          </button>
          <button
            type="button"
            role="menuitem"
            className="danger-item"
            onClick={() => {
              const target = menu.conversation;
              setMenu(null);
              setDeleteTarget(target);
              setDeleteError(null);
              openDialog(deleteDialog.current);
            }}
          >
            <Trash2 size={14} aria-hidden="true" />
            删除
          </button>
        </div>
      )}

      <AppDialog
        ref={renameDialog}
        className="form-dialog"
        aria-labelledby="sidebar-rename-title"
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (!renameTarget) return;
            setRenaming(true);
            setRenameError(null);
            void studio
              .renameConversationById(renameTarget.id, renameTitle)
              .then(() => {
                renameDialog.current?.close();
                notify("会话标题已更新");
              })
              .catch((value) => setRenameError(errorText(value)))
              .finally(() => setRenaming(false));
          }}
        >
          <h2 id="sidebar-rename-title">修改对话标题</h2>
          <div className="field">
            <label htmlFor="sidebar-conversation-title">标题</label>
            <input
              id="sidebar-conversation-title"
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
              {renaming ? "正在保存…" : "保存标题"}
            </button>
          </div>
        </form>
      </AppDialog>

      <AppDialog
        ref={deleteDialog}
        className="form-dialog"
        aria-labelledby="sidebar-delete-title"
      >
        <div>
          <h2 id="sidebar-delete-title">删除对话</h2>
          <p>
            确定要删除对话「<strong>{deleteTarget?.title}</strong>」吗？删除后将从当前工作区列表移除。
          </p>
          {deleteError && (
            <p className="form-error" role="alert">
              {deleteError}
            </p>
          )}
          <div className="dialog-actions">
            <button
              className="button secondary"
              type="button"
              disabled={deleting}
              onClick={() => deleteDialog.current?.close()}
            >
              取消
            </button>
            <button
              className="button danger"
              type="button"
              disabled={deleting}
              onClick={() => {
                if (!deleteTarget) return;
                setDeleting(true);
                setDeleteError(null);
                void studio
                  .deleteConversation(deleteTarget.id)
                  .then(() => {
                    deleteDialog.current?.close();
                    notify(`已删除对话「${deleteTarget.title}」`);
                  })
                  .catch((value) => setDeleteError(errorText(value)))
                  .finally(() => setDeleting(false));
              }}
            >
              {deleting ? "正在删除…" : "确认删除"}
            </button>
          </div>
        </div>
      </AppDialog>
    </>
  );
}
