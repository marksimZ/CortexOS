import { createClient, type Command, type WorkspaceArgs } from "./request";
import type { WebSearchResult } from "./web-search";

export type ReminderInput = {
  title: string;
  kind: "once" | "daily" | "weekdays" | "weekly";
  date: string;
  time: string;
  weekday: number;
  newsQuery?: string | null;
};

type EditableState = "active" | "paused" | "cancelled";
export type Reminder = ReminderInput & {
  id: string;
  state: EditableState | "completed";
  nextAt: string | null;
};
export type ReminderRun = {
  id: string;
  title: string;
  scheduledAt: string;
  status: "delivered" | "missed";
  brief?: {
    status: "pending" | "ready" | "empty" | "failed";
    query: string;
    results: WebSearchResult[];
    error: string | null;
  } | null;
};
export type ReminderList = { reminders: Reminder[]; runs: ReminderRun[] };

const request = createClient<{
  list_reminders: Command<WorkspaceArgs, ReminderList>;
  save_reminder: Command<
    WorkspaceArgs & { id: string | null; input: ReminderInput },
    void
  >;
  set_reminder_state: Command<
    WorkspaceArgs & { id: string; state: EditableState },
    void
  >;
}>();

export const listReminders = (workspaceId: string) =>
  request("list_reminders", { workspaceId });
export const saveReminder = (
  workspaceId: string,
  id: string | null,
  input: ReminderInput,
) => request("save_reminder", { workspaceId, id, input });
export const setReminderState = (
  workspaceId: string,
  id: string,
  state: EditableState,
) => request("set_reminder_state", { workspaceId, id, state });
