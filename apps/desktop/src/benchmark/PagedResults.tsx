import { benchmark, type ResultPage } from "../api/benchmark";
import { ResultDrawer, type ReportScope as Scope, type ResultRow as Row } from "./ResultDrawer";
import type { ResultFilter } from "./ReportOptions";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { errorText, type Batch, type ModelReport } from "./personalized";
import { ResultStatus } from "./ResultStatus";

export function PagedResults({ workspaceId, batch, report, scope, filter, page, selection, onPage, onSelect, onClose }: { workspaceId: string; batch: Batch; report: ModelReport; scope: Scope; filter: ResultFilter; page: number; selection?: number; onPage: (page: number)=>void; onSelect: (ordinal: number)=>void; onClose: ()=>void }) {
  const [data, setData] = useState<(ResultPage & { key: string }) | null>(null), [cached, setCached] = useState<Row | null>(null);
  const [loading, setLoading] = useState(true), [error, setError] = useState(""), [reload, setReload] = useState(0);
  const drawerId = useId();
  const revealed = useRef<string | undefined>(undefined);
  const filterKey = JSON.stringify(filter), key = JSON.stringify([workspaceId,scope,batch.id,report.modelIndex,page,filterKey]);
  const revision = `${batch.status}:${JSON.stringify(report.statistics.states)}:${report.statistics.attempted}`;
  useEffect(() => {
    let current = true;
    setLoading(true); setError("");
    void benchmark("get_benchmark_result_page", { workspaceId, batchId: batch.id, scope, modelIndex: report.modelIndex, page, pageSize: 20, filter })
      .then(next => { if (current) { setData({...next,key}); setError(""); } })
      .catch(e => { if (current) setError(errorText(e)); })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [workspaceId, batch.id, scope, report.modelIndex, page, revision, reload, filterKey, key]);
  const rows = data?.key === key ? data.rows : [];
  const currentRow = rows.find(row=>row.ordinal===selection);
  const selected = selection==null?null:currentRow??(cached?.ordinal===selection?cached:null);
  useEffect(()=>{if(currentRow)setCached(currentRow);else if(selection==null)setCached(null);},[data,selection]);
  useLayoutEffect(()=>{
    if(data?.key!==key)return;
    const previous=revealed.current;revealed.current=key;
    if(selection==null&&((previous===undefined&&page===0)||previous===key))return;
    document.getElementById(selection==null?`${drawerId}-results`:`${drawerId}-item-${selection}`)?.scrollIntoView({block:"nearest"});
  },[data?.key,selection,key,page,drawerId]);
  const total = data?.key === key ? data.total : 0, pages = Math.max(1, Math.ceil(total / 20));
  const change = onPage;
  return <section id={`${drawerId}-results`} tabIndex={-1} className="benchmark-results" aria-label={`${report.name} 逐题结果`}>
    <header className="profile-header"><h4>逐题结果</h4><nav className="benchmark-pagination" aria-label={`${report.name} 结果分页`}>
      <button className="button text-button" disabled={loading || page === 0} onClick={() => change(page - 1)}>上一页</button>
      <span className="benchmark-page-count">{data?.key===key?`第 ${page + 1} / ${pages} 页 · ${total} 项`:`第 ${page + 1} 页`}</span><span>匹配 {data?.key===key ? total : "…"} / 计划 {report.statistics.planned} 项</span>
      <button className="button text-button" disabled={loading || page + 1 >= pages} onClick={() => change(page + 1)}>下一页</button>
    </nav></header>
    {error && <div role="alert" className="cloud-error"><p>{error}</p><button className="button secondary" onClick={() => setReload(n => n + 1)}>重新读取本页</button>{page > 0 && <button className="button text-button" onClick={() => change(0)}>返回第一页</button>}</div>}
    {loading && <p role="status">正在读取本页…</p>}
    {!loading && !error && rows.length === 0 && <p>没有符合筛选条件的结果。</p>}
    <div aria-busy={loading}>{rows.map(row => <div className="personal-details benchmark-result" key={row.ordinal}>
      <button id={`${drawerId}-item-${row.ordinal}`} className="benchmark-result-button" aria-haspopup="dialog" aria-expanded={selected?.ordinal === row.ordinal} aria-controls={selected?.ordinal === row.ordinal ? drawerId : undefined} onClick={() => onSelect(row.ordinal)}>
        <span className="benchmark-result-ordinal">{row.ordinal + 1}.</span>
        <span className="benchmark-result-question">{row.question}</span>
        <span className="benchmark-result-repetition">第 {row.repetition} 次</span>
        <ResultStatus state={row.state} />
      </button>
    </div>)}</div>
    {selected && <ResultDrawer id={drawerId} workspaceId={workspaceId} batch={batch} report={report} scope={scope} row={rows.find(row => row.ordinal === selected.ordinal) ?? selected} rows={rows} onSelect={row=>onSelect(row.ordinal)} onClose={onClose} returnFocus={()=>document.getElementById(`${drawerId}-item-${selected.ordinal}`)??document.getElementById(`${drawerId}-results`)} />}
  </section>;
}
