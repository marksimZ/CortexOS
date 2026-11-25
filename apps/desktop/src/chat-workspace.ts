import { chat } from "./api/chat";
import { useCallback, useRef, useState } from "react";
import { preference, remember } from "./preferences";
import type { Conversation, Message } from "./studio";

export function useChatWorkspace() {
  const [workspaceId, setWorkspaceId] = useState<string | null>(null);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [messages, setMessages] = useState<Message[]>([]);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [creatingConversation, setCreatingConversation] = useState(false);
  const view = useRef({ workspaceId, conversationId });
  const viewVersion = useRef(0);
  const workspaceRequest = useRef(0);
  const messageRequest = useRef(0);

  const refreshMessages = useCallback(
    async (workspace: string, conversation: string) => {
      if (view.current.workspaceId !== workspace || view.current.conversationId !== conversation) return;
      const version = viewVersion.current;
      const request = ++messageRequest.current;
      const messages = await chat("load_messages", {
        workspaceId: workspace,
        conversationId: conversation,
      });
      if (
        version === viewVersion.current &&
        request === messageRequest.current &&
        view.current.workspaceId === workspace &&
        view.current.conversationId === conversation
      ) {
        setMessages(messages);
      }
    },
    [],
  );

  const refreshConversations = useCallback(async (workspace: string) => {
    const list = await chat("list_conversations", {
      workspaceId: workspace,
    });
    if (view.current.workspaceId === workspace) setConversations(list);
    return list;
  }, []);

  const openConversation = useCallback(
    async (workspace: string, conversation: string) => {
      const version = ++viewVersion.current;
      const request = ++messageRequest.current;
      view.current = { workspaceId: workspace, conversationId: conversation };
      setConversationId(conversation);
      setMessages([]);
      setLoadingMessages(true);
      remember(`conversation.${workspace}`, conversation);
      try {
        const messages = await chat("load_messages", {
          workspaceId: workspace,
          conversationId: conversation,
        });
        if (version === viewVersion.current && request === messageRequest.current) setMessages(messages);
      } finally {
        if (version === viewVersion.current) setLoadingMessages(false);
      }
    },
    [],
  );

  const openWorkspace = useCallback(
    async (workspace: string, targetConversation?: string | null, signal?: AbortSignal) => {
      const request = ++workspaceRequest.current;
      const version = ++viewVersion.current;
      view.current = { workspaceId: workspace, conversationId: null };
      setWorkspaceId(workspace);
      setConversationId(null);
      setConversations([]);
      setMessages([]);
      remember("workspace", workspace);
      setLoadingMessages(true);
      try {
        const list = await chat("list_conversations", { workspaceId: workspace });
        if (request !== workspaceRequest.current || view.current.workspaceId !== workspace) return;
        setConversations(list);
        if (version !== viewVersion.current) return;
        if (targetConversation && !signal?.aborted) {
          await openConversation(workspace, targetConversation);
          return;
        }
        const saved = preference(`conversation.${workspace}`);
        const activeList = list.filter((item) => !item.archived);
        const first =
          list.find((item) => item.id === saved) ?? activeList[0] ?? list[0];
        if (first) await openConversation(workspace, first.id);
      } finally {
        if (version === viewVersion.current) setLoadingMessages(false);
      }
    },
    [openConversation],
  );

  const newConversation = async () => {
    const workspace = view.current.workspaceId;
    if (!workspace) throw "请先创建工作区。";
    setCreatingConversation(true);
    try {
      const conversation = await chat("create_conversation", {
        workspaceId: workspace,
      });
      await refreshConversations(workspace);
      if (view.current.workspaceId === workspace)
        await openConversation(workspace, conversation.id);
      return conversation;
    } finally {
      setCreatingConversation(false);
    }
  };

  const renameConversationById = async (conversationId: string, title: string) => {
    const workspace = view.current.workspaceId;
    if (!workspace) return;
    await chat("rename_conversation", {
      workspaceId: workspace,
      conversationId,
      title,
    });
    await refreshConversations(workspace);
  };

  const renameConversation = async (title: string) => {
    const { conversationId: conversation } = view.current;
    if (!conversation) return;
    await renameConversationById(conversation, title);
  };

  const archiveConversation = async (conversationId: string, archived: boolean) => {
    const workspace = view.current.workspaceId;
    if (!workspace) return;
    await chat("archive_conversation", {
      workspaceId: workspace,
      conversationId,
      archived,
    });
    const list = await refreshConversations(workspace);
    if (archived && view.current.conversationId === conversationId) {
      const next = list.find((item) => !item.archived && item.id !== conversationId);
      if (next) {
        await openConversation(workspace, next.id);
      }
    }
  };

  const deleteConversation = async (conversationId: string) => {
    const workspace = view.current.workspaceId;
    if (!workspace) return;
    await chat("delete_conversation", {
      workspaceId: workspace,
      conversationId,
    });
    const list = await refreshConversations(workspace);
    if (view.current.conversationId === conversationId) {
      const next = list.find((item) => !item.archived) ?? list[0];
      if (next) {
        await openConversation(workspace, next.id);
      } else {
        view.current = { workspaceId: workspace, conversationId: null };
        setConversationId(null);
        setMessages([]);
      }
    }
  };

  return {
    workspaceId, conversationId, conversations, messages, loadingMessages, creatingConversation,
    view, openWorkspace, openConversation, refreshMessages, refreshConversations, newConversation,
    renameConversation, renameConversationById, archiveConversation, deleteConversation,
  };
}
