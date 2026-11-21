import { createClient, type Command, type WorkspaceArgs } from "./request";

type Workspace = { id: string; name: string };
export type PortableUi = Record<string, string>;
export type BackupManifest = {
  formatVersion: number;
  backupId: string;
  appVersion: string;
  schemaVersion: number;
  createdAt: string;
  workspaceId: string | null;
  workspaces: Workspace[];
  files: { name: string; bytes: number; sha256: string }[];
  dependencies: string[];
};
export type BackupPreview = {
  workspaces: Workspace[];
  estimatedDatabaseBytes: number;
  availableBytes: number | null;
  includes: string[];
  dependencies: string[];
};
export type OpenBackup = {
  manifest: BackupManifest;
  path: string;
  conflicts: Workspace[];
  availableBytes: number | null;
  pending: boolean;
};
export type RestoreStatus = {
  phase: "idle" | "failed" | "pending" | "apply_ui";
  message: string | null;
  id: string | null;
  scope: string | null;
  ui: PortableUi | null;
};
export type BackupHistory = {
  id: string;
  kind: "backup" | "restore" | "restore_failed";
  at: string;
  path: string;
  safetyPath: string | null;
  workspaces: Workspace[];
  detail: string | null;
};

export const portability = createClient<{
  backup_preview: Command<{ workspaceId: string | null }, BackupPreview>;
  backup_save: Command<
    { workspaceId: string | null; ui: PortableUi },
    BackupManifest | null
  >;
  backup_open: Command<undefined, OpenBackup | null>;
  restore_stage: Command<
    { backupId: string; confirmed: boolean; previousUi: PortableUi },
    void
  >;
  restore_cancel: Command<undefined, void>;
  restore_on_start: Command<undefined, RestoreStatus>;
  restore_ack: Command<{ id: string }, void>;
  backup_history: Command<undefined, BackupHistory[]>;
  export_knowledge: Command<
    WorkspaceArgs & { format: "json" | "markdown" },
    boolean
  >;
}>();
