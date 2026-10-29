import { invoke } from "@tauri-apps/api/core";
import { navigationHash, useNavigation } from "../navigation";
import { BenchmarkLink } from "./BenchmarkLink";
import { reportParams } from "./reportNavigation";
import { useEffect, useState } from "react";
import { StandardBenchmarks } from "./StandardBenchmarks";
import type { Studio } from "../studio";
import { CloudConnection } from "./CloudConnection";
import { PersonalizedWorkbench } from "./PersonalizedWorkbench";
import "./benchmark.css";

export function BenchmarkPage({ studio, personalized }: { studio: Studio; personalized: boolean }) {
  const [profileOpen, setProfileOpen] = useState(false);
  const route=useNavigation(),requestedBatch=reportParams(route.hash).get("batch");
  useEffect(()=>{if(!personalized||!requestedBatch)return;let current=true;void invoke<{allowed:boolean}>("personalized_access").then(status=>{if(current&&status.allowed)setProfileOpen(true);}).catch(()=>{});return()=>{current=false;};},[personalized,requestedBatch]);
  if(route.workspace&&route.workspace!==studio.workspaceId)return <div><p role="status">{studio.data&&!studio.data.workspaces.some(w=>w.id===route.workspace)?"链接中的工作区不存在。":"正在切换到链接中的工作区…"}</p><button className="button secondary" onClick={()=>{studio.clearError();window.location.hash=navigationHash("benchmark",{workspace:studio.workspaceId});}}>返回当前工作区</button></div>;
  return <section className="standard-page benchmark-page" aria-labelledby="page-title">
    <h1 id="page-title" className="sr-only">评测</h1>
    <nav className="benchmark-tabs" aria-label="评测类型">
      <a href={navigationHash("benchmark",{workspace:studio.workspaceId})} aria-current={!personalized ? "page" : undefined}>标准评测</a>
      <a href={navigationHash("benchmark",{workspace:studio.workspaceId,view:"personalized"})} aria-current={personalized ? "page" : undefined}>个性化评测</a>
    </nav>
    <BenchmarkLink />
    <div className="page-content benchmark-content">
      {personalized ? profileOpen ? <PersonalizedWorkbench workspaceId={studio.workspaceId} onConnection={()=>setProfileOpen(false)} /> : <CloudConnection desktop={studio.desktop} onContinue={()=>setProfileOpen(true)} /> : <StandardBenchmarks desktop={studio.desktop} workspaceId={studio.workspaceId}/>}
    </div>
  </section>;
}
