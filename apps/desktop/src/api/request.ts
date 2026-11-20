import { invoke } from "@tauri-apps/api/core";

export type Command<Args, Result> = { args: Args; result: Result };
type Contract = Record<
  string,
  Command<Record<string, unknown> | undefined, unknown>
>;

export function createClient<Commands extends Contract>() {
  return async function request<Name extends keyof Commands & string>(
    name: Name,
    ...[args, signal]: Commands[Name]["args"] extends undefined
      ? [args?: undefined, signal?: AbortSignal]
      : [args: Commands[Name]["args"], signal?: AbortSignal]
  ): Promise<Commands[Name]["result"]> {
    // A view can stop consuming a reply. Cancelling backend work uses its own Rust command.
    signal?.throwIfAborted();
    try {
      const result = await invoke<Commands[Name]["result"]>(name, args);
      signal?.throwIfAborted();
      return result;
    } catch (error) {
      signal?.throwIfAborted();
      throw error;
    }
  };
}

export type WorkspaceArgs = { workspaceId: string };
