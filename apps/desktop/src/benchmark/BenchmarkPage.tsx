import { invoke } from "@tauri-apps/api/core";
import { navigationHash, useNavigation } from "../navigation";
import { BenchmarkLink } from "./BenchmarkLink";
import { reportParams } from "./reportNavigation";
import { useEffect, useState } from "react";
import type { Studio } from "../studio";
import { lazyView } from "../components/lazyView";
import "./benchmark.css";

const HarnessFitLab = lazyView(() => import("./harness/HarnessFitLab"), (module) => module.HarnessFitLab);
const StandardBenchmarks = lazyView(() => import("./StandardBenchmarks"), (module) => module.StandardBenchmarks);
const CloudConnection = lazyView(() => import("./CloudConnection"), (module) => module.CloudConnection);
const PersonalizedWorkbench = lazyView(() => import("./PersonalizedWorkbench"), (module) => module.PersonalizedWorkbench);
const RagEvaluation = lazyView(() => import("./RagEvaluation"), (module) => module.RagEvaluation);

export function BenchmarkPage({ studio, personalized }: { studio: Studio; personalized: boolean }) {
  const [profileOpen, setProfileOpen] = useState(false);
  const route=useNavigation(),requestedBatch=reportParams(route.hash).get("batch");
  const harness=new URLSearchParams(route.hash.split('?')[1]).get('view')==='harness';
  const rag=reportParams(route.hash).get("view")==="rag";
  useEffect(()=>{if(!personalized||!requestedBatch)return;let current=true;void invoke<{allowed:boolean}>("personalized_access").then(status=>{if(current&&status.allowed)setProfileOpen(true);}).catch(()=>{});return()=>{current=false;};},[personalized,requestedBatch]);
  if(route.workspace&&route.workspace!==studio.workspaceId)return <div><p role="status">{studio.data&&!studio.data.workspaces.some(w=>w.id===route.workspace)?"链接中的工作区不存在。":"正在切换到链接中的工作区…"}</p><button className="button secondary" onClick={()=>{studio.clearError();window.location.hash=navigationHash("benchmark",{workspace:studio.workspaceId});}}>返回当前工作区</button></div>;
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
