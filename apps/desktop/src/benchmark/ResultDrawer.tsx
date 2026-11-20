import { benchmark } from "../api/benchmark";
import { notify } from "../components/Toast";
import { useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Link2, RotateCw, X } from "lucide-react";
import { AppDialog, openDialog } from "../components/AppDialog";
import { MarkdownContent } from "../components/MarkdownContent";
import { navigate } from "../navigation";
import { CaseMeasurements } from "./PerformanceDetails";
import { errorText, ruleName, stateName, type Batch, type Measurement, type ModelReport } from "./personalized";

export type ReportScope = "standard" | "personalized";
export interface ResultRow { ordinal: number; caseId: string; state: string; status: string; repetition: number; question: string; error: string | null; runId: string | null; evaluator?: string; difficulty?: string }
export interface ResultDetail {
  itemError?: string | null;
  case: { input: { prompt: string; response_schema?: unknown; tools?: unknown[] }; expected?: unknown; scoring?: { id: string; version: number; contract?: unknown }; fixture?: { id: string; title: string; lines: string[] }[]; content_hash?: string };
  output: string | null; performance: Measurement | null; summary?: { id: string };
  config?: unknown; model?: { cloudRequest?: unknown; provider?: string };
  execution: { run: { id: string; status: string; errorMessage?: string | null }; snapshot?: { request: unknown; requestHash: string } | null; contextManifest?: unknown; events?: unknown[] } | null;
  evaluations: { details?: { state?: string; regrade?: unknown }; [key: string]: unknown }[]; events: unknown[]; hardware: unknown; resources: unknown[];
}
const json = (value: unknown) => JSON.stringify(value, null, 2);

