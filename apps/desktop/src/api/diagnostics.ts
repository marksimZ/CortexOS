import { createClient, type Command } from "./request";

type Queue = { queued: number; running: number; cancelling: number };
export type DiagnosticsReport = {
  appVersion: string;
  capturedAt: string;
  backend: string;
  closing: boolean;
  ollama: string;
  docker: string;
  queue: Queue | null;
  storage: Queue | null;
  database: {
    schemaVersion: number;
    cloudConnections: { provider: string; status: string; count: number }[];
    activity: { area: string; status: string; count: number }[];
    problems: {
      area: string;
      category: string;
      nextAction: string;
      count: number;
      lastSeenUnix: number;
    }[];
  } | null;
  historyHours: number;
  resources: {
    systemCpuPercent: number | null;
    systemMemory: { usedBytes: number; totalBytes: number } | null;
    backendWorkingSetBytes: number | null;
    gpus:
      | {
          usedBytes: number | null;
          totalBytes: number | null;
          utilizationPercent: number | null;
        }[]
      | null;
  };
};

export const diagnostics = createClient<{
  read_diagnostics: Command<undefined, DiagnosticsReport>;
  export_diagnostics: Command<undefined, boolean>;
}>();
