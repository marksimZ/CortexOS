import { benchmark, type ResultLocation } from "../api/benchmark";
import { useEffect, useMemo, useState } from "react";
import { navigate, navigationHash, navigationParams, parseNavigation, useNavigation } from "../navigation";
import { errorText, ruleName, type Batch } from "./personalized";
import type { ReportOrder, ResultFilter } from "./ReportOptions";
import type { ReportScope } from "./ResultDrawer";

const states = ["Passed", "Failed", "Unsupported", "Not Run", "Execution Error", "Scoring Error", "Needs Review", "In Progress", "Cancelled"];
export const reportParams = navigationParams;
function matchesReport(hash: string, workspaceId: string, batchId: string, scope: ReportScope) {
  const route = parseNavigation(hash);
  return route.page === "benchmark" && route.benchmarkView === scope &&
    (!route.workspace || route.workspace === workspaceId) && route.params.get("batch") === batchId;
}
const integer = (value: string | null, max = 0xffffffff) => value != null && /^(0|[1-9]\d*)$/.test(value) && Number.isSafeInteger(Number(value)) && Number(value) <= max ? Number(value) : undefined;
export function reportLink(value: string) {
  if (!value.startsWith("#/benchmark?") || value.length > 8192) throw new Error("请粘贴 CortexOS 的评测定位链接（#/benchmark?...）。");
  const p = reportParams(value);
  for (const key of p.keys()) if (p.getAll(key).length !== 1) throw new Error("定位链接含重复参数，请重新复制。");
  if (!p.get("workspace") || !p.get("batch") || (p.has("view") && p.get("view") !== "personalized")) throw new Error("定位链接缺少工作区、批次或评测类型不正确。");
  p.delete("back");
  return navigationHash("benchmark", Object.fromEntries(p));
}
export function benchmarkReturn(back: string | null, workspace: string | null, run: string | null) {
  if (!back || !workspace || !run) return null;
  try {
    const p = reportParams(reportLink(back));
    if (p.get("workspace") !== workspace) return null;
    p.set("trace", run);
    return navigationHash("benchmark", Object.fromEntries(p));
  } catch { return null; }
}
function read(hash: string, batch: Batch) {
  const p = reportParams(hash);
  const result = { order: "plan" as ReportOrder, model: "all", filter: {} as ResultFilter, pages: {} as Record<number, number>, item: undefined as number | undefined, trace: undefined as string | undefined, error: "" };
  try {
    if (hash.length > 8192) throw new Error("定位链接过长，请重新复制。");
    for (const key of p.keys()) if (p.getAll(key).length > 1) throw new Error("定位链接含重复参数。");
    const allowed = (key: string, values: string[]) => { const value = p.get(key); if (value != null && !values.includes(value)) throw new Error("定位链接中的筛选或排序条件无效。"); return value; };
    result.order = (allowed("order", ["plan", "correctness", "coverage", "speed"]) ?? "plan") as ReportOrder;
    if (p.has("model") && p.get("model") !== "all") {
      const model = integer(p.get("model"), batch.plan.input.models.length - 1);
      if (model == null) throw new Error("定位链接中的模型不存在。");
      result.model = String(model);
    }
    const state = allowed("state", states), evaluator = allowed("ability", Object.keys(ruleName)), difficulty = allowed("difficulty", ["easy", "medium", "hard"]);
    if (state) result.filter.state = state;
    if (evaluator) result.filter.evaluator = evaluator;
    if (difficulty) result.filter.difficulty = difficulty;
    if (p.has("repeat")) { const n = integer(p.get("repeat"), batch.plan.input.repetitions ?? 1); if (!n) throw new Error("定位链接中的重复轮次不存在。"); result.filter.repetition = n; }
    for (const key of p.keys()) if (key.startsWith("page")) {
      const model = integer(key.slice(4), batch.plan.input.models.length - 1), page = integer(p.get(key));
      if (model == null || !page) throw new Error("定位链接中的页码无效。");
      result.pages[model] = page - 1;
    }
    if (p.has("item")) { result.item = integer(p.get("item")); if (result.item == null) throw new Error("定位链接中的题目序号无效。"); }
    if (p.has("trace")) { const trace = p.get("trace")!; if (!trace || trace.length > 128 || /[\u0000-\u001f]/.test(trace)) throw new Error("执行记录编号无效。"); result.trace = trace; }
  } catch (e) { result.error = e instanceof Error ? e.message : "定位链接无效。"; }
  return result;
}
export function useReportNavigation(workspaceId: string, batch: Batch | null, scope: ReportScope) {
  const route = useNavigation();
  const state = useMemo(() => batch ? read(route.hash, batch) : null, [route.hash, batch?.id, batch?.planHash]);
  const pointer = state?.item != null || state?.trace != null;
  const query = JSON.stringify([workspaceId, scope, batch?.id, state?.item, state?.trace, state?.filter]);
  const [located, setLocated] = useState<{ query: string; value?: ResultLocation; error?: string }>();
  const [retry, setRetry] = useState(0);
  const update = (patch: Record<string, string | number | null | undefined>, options: { replace?: boolean; resetPages?: boolean; state?: unknown } = {}) => {
    if (!batch) return;
    if (!matchesReport(window.location.hash, workspaceId, batch.id, scope)) return;
    const p = reportParams(window.location.hash);
    p.set("workspace", workspaceId); p.delete("back");
    if (options.resetPages) for (const key of [...p.keys()]) if (/^page\d+$/.test(key)) p.delete(key);
    for (const [key, value] of Object.entries(patch)) if (value == null) p.delete(key); else p.set(key, String(value));
    navigate("benchmark", Object.fromEntries(p), { replace: options.replace, state: options.state ?? (options.replace ? window.history.state : null) });
  };
  useEffect(() => {
    if (!batch || !state || state.error || !pointer || !matchesReport(route.hash, workspaceId, batch.id, scope)) return;
    const request = new AbortController();
    void benchmark("get_benchmark_result_location", { workspaceId, batchId: batch.id, scope, ordinal: state.item ?? null, executionRunId: state.trace ?? null, pageSize: 20, filter: state.filter }, request.signal)
      .then(value => {
        if (window.location.hash !== route.hash) return;
        setLocated({ query, value });
        if (value.matchesFilter && (state.model === "all" || state.model === String(value.modelIndex)) && (state.item !== value.ordinal || (state.pages[value.modelIndex] ?? 0) !== value.page)) update({ item: value.ordinal, [`page${value.modelIndex}`]: value.page! + 1 }, { replace: true });
      }).catch(e => { if (!request.signal.aborted) setLocated({ query, error: errorText(e) }); });
    return () => request.abort();
  }, [workspaceId, batch?.id, scope, query, retry, state?.error, route.hash]);
  const found = located?.query === query ? located : undefined;
  const mismatch = found?.value && (!found.value.matchesFilter || (state?.model !== "all" && state?.model !== String(found.value.modelIndex)));
  const locationError = state?.error || found?.error || (mismatch ? "这道题不符合当前模型或筛选条件。可以清除条件后定位。" : "");
  const selection = !locationError && found?.value && (state?.pages[found.value.modelIndex] ?? 0) === found.value.page ? found.value : undefined;
  return {
    state, locationError, locating: !!pointer && !found && !state?.error, selection, update,
    retry: () => setRetry(n => n + 1),
    clear: (keepItem = false) => update({ order: state?.error ? null : state?.order, model: null, state: null, ability: null, difficulty: null, repeat: null, ...(keepItem ? {} : { item: null, trace: null }) }, { resetPages: true, replace: true }),
    filter: (filter: ResultFilter) => update({ state: filter.state, ability: filter.evaluator, difficulty: filter.difficulty, repeat: filter.repetition, item: null, trace: null }, { resetPages: true }),
    open: (ordinal: number, modelIndex: number, page: number) => {
      setLocated({query:JSON.stringify([workspaceId,scope,batch?.id,ordinal,undefined,state?.filter]),value:{ordinal,modelIndex,page,matchesFilter:true}});
      update({item:ordinal,trace:null},{replace:!!pointer,state:pointer?window.history.state:{cortexReportReturn:window.location.hash}});
    },
    close: () => { const back = window.history.state?.cortexReportReturn; if (typeof back === "string" && reportParams(back).get("batch") === batch?.id && reportParams(back).get("workspace") === workspaceId) window.history.back(); else update({ item: null, trace: null }, { replace: true }); },
  };
}
