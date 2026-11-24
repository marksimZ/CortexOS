import type { SourceEvidence, SourceRef } from "./SourceBrowser";
import type { Message, VersionLineage } from "./studio";

export const kinds: Record<string, string> = {
  knowledge: "知识",
  context: "背景",
  hypothesis: "假设",
  decision: "决定",
  evidence: "证据",
  conflict: "争议",
};
export interface KnowledgeNode {
  id: string;
  workspaceId: string;
  title: string;
  kind: string;
  latestVersion: number;
  effectiveVersion: number | null;
  revision: number;
  updatedAt: string;
  sourceMessageId: string | null;
  localOnly?: boolean;
}
interface Version {
  version: number;
  title: string;
  kind: string;
  content: string;
  changeReason: string;
  createdAt: string;
  approvedAt: string | null;
  validity?: { validFrom: string | null; validTo: string | null };
  lineage?: VersionLineage;
}
export interface KnowledgeDetail {
  node: KnowledgeNode;
  versions: Version[];
  sourceConversationId: string | null;
  sourceExcerpt: string | null;
  sourceHash: string | null;
  sourceEvidence?: SourceEvidence[];
}
export interface EditKnowledge {
  returnTo?: "graph";
  workspaceId: string;
  detail?: KnowledgeDetail;
  source?: Message;
  excerpt?: string;
  sourceDocument?: SourceRef;
  sourceName?: string;
}
