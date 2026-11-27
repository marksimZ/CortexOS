import { chat } from "./api/chat";
import { useCallback, useRef, useState, type SetStateAction } from "react";
import type { ModelSettings } from "./studio";
import { rebaseSettingsDraft } from "./settings-draft";

export const defaultSettings: ModelSettings = {
  model: "qwen3:4b-instruct",
  numCtx: 4096,
  temperature: 0.3,
  topP: 0.9,
  numPredict: 0,
  timeoutSeconds: 180,
  deepThinking: false,
};
export function useStudioSettings() {
  const [state, setState] = useState({ saved: defaultSettings, draft: defaultSettings });
  const writes = useRef(0);
  const settingsRevision = useCallback(() => writes.current, []);
  const setSettingsDraft = useCallback((change: SetStateAction<ModelSettings>) => {
    setState(current => ({
      ...current,
      draft: typeof change === "function" ? change(current.draft) : change,
    }));
  }, []);
  const applySettings = useCallback((saved: ModelSettings, revision: number) => {
    if (revision !== writes.current) return;
    setState(current => ({ saved, draft: rebaseSettingsDraft(current.draft, current.saved, saved) }));
  }, []);
  const saveSettings = useCallback(async (next: ModelSettings) => {
    const write = ++writes.current;
    const saved = await chat("save_settings", { settings: next });
    if (write !== writes.current) return;
    writes.current++;
    setState(current => ({
      saved,
      draft: rebaseSettingsDraft(current.draft, next, saved, current.saved),
    }));
  }, []);
  return { settings: state.saved, settingsDraft: state.draft, setSettingsDraft, settingsRevision, applySettings, saveSettings };
}
