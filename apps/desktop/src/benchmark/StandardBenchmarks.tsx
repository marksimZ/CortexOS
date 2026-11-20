import { benchmark } from "../api/benchmark";
import { useEffect, useState } from "react";
import { navigate, useNavigation } from "../navigation";
import { StandardSetup } from "./StandardSetup";
import { PersonalizedReport } from "./PersonalizedReport";
import { useBatchOverview } from "./useBatchOverview";
import { reportParams } from "./reportNavigation";
import { errorText, type History } from "./personalized";
export function StandardBenchmarks({ desktop, workspaceId }: { desktop: boolean; workspaceId: string | null }) {
  if (!desktop) return <section className="benchmark-catalog"><h2>本地标准评测</h2><p>请在桌面应用中使用本机模型。</p></section>;
  if (!workspaceId) return <p>请先选择工作区。</p>;
  return <WorkspaceBenchmarks key={workspaceId} workspaceId={workspaceId} />;
}
function WorkspaceBenchmarks({ workspaceId }: { workspaceId: string }) {
  const overview = useBatchOverview(workspaceId, "standard"), route = useNavigation();
  const [history, setHistory] = useState<History[]>([]), [error, setError] = useState("");
  const params = reportParams(route.hash), showSetup = !overview.id && params.get("tab") !== "history";
  useEffect(() => {
    let current = true;
    void benchmark("list_standard_benchmarks", { workspaceId }).then(items => {
      if (!current) return;
      setHistory(items); setError("");
      const p = reportParams(window.location.hash);
      if (!p.has("batch") && !p.has("tab")) { const active = items.find(h => ["running", "stopping"].includes(h.status)); if (active) overview.open(active.id, true); }
    }).catch(e => { if (current) setError(errorText(e)); });
    return () => { current = false; };
  }, [workspaceId, overview.id, overview.batch?.status]);
  const setup = () => navigate("benchmark", { workspace: workspaceId, tab: "setup" });
  return <div className="personal-workbench">
    {(error || overview.error) && <div className="cloud-error" role="alert"><p>{error || overview.error}</p><button className="button secondary" onClick={overview.refresh}>重新读取</button><button className="button text-button" onClick={() => navigate("benchmark", { workspace: workspaceId, tab: "history" })}>返回评测列表</button></div>}
    <nav className="profile-steps" aria-label="标准评测操作"><button className="button secondary" aria-current={showSetup ? "page" : undefined} onClick={setup}>新建标准评测</button><button className="button secondary" aria-current={!showSetup ? "page" : undefined} onClick={() => { if (overview.id) overview.refresh(); else navigate("benchmark", { workspace: workspaceId, tab: "history" }); }}>运行与历史{overview.batch && ["running", "stopping"].includes(overview.batch.status) ? " · 正在进行" : ""}</button></nav>
    {showSetup ? <StandardSetup workspaceId={workspaceId} onPlan={batch => overview.open(batch.id)} /> : overview.loading ? <p role="status">正在读取评测报告…</p> : <PersonalizedReport key={overview.id ?? "history"} scope="standard" workspaceId={workspaceId} batch={overview.batch} onBatch={overview.update} history={history} onOpen={overview.open} onNew={setup} />}
  </div>;
}
