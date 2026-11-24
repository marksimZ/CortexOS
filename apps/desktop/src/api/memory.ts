import { createClient, type Command, type WorkspaceArgs } from "./request";

export interface GlobalPersona {
  displayName: string;
  roleBackground: string;
  communicationStyle: string;
  preferences: string[];
  rules: string[];
  updatedAt: string;
}

export interface MemoryCandidate {
  id: string;
  workspaceId: string;
  conversationId: string;
  sourceMessageId: string | null;
  scope: "global_preference" | "global_rule" | "workspace_fact";
  title: string;
  content: string;
  status: "pending" | "accepted" | "dismissed";
  createdAt: string;
}

export const memoryApi = createClient<{
  get_global_persona: Command<undefined, GlobalPersona>;
  save_global_persona: Command<{ input: GlobalPersona }, GlobalPersona>;
  list_memory_candidates: Command<
    WorkspaceArgs & { conversationId?: string | null },
    MemoryCandidate[]
  >;
  resolve_memory_candidate: Command<
    WorkspaceArgs & {
      candidateId: string;
      action: "accept" | "dismiss";
      editedContent?: string | null;
    },
    void
  >;
  promote_rule_from_chat: Command<{ rule: string }, GlobalPersona>;
  generate_weekly_digest_draft: Command<
    WorkspaceArgs & { days?: number },
    {
      workspaceId: string;
      periodDays: number;
      episodeCount: number;
      knowledgeCount: number;
      markdown: string;
    }
  >;
}>();
