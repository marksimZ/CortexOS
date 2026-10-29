import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { navigate, useNavigation } from "../navigation";
import { reportParams } from "./reportNavigation";
import { errorText, type Batch } from "./personalized";
import type { ReportScope } from "./ResultDrawer";

export function useBatchOverview(workspaceId: string, scope: ReportScope) {
  const route = useNavigation(), params = reportParams(route.hash);
  const id = params.get("batch");
  const context = route.page === "benchmark" && route.personalized === (scope === "personalized") && (!route.workspace || route.workspace === workspaceId);
  const invalid = id != null && (!id || id.length > 128 || /[\u0000-\u001f]/.test(id) || params.getAll("batch").length !== 1) ? "批次定位链接无效，请返回评测列表。" : "";
  const key = JSON.stringify([workspaceId, scope, id]), activeKey = useRef(key); activeKey.current = key;
  const [record, setRecord] = useState<{ key: string; batch: Batch | null; error: string }>();
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (!id || invalid || !context) return;
    let current = true, timer: number | undefined;
    const read = async () => {
      try {
        const batch = await invoke<Batch>("get_benchmark_overview", { workspaceId, batchId: id, scope });
        if (!current) return;
        if (batch.id !== id || (batch.scope && batch.scope !== scope)) throw new Error("读取记录与定位链接不一致。");
        setRecord({ key, batch, error: "" });
        if (["running", "stopping"].includes(batch.status)) timer = window.setTimeout(() => void read(), 1000);
      } catch (e) {
        if (current) setRecord(previous => ({ key, batch: previous?.key === key ? previous.batch : null, error: errorText(e) }));
      }
    };
    void read();
    return () => { current = false; if (timer !== undefined) window.clearTimeout(timer); };
  }, [workspaceId, scope, id, key, revision, invalid, context]);
  return {
    id, batch: !invalid && context && record?.key === key ? record.batch : null,
    loading: !!id && !invalid && context && record?.key !== key,
    error: invalid || (context && record?.key === key ? record.error : ""),
    refresh: () => setRevision(n => n + 1),
    update: (batch: Batch) => { if (activeKey.current === key && batch.id === id) { setRecord({ key, batch, error: "" }); setRevision(n => n + 1); } },
    open: (next: string, replace = false) => { if (next === id && context) setRevision(n => n + 1); else navigate("benchmark", { workspace: workspaceId, view: scope === "personalized" ? "personalized" : undefined, batch: next }, { replace }); },
  };
}
