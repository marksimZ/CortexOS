import { invoke } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";
import type { ModelSettings } from "./studio";

export const defaultSettings: ModelSettings = {
  model: "qwen3:4b-instruct",
  numCtx: 4096,
  temperature: 0.3,
  numPredict: 512,
  timeoutSeconds: 180,
};
export function useStudioSettings() {
  const [settings, setSettings] = useState(defaultSettings);
  const [settingsDraft, setSettingsDraft] = useState(defaultSettings);
  const applySettings = useCallback((saved: ModelSettings) => {
    setSettings(saved);
    setSettingsDraft(saved);
  }, []);
  const saveSettings = useCallback(async (next: ModelSettings) => {
    const saved = await invoke<ModelSettings>("save_settings", { settings: next });
    applySettings(saved);
  }, [applySettings]);
  return { settings, settingsDraft, setSettingsDraft, applySettings, saveSettings };
}
