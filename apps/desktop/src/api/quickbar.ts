import { createClient, type Command, type WorkspaceArgs } from "./request";

export interface QuickbarStatus {
  enabled: boolean;
  registered: boolean;
  shortcut: string;
  error: string | null;
}

export const quickbarApi = createClient<{
  quickbar_status: Command<undefined, QuickbarStatus>;
  set_quickbar_enabled: Command<{ enabled: boolean }, QuickbarStatus>;
  toggle_quickbar: Command<undefined, boolean>;
  hide_quickbar: Command<undefined, void>;
  resize_quickbar: Command<{ height: number }, void>;
  quickbar_save_note: Command<WorkspaceArgs & { content: string }, string>;
  open_in_main_window: Command<
    WorkspaceArgs & { conversationId?: string | null; nodeId?: string | null },
    void
  >;
}>();
