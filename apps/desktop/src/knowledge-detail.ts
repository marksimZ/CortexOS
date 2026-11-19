import { invoke } from "@tauri-apps/api/core";
import type { KnowledgeDetail } from "./Knowledge";

export const getKnowledge = (workspaceId: string, nodeId: string) =>
  invoke<KnowledgeDetail>("get_knowledge", { workspaceId, nodeId });
