import { useCallback, useEffect, useState } from "react";
import { preference, remember } from "./preferences";

export interface ModelUsagePreference {
  local: boolean;
  cloud: boolean;
}

const PREF_KEY = "model-usage-preference";
const PROMPTED_KEY = "model-usage-prompted";
export const MODEL_USAGE_CHANGED_EVENT = "cortexos-model-usage-changed";
export const CLOUD_CONNECTIONS_CHANGED_EVENT =
  "cortexos-cloud-connections-changed";

const DEFAULT_USAGE: ModelUsagePreference = { local: true, cloud: true };

export function normalizeModelUsagePreference(
  next: Partial<ModelUsagePreference> | null | undefined,
): ModelUsagePreference {
  const local = typeof next?.local === "boolean" ? next.local : true;
  const cloud = typeof next?.cloud === "boolean" ? next.cloud : true;
  if (!local && !cloud) {
    return { ...DEFAULT_USAGE };
  }
  return { local, cloud };
}

export function readModelUsagePreference(): ModelUsagePreference {
  const raw = preference(PREF_KEY);
  if (!raw) return { ...DEFAULT_USAGE };
  try {
    const parsed = JSON.parse(raw) as Partial<ModelUsagePreference>;
    return normalizeModelUsagePreference(parsed);
  } catch {
    return { ...DEFAULT_USAGE };
  }
}

export function saveModelUsagePreference(
  next: ModelUsagePreference,
): ModelUsagePreference {
  const normalized = normalizeModelUsagePreference(next);
  remember(PREF_KEY, JSON.stringify(normalized));
  if (typeof window !== "undefined") {
    window.dispatchEvent(
      new CustomEvent<ModelUsagePreference>(MODEL_USAGE_CHANGED_EVENT, {
        detail: normalized,
      }),
    );
  }
  return normalized;
}

export function isModelUsagePrompted(): boolean {
  return preference(PROMPTED_KEY) === "true";
}

export function markModelUsagePrompted(): void {
  remember(PROMPTED_KEY, "true");
}

export function emitCloudConnectionsChanged(workspaceId?: string | null): void {
  if (typeof window !== "undefined") {
    window.dispatchEvent(
      new CustomEvent(CLOUD_CONNECTIONS_CHANGED_EVENT, {
        detail: { workspaceId: workspaceId ?? null },
      }),
    );
  }
}

export function useModelUsagePreference(): [
  ModelUsagePreference,
  (next: ModelUsagePreference) => ModelUsagePreference,
] {
  const [usage, setUsage] = useState<ModelUsagePreference>(() =>
    readModelUsagePreference(),
  );

  useEffect(() => {
    const sync = () => setUsage(readModelUsagePreference());
    const onCustom = (event: Event) => {
      const detail = (event as CustomEvent<ModelUsagePreference>).detail;
      setUsage(detail ? normalizeModelUsagePreference(detail) : readModelUsagePreference());
    };
    window.addEventListener(MODEL_USAGE_CHANGED_EVENT, onCustom);
    window.addEventListener("storage", sync);
    return () => {
      window.removeEventListener(MODEL_USAGE_CHANGED_EVENT, onCustom);
      window.removeEventListener("storage", sync);
    };
  }, []);

  const update = useCallback((next: ModelUsagePreference) => {
    const saved = saveModelUsagePreference(next);
    setUsage(saved);
    return saved;
  }, []);

  return [usage, update];
}
