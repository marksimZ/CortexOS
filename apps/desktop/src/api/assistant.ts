import { createClient, type Command, type WorkspaceArgs } from "./request";

export type AssistantDraft = { name: string; instructions: string };

export type AssistantSettings = {
  draft: AssistantDraft;
  activeVersion: number | null;
  versions: (AssistantDraft & { version: number })[];
};

export const assistant = createClient<{
  read_assistant: Command<WorkspaceArgs, AssistantSettings>;
  save_assistant_draft: Command<WorkspaceArgs & AssistantDraft, void>;
  publish_assistant: Command<WorkspaceArgs & AssistantDraft, void>;
  activate_assistant: Command<WorkspaceArgs & { version: number | null }, void>;
}>();
