import type { ModelSettings } from "./studio";

export function rebaseSettingsDraft(
  draft: ModelSettings,
  previous: ModelSettings,
  saved: ModelSettings,
  previousSaved?: ModelSettings,
): ModelSettings {
  const field = <Key extends keyof ModelSettings>(key: Key) =>
    Object.is(draft[key], previous[key]) ||
    (previousSaved !== undefined && Object.is(draft[key], previousSaved[key]))
      ? saved[key]
      : draft[key];

  return {
    model: field("model"),
    numCtx: field("numCtx"),
    temperature: field("temperature"),
    topP: field("topP"),
    numPredict: field("numPredict"),
    timeoutSeconds: field("timeoutSeconds"),
    deepThinking: field("deepThinking"),
  };
}
