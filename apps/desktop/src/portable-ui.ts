import { isTauri } from "@tauri-apps/api/core";
import { portability, type PortableUi } from "./api/portability";
import { snapshotDrafts } from "./draft-store";

const globalKeys = new Set(
  ["appearance", "workspace", "evidence-open", "graph-motion"].map(
    (key) => "cortexos." + key,
  ),
);

function portableKey(key: string, workspaceId: string | null) {
  if (globalKeys.has(key)) return workspaceId === null;
  if (key.startsWith("cortexos.conversation.")) {
    return (
      workspaceId === null || key === "cortexos.conversation." + workspaceId
    );
  }
  const match = /^cortexos\.draft\.v1\.([^.]+)\.(.+)$/.exec(key);
  if (!match) return false;
  try {
    const decoded = decodeURIComponent(match[2]);
    let owner: unknown;
    switch (match[1]) {
      case "knowledge":
      case "relation":
        owner = JSON.parse(decoded)[0];
        break;
      case "assistant":
      case "harness-analysis":
      case "standard-benchmark":
        owner = decoded;
        break;
      case "chat":
      case "dataset-review":
      case "benchmark-profile":
      case "programming":
      case "task-review":
      case "workspace-handoff":
        owner = decoded.includes(":") ? decoded.split(":", 1)[0] : null;
        break;
      default:
        return false;
    }
    return (
      typeof owner === "string" &&
      !!owner &&
      (workspaceId === null || workspaceId === owner)
    );
  } catch {
    return false;
  }
}

export function capturePortableUi(workspaceId: string | null): PortableUi {
  return snapshotDrafts((key) => portableKey(key, workspaceId));
}

let startupMessage = "";
export const portabilityStartupMessage = () => startupMessage;

export async function restorePortableUi() {
  if (!isTauri()) return;
  let errorDuringApply = false;
  try {
    const state = await portability("restore_on_start");
    if (state.phase !== "apply_ui" || !state.ui || !state.id) return;
    const saved = capturePortableUi(state.scope);
    const changed = Object.fromEntries(
      Object.entries(state.ui).filter(([key]) => portableKey(key, state.scope)),
    );
    try {
      errorDuringApply = true;
      for (const key of Object.keys(saved)) {
        if (!(key in changed)) localStorage.removeItem(key);
      }
      for (const [key, value] of Object.entries(changed))
        localStorage.setItem(key, value);
    } catch (error) {
      for (const key of Object.keys(changed)) {
        if (!(key in saved)) localStorage.removeItem(key);
      }
      for (const [key, value] of Object.entries(saved))
        localStorage.setItem(key, value);
      throw error;
    }
    errorDuringApply = false;
    await portability("restore_ack", { id: state.id });
    startupMessage = "数据、草稿和界面偏好已恢复。";
  } catch (error) {
    startupMessage = errorDuringApply
      ? "数据库已恢复，但界面未能保存草稿和偏好。请保留安全备份，重新打开应用后可重试。"
      : typeof error === "string"
        ? error
        : "未能读取数据恢复状态，请在设置中查看。";
  }
}
