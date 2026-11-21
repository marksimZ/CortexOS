import { createClient, type Command, type WorkspaceArgs } from "./request";

export type TaskKind =
  | "chat"
  | "benchmark"
  | "harness_validation"
  | "harness_workspace"
  | "harness_session"
  | "harness_experiment"
  | "rag_index"
  | "local_maintenance";

export type LiveTask = WorkspaceArgs & {
  id: string;
  kind: TaskKind;
  status: "queued" | "running" | "cancelling";
};

export type TaskRecord = {
  id: string;
  kind: string;
  status: string;
  at: string;
  error: string | null;
  contextLabel: string | null;
  conversation: string | null;
  message: string | null;
  assessment: number | null;
  processed: number | null;
  total: number | null;
};

export const taskCenter = createClient<{
  read_live_tasks: Command<{ workspaceId: string | null }, LiveTask[]>;
  read_task_history: Command<
    WorkspaceArgs & { activeId: string | null },
    TaskRecord[]
  >;
  cancel_waiting_task: Command<
    WorkspaceArgs & { taskId: string; kind: TaskKind },
    void
  >;
  stop_standard_benchmark: Command<WorkspaceArgs & { batchId: string }, void>;
  stop_personalized_task: Command<WorkspaceArgs & { taskId: string }, void>;
  stop_harness_experiment: Command<
    WorkspaceArgs & { experimentId: string },
    void
  >;
  stop_harness_programming_validation: Command<
    WorkspaceArgs & { validationId: string },
    void
  >;
  rag_cancel_index: Command<WorkspaceArgs & { requestId: string }, void>;
  cancel_rag_runtime_experiment: Command<WorkspaceArgs & { id: string }, void>;
}>();
