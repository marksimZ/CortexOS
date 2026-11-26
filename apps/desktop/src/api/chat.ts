import type { Channel } from "@tauri-apps/api/core";
import type { StreamUpdate } from "../chat-stream";
import type {
  ChatRuntimeRoute,
  CloudConsent,
  CloudKnowledgeGrant,
  GenerationTarget,
  RuntimeConnection,
} from "../runtime-profiles";
import type {
  Bootstrap,
  ContextManifest,
  ContextSelection,
  Conversation,
  KnowledgeScope,
  Message,
  MessageImage,
  ModelHealth,
  ModelSettings,
  RunDetail,
  RunSummary,
  Workspace,
} from "../studio";
import { createClient, type Command, type WorkspaceArgs } from "./request";

type ConversationArgs = WorkspaceArgs & { conversationId: string };
type RunArgs = WorkspaceArgs & { runId: string };
type ContextArgs = WorkspaceArgs & {
  content: string;
  contextSelection: ContextSelection[];
  knowledgeScope: KnowledgeScope | null;
  useGraph: boolean;
};
export type ContextCursor = { id: string; updatedAt: string };
export type ContextOption = ContextCursor & {
  localOnly?: boolean;
  title: string;
  latestVersion: number;
  effectiveVersion: number;
};
type ContextOptionsPage = { items: ContextOption[]; hasMore: boolean };
type RunPreparing = Pick<RunSummary, "id" | "workspaceId" | "conversationId">;
export type ChatSearchHit = {
  conversationId: string;
  conversationTitle: string;
  archived: boolean;
  messageId: string;
  role: "user" | "assistant";
  snippet: string;
  createdAt: string;
};

export const chat = createClient<{
  bootstrap: Command<undefined, Bootstrap>;
  check_ollama: Command<undefined, ModelHealth>;
  create_workspace: Command<
    { name: string; template?: string | null },
    Workspace
  >;
  list_conversations: Command<WorkspaceArgs, Conversation[]>;
  create_conversation: Command<WorkspaceArgs, Conversation>;
  rename_conversation: Command<ConversationArgs & { title: string }, void>;
  archive_conversation: Command<ConversationArgs & { archived: boolean }, void>;
  delete_conversation: Command<ConversationArgs, void>;
  search_chat_messages: Command<
    WorkspaceArgs & { query: string; limit?: number },
    ChatSearchHit[]
  >;
  load_messages: Command<ConversationArgs, Message[]>;
  save_settings: Command<{ settings: ModelSettings }, ModelSettings>;
  set_conversation_retrieval: Command<
    ConversationArgs & { enabled: boolean },
    void
  >;
  get_chat_run_status: Command<RunArgs, string>;
  cancel_run: Command<RunArgs, void>;
  get_run: Command<RunArgs, RunDetail>;
  start_chat: Command<
    ContextArgs &
      ConversationArgs & {
        images?: MessageImage[];
        cloudConsent?: CloudConsent;
        useKnowledgeRoute?: boolean;
        generationTarget?: GenerationTarget;
        onPreparing: Channel<RunPreparing>;
        onUpdate: Channel<StreamUpdate>;
      },
    RunSummary
  >;
  list_context_options: Command<
    WorkspaceArgs & { day: string; after: ContextCursor | null },
    ContextOptionsPage
  >;
  preview_context: Command<
    ContextArgs & {
      conversationId: string | null;
      images?: MessageImage[];
      generationTarget?: GenerationTarget;
    },
    ContextManifest
  >;
  chat_runtime_route: Command<
    WorkspaceArgs & { knowledge: boolean; generationTarget?: GenerationTarget },
    ChatRuntimeRoute
  >;
  list_runtime_cloud_connections: Command<WorkspaceArgs, RuntimeConnection[]>;
  list_model_cloud_knowledge: Command<
    WorkspaceArgs & { connectionId: string; includeCandidates?: boolean },
    { grants: CloudKnowledgeGrant[] }
  >;
}>();
