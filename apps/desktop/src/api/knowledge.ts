import type { GraphData, GraphEdge, GraphNode, Point } from "../graph-data";
import type { GraphNodePage, HistoryPage, RelationPage } from "../graph-query";
import type { KnowledgeDetail, KnowledgeNode } from "../knowledge-types";
import type {
  Preview,
  Snapshot,
  SourceDocument,
  SourceRef,
} from "../SourceBrowser";
import type { RunSummary } from "../studio";
import { createClient, type Command, type WorkspaceArgs } from "./request";

type NodeArgs = WorkspaceArgs & { nodeId: string };
type SourceArgs = WorkspaceArgs & { sourceId: string };
type TicketArgs = WorkspaceArgs & { ticket: string };
export type FolderSyncReport = {
  folder: string; imported: number; updated: number; unchanged: number;
  skipped: number; remaining: number; error: string | null;
};
export function formatFolderSync(report: Pick<FolderSyncReport, "imported" | "updated" | "unchanged" | "skipped" | "remaining">) {
  return `新增 ${report.imported}、更新 ${report.updated}、未变 ${report.unchanged}` +
    (report.skipped ? `、跳过 ${report.skipped}` : "") +
    (report.remaining ? `，剩余 ${report.remaining} 个文件将在下次同步时继续` : "");
}
type SaveKnowledge = {
  input: WorkspaceArgs & {
    nodeId: string | null;
    expectedRevision: number | null;
    title: string;
    kind: string;
    content: string;
    changeReason: string;
    sourceMessageId: string | null;
    sourceExcerpt: string | null;
  };
  validity?: { validFrom: string | null; validTo: string | null };
};
type ViewArgs = { expectedRevision: number; camera: Point; target: Point };
type PageArgs = {
  offset: number;
  limit: number;
  expectedSnapshot: string | null;
};

export const knowledge = createClient<{
  list_knowledge: Command<WorkspaceArgs, KnowledgeNode[]>;
  get_knowledge: Command<NodeArgs, KnowledgeDetail>;
  save_knowledge: Command<SaveKnowledge, string>;
  save_source_knowledge: Command<SaveKnowledge & { source: SourceRef }, string>;
  approve_knowledge: Command<NodeArgs & { expectedRevision: number }, void>;
  set_knowledge_local_only: Command<NodeArgs & { localOnly: boolean }, void>;
  list_sources: Command<WorkspaceArgs, SourceDocument[]>;
  get_source: Command<SourceArgs & { version: number }, Snapshot>;
  choose_source: Command<WorkspaceArgs, Preview | null>;
  import_source_folder: Command<
    WorkspaceArgs,
    FolderSyncReport | null
  >;
  list_watched_folders: Command<
    WorkspaceArgs,
    { id: string; workspaceId: string; path: string; lastSyncedAt: string | null; createdAt: string }[]
  >;
  sync_watched_folder: Command<
    WorkspaceArgs & { folderId: string },
    FolderSyncReport
  >;
  remove_watched_folder: Command<WorkspaceArgs & { folderId: string }, void>;
  sync_workspace_watched_folders: Command<
    WorkspaceArgs,
    FolderSyncReport[]
  >;
  commit_source: Command<TicketArgs, SourceDocument>;
  discard_source: Command<TicketArgs, void>;
  revoke_source: Command<SourceArgs & { expectedRevision: number }, void>;
  query_graph_nodes: Command<
    {
      input: WorkspaceArgs &
        PageArgs & {
          query: string;
          kind: string;
          status: string;
          sinceUtc: string | null;
          rootNodeId: string | null;
          rootNodeIds?: string[];
        };
    },
    GraphNodePage
  >;
  load_graph_node: Command<NodeArgs, GraphNode>;
  load_graph_links: Command<
    WorkspaceArgs & { nodeIds: string[]; expectedSnapshot: string },
    GraphEdge[]
  >;
  query_graph_relations: Command<NodeArgs & PageArgs, RelationPage>;
  load_graph_edge_history: Command<
    WorkspaceArgs & {
      edgeId: string;
      beforeVersion: number | null;
      limit: number;
    },
    HistoryPage
  >;
  save_graph_view: Command<
    {
      input: WorkspaceArgs &
        ViewArgs & { positions: GraphData["view"]["positions"] };
    },
    number
  >;
  save_knowledge_edge: Command<
    {
      input: WorkspaceArgs & {
        edgeId: string | null;
        expectedVersion: number | null;
        fromNodeId: string;
        toNodeId: string;
        relation: string;
        active: boolean;
        changeReason: string;
      };
    },
    string
  >;
  list_runs: Command<WorkspaceArgs & { offset?: number }, RunSummary[]>;
}>();
