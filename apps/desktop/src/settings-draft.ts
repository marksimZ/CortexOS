import type { ModelSettings } from "./studio";

export function rebaseSettingsDraft(
  draft: ModelSettings,
  previous: ModelSettings,
  saved: ModelSettings,
): ModelSettings {
  const field = <Key extends keyof ModelSettings>(key: Key) =>
    Object.is(draft[key], previous[key]) ? saved[key] : draft[key];

  return {
    model: field("model"),
    numCtx: field("numCtx"),
    temperature: field("temperature"),
    numPredict: field("numPredict"),
    timeoutSeconds: field("timeoutSeconds"),
  };
}
