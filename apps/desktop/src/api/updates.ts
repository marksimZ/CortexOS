import type { Channel } from "@tauri-apps/api/core";
import { createClient, type Command } from "./request";

export type UpdateChannel = "stable" | "preview";
export type UpdateChannels = {
  version: string;
  channels: UpdateChannel[];
  canInstall: boolean;
};
export type UpdateInfo = { id: string; version: string; notes: string };
export type UpdateProgress = { bytes: number; total: number | null };

export const updates = createClient<{
  update_channels: Command<undefined, UpdateChannels>;
  check_update: Command<{ channel: UpdateChannel }, UpdateInfo | null>;
  download_update: Command<
    { id: string; progress: Channel<UpdateProgress> },
    void
  >;
  install_update: Command<{ id: string }, boolean>;
}>();
