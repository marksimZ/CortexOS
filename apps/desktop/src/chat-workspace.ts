import { invoke } from "@tauri-apps/api/core";
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
  const messageRequest = useRef(0);

  const refreshMessages = useCallback(
    async (workspace: string, conversation: string) => {
      if (view.current.workspaceId !== workspace || view.current.conversationId !== conversation) return;
      const version = viewVersion.current;
      const request = ++messageRequest.current;
      const messages = await invoke<Message[]>("load_messages", {
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
    const list = await invoke<Conversation[]>("list_conversations", {
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
        const messages = await invoke<Message[]>("load_messages", {
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
    async (workspace: string) => {
      const version = ++viewVersion.current;
      view.current = { workspaceId: workspace, conversationId: null };
      setWorkspaceId(workspace);
      setConversationId(null);
      setConversations([]);
      setMessages([]);
      remember("workspace", workspace);
      setLoadingMessages(true);
      try {
        const list = await invoke<Conversation[]>("list_conversations", { workspaceId: workspace });
        if (version !== viewVersion.current) return;
        setConversations(list);
        const saved = preference(`conversation.${workspace}`);
        const first = list.find((item) => item.id === saved) ?? list[0];
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
      const conversation = await invoke<Conversation>("create_conversation", {
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

  const renameConversation = async (title: string) => {
    const { workspaceId: workspace, conversationId: conversation } =
      view.current;
    if (!workspace || !conversation) return;
    await invoke("rename_conversation", {
      workspaceId: workspace,
      conversationId: conversation,
      title,
    });
    await refreshConversations(workspace);
  };

  return {
    workspaceId, conversationId, conversations, messages, loadingMessages, creatingConversation,
    view, openWorkspace, openConversation, refreshMessages, refreshConversations, newConversation, renameConversation,
  };
}
