import { createClient, type Command, type WorkspaceArgs } from "./request";

export interface GlobalPersona {
  assistantName: string;
  displayName: string;
  roleBackground: string;
  communicationStyle: string;
  preferences: string[];
  rules: string[];
  updatedAt: string;
}

export function assistantLabel(name?: string | null, fallback = "专属助手"): string {
  const clean = name?.trim();
  return clean || fallback;
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
  replacesNodeId?: string | null;
  replacesTitle?: string | null;
  replacesValue?: string | null;
  conflictReason?: string | null;
}

export interface FocusEpisode {
  conversationId: string;
  title: string;
  summary: string;
  updatedAt: string;
}

export interface FocusSourceUpdate {
  sourceId: string;
  nodeId: string | null;
  name: string;
  version: number;
  updatedAt: string;
}

export interface FocusBrief {
  title: string;
  query: string;
  scheduledAt: string;
  headlines: { title: string; url: string }[];
}

export interface FocusReminder {
  id: string;
  title: string;
  nextDate: string;
  time: string;
  isBriefing: boolean;
}

export interface DailyFocus {
  workspaceId: string;
  workspaceName: string;
  assistantName?: string;
  activeAssistantName?: string | null;
  displayName: string;
  hasActivity: boolean;
  knowledgeCount: number;
  pendingMemoryCount: number;
  recentEpisodes: FocusEpisode[];
  recentSources: FocusSourceUpdate[];
  latestBrief: FocusBrief | null;
  upcomingReminders: FocusReminder[];
  suggestedPrompts: string[];
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
  read_daily_focus: Command<WorkspaceArgs, DailyFocus>;
}>();