export function ResultDrawer({ id, workspaceId, batch, report, scope, row, rows, onSelect, onClose, returnFocus }: {
  id: string; workspaceId: string; batch: Batch; report: ModelReport; scope: ReportScope; row: ResultRow; rows: ResultRow[]; onSelect: (row: ResultRow) => void; onClose: () => void; returnFocus?: () => Element | null;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const identity = `${workspaceId}:${scope}:${batch.id}:${row.ordinal}`;
  const [response, setResponse] = useState<{ identity: string; detail: ResultDetail }>();
  const [request, setRequest] = useState<{ identity: string; loading: boolean; error: string }>();
  const [reload, setReload] = useState(0), [original, setOriginal] = useState(false);
  const [linkError,setLinkError] = useState("");
  const copy = async () => { try { await navigator.clipboard.writeText(window.location.hash); setLinkError(""); notify("定位链接已复制"); } catch { setLinkError("未能复制链接，请检查剪贴板权限后重试。"); } };
  const detail = response?.identity === identity ? response.detail : null;
  const loading = request?.identity !== identity || request.loading;
  const error = request?.identity === identity ? request.error : "";
  useEffect(() => { openDialog(dialog.current, returnFocus); }, []);
  useEffect(() => {
    let current = true, pending = false;
    let timer: number | undefined;
    const read = async () => {
      if (pending) return;
      pending = true;
      let again = ["running", "stopping"].includes(batch.status);
      try {
        const next = await benchmark("get_benchmark_result_detail", { workspaceId, batchId: batch.id, scope, ordinal: row.ordinal });
        again = again && next.evaluations.length === 0;
        if (current) { setResponse({ identity, detail: next }); setRequest({ identity, loading: false, error: "" }); }
      } catch (e) { if (current) setRequest({ identity, loading: false, error: errorText(e) }); }
      finally { pending = false; if (current && again) timer = window.setTimeout(() => void read(), 2000); }
    };
    setRequest({ identity, loading: true, error: "" }); void read();
    return () => { current = false; if (timer !== undefined) window.clearTimeout(timer); };
  }, [identity, workspaceId, batch.id, batch.status, scope, row.ordinal, row.state, reload]);
  const position = rows.findIndex(item => item.ordinal === row.ordinal);
  const savedState = detail?.evaluations.filter(e=>!e.details?.regrade).at(-1)?.details?.state;
  const rawState = detail?.execution?.run.status;
  const state = savedState ?? (rawState === "running" ? "In Progress" : rawState === "completed" ? "Needs Review" : rawState === "cancelled" || rawState === "interrupted" ? "Cancelled" : rawState ? "Execution Error" : row.state);
  const errors = [...new Set((detail ? [detail.itemError, detail.execution?.run.errorMessage] : [row.error]).filter(Boolean))];
  const sources = detail?.case.fixture ?? [];
  const close = () => dialog.current?.close();
  return <AppDialog id={id} ref={dialog} className="side-drawer panel-dialog benchmark-result-drawer" aria-labelledby={`${id}-title`} onClose={onClose}>
    <header className="panel-header">
      <div className="benchmark-drawer-heading"><h2 id={`${id}-title`}>单题详情</h2><p title={report.name}>{report.name}</p></div>
      <div className="benchmark-drawer-actions"><button className="icon-button" aria-label="复制定位链接" title="复制定位链接" onClick={()=>void copy()}><Link2 size={18} aria-hidden="true" /></button><button className="icon-button" aria-label="刷新本题" title="刷新本题" disabled={loading} onClick={() => setReload(n => n + 1)}><RotateCw size={18} aria-hidden="true" /></button><button className="icon-button" aria-label="关闭单题详情" title="关闭单题详情" onClick={close}><X size={20} aria-hidden="true" /></button></div>
    </header>
    <div key={identity} className="panel-body benchmark-result-content" aria-busy={loading}>
      <div className="benchmark-result-meta"><strong>{stateName[state] ?? state}</strong><span>{row.caseId}</span><span>第 {row.repetition} 次</span>{row.evaluator && <span>{ruleName[row.evaluator] ?? row.evaluator}</span>}</div>
      {linkError&&<p role="alert" className="cloud-error">{linkError}</p>}
      {loading && <p role="status">正在读取本题…</p>}
      {error && <div role="alert" className="cloud-error"><p>{error}</p><button className="button secondary" onClick={() => setReload(n => n + 1)}>重新读取本题</button></div>}
      {errors.map(message => <p key={message} role="alert" className="cloud-error">{message}</p>)}
      {detail && <>
        <section aria-label="题目"><h3>题目</h3><p className="benchmark-question">{detail.case.input.prompt}</p></section>
        <section aria-label="模型回答"><header className="profile-header"><h3>模型回答</h3>{detail.output && <div className="benchmark-drawer-actions" role="group" aria-label="回答显示方式"><button className="button text-button" aria-pressed={!original} onClick={() => setOriginal(false)}>阅读</button><button className="button text-button" aria-pressed={original} onClick={() => setOriginal(true)}>原文</button></div>}</header>
          {detail.output ? original ? <pre className="benchmark-answer-raw">{detail.output}</pre> : <MarkdownContent content={detail.output} /> : <p>{detail.execution ? "本次运行没有保存文字输出。" : row.status === "queued" ? "尚未发送，没有模型输出。" : "尚无可查看的模型输出。"}</p>}
        </section>
        <details className="personal-details"><summary>评分依据（{detail.evaluations.length} 条记录）</summary>{detail.evaluations.length ? <pre>{json(detail.evaluations)}</pre> : <p>尚无已保存评分。</p>}</details>
        <details className="personal-details"><summary>标准答案与题目要求</summary><pre>{json({ expected: detail.case.expected, scoring: detail.case.scoring, responseSchema: detail.case.input.response_schema, tools: detail.case.input.tools })}</pre></details>
        <details className="personal-details benchmark-case-sources"><summary>题目附带的固定资料（{sources.length} 份）</summary>{sources.length ? sources.map(source => <section key={source.id}><h3>{source.title}</h3><p>资料编号：{source.id}</p><ol>{source.lines.map((line, index) => <li key={index}>{line}</li>)}</ol></section>) : <p>这道题没有附带固定资料。</p>}</details>
        <CaseMeasurements key={identity} value={detail.performance ?? undefined} workspaceId={workspaceId} runId={detail.summary?.id ?? null} stored={{ hardware: detail.hardware, resources: detail.resources }} />
        <details className="personal-details"><summary>版本、模型与请求</summary>
          <h3>冻结计划</h3><pre>{json({ batchId: batch.id, planHash: batch.planHash, datasetId: batch.plan.input.datasetId, datasetVersion: batch.plan.input.datasetVersion, datasetHash: batch.plan.input.datasetHash, model: batch.plan.input.models[report.modelIndex], cloudBinding: batch.plan.binding })}</pre>
          {detail.config != null && <details><summary>已保存的运行设置</summary><pre>{json(detail.config)}</pre></details>}
          {detail.model != null && <details><summary>已保存的模型检查记录</summary><pre>{json(detail.model)}</pre></details>}
          {detail.execution ? <>
            <details><summary>已保存的请求内容</summary>{detail.model?.cloudRequest != null ? <pre>{json(detail.model.cloudRequest)}</pre> : detail.model?.provider ? <p>未保存可查看的云端请求内容。</p> : detail.execution.snapshot ? <pre>{json(detail.execution.snapshot)}</pre> : <p>没有已保存的请求快照。</p>}</details>
            <details><summary>本次上下文清单</summary><pre>{json(detail.execution.contextManifest)}</pre></details>
          </> : <p>尚未创建模型运行，没有实际请求快照。</p>}
        </details>
        <details className="personal-details"><summary>实际处理记录</summary>{detail.execution?.events?.length ? <><h3>执行记录</h3><pre>{json(detail.execution.events)}</pre></> : <p>尚无已保存的执行记录。</p>}{!!detail.events.length && <><h3>评测记录</h3><pre>{json(detail.events)}</pre></>}</details>
        {detail.execution && <button className="button secondary" onClick={() => navigate("graph", { workspace: workspaceId, run: detail.execution!.run.id, mode: "execution", back: window.location.hash })}>查看 3D 执行过程</button>}
      </>}
    </div>
    <footer className="panel-footer benchmark-drawer-footer"><button className="button text-button" onClick={close}>返回结果列表</button><nav className="benchmark-drawer-actions" aria-label="当前页题目切换"><button className="button secondary" disabled={position <= 0} onClick={() => onSelect(rows[position - 1])}><ChevronLeft size={16} aria-hidden="true" />上一题</button><button className="button secondary" disabled={position < 0 || position + 1 >= rows.length} onClick={() => onSelect(rows[position + 1])}>下一题<ChevronRight size={16} aria-hidden="true" /></button></nav></footer>
  </AppDialog>;
}
