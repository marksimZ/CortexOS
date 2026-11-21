import { benchmark } from "../api/benchmark";
import { navigate, navigationHash, useNavigation } from "../navigation";
import { BenchmarkLink } from "./BenchmarkLink";
import { useEffect, useState } from "react";
import type { Studio } from "../studio";
import { lazyView } from "../components/lazyView";
import "./benchmark.css";

const HarnessFitLab = lazyView(() => import("./harness/HarnessFitLab"), (module) => module.HarnessFitLab);
const StandardBenchmarks = lazyView(() => import("./StandardBenchmarks"), (module) => module.StandardBenchmarks);
const CloudConnection = lazyView(() => import("./CloudConnection"), (module) => module.CloudConnection);
const PersonalizedWorkbench = lazyView(() => import("./PersonalizedWorkbench"), (module) => module.PersonalizedWorkbench);
const RagEvaluation = lazyView(() => import("./RagEvaluation"), (module) => module.RagEvaluation);

export function BenchmarkPage({ studio }: { studio: Studio }) {
  const [profileOpen, setProfileOpen] = useState(false);
  const route = useNavigation();
  const requestedBatch = route.params.get("batch");
  const requestedJob = route.params.get("job");
  const harness = route.benchmarkView === "harness";
  const rag = route.benchmarkView === "rag";
  const personalized = route.benchmarkView === "personalized";
  useEffect(() => {
    if (!personalized || (!requestedBatch && !requestedJob)) return;
    const request = new AbortController();
    void benchmark("personalized_access", undefined, request.signal)
      .then(status => { if (status.allowed) setProfileOpen(true); })
      .catch(() => {}); // The connection screen remains available when access expires.
    return () => request.abort();
  }, [personalized, requestedBatch, requestedJob]);
  if(route.workspace&&route.workspace!==studio.workspaceId)return <div><p role="status">{studio.data&&!studio.data.workspaces.some(w=>w.id===route.workspace)?"链接中的工作区不存在。":"正在切换到链接中的工作区…"}</p><button className="button secondary" onClick={()=>{studio.clearError();navigate("benchmark",{workspace:studio.workspaceId});}}>返回当前工作区</button></div>;
  return <section className="standard-page benchmark-page" aria-labelledby="page-title">
    <h1 id="page-title" className="sr-only">评测</h1>
    <nav className="benchmark-tabs" aria-label="评测类型">
      <a href={navigationHash("benchmark",{workspace:studio.workspaceId})} aria-current={!personalized&&!harness&&!rag ? "page" : undefined}>标准评测</a>
      <a href={navigationHash("benchmark",{workspace:studio.workspaceId,view:"personalized"})} aria-current={personalized ? "page" : undefined}>个性化评测</a>
      <a href={navigationHash("benchmark",{workspace:studio.workspaceId,view:"harness"})} aria-current={harness ? "page" : undefined}>项目适配</a>
      <a href={navigationHash("benchmark",{workspace:studio.workspaceId,view:"rag"})} aria-current={rag ? "page" : undefined}>检索评测</a>
    </nav>
    {!harness&&!rag&&<BenchmarkLink />}
    <div className={`page-content benchmark-content${harness?' harness-content':''}`}>
      {rag ? <RagEvaluation desktop={studio.desktop} workspaceId={studio.workspaceId} /> : harness?<HarnessFitLab workspaceId={studio.workspaceId} desktop={studio.desktop}/>:personalized ? profileOpen ? <PersonalizedWorkbench workspaceId={studio.workspaceId} onConnection={()=>setProfileOpen(false)} /> : <CloudConnection desktop={studio.desktop} onContinue={()=>setProfileOpen(true)} /> : <StandardBenchmarks desktop={studio.desktop} workspaceId={studio.workspaceId}/>}
    </div>
  </section>;
}
