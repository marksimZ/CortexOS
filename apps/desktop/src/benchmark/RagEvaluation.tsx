import { invoke } from "@tauri-apps/api/core";
import { useEffect, useId, useState } from "react";
import { SelectMenu } from "../components/SelectMenu";
import { errorText, type KnowledgeReference, type RetrievalReport } from "../studio";
import { RagRuntimeEvaluation } from "./RagRuntimeEvaluation";

const policies = {
  off: "关闭查找",
  fts5: "按词语查找",
  hybrid: "词语与语义（固定向量）",
  hybrid_rerank: "混合查找并重排（固定向量）",
  graph: "词语与知识关系",
  version_aware: "按题目指定的日期",
};
export type Policy = keyof typeof policies;

const retrievalMetrics = {
  recallAt5: "前 5 名召回率（Recall@5）",
  mrr: "首个命中排名（MRR@5）",
  ndcg: "排序质量（nDCG@5）",
  contextPrecision: "已选资料准确率",
  contextRecall: "已选资料召回率",
  conflictDetectionAccuracy: "检索冲突识别准确率",
  retrievalLatencyMs: "查找用时",
  contextEfficiency: "相关资料所占字节比例",
};
const replayMetrics = {
  citationPrecision: "引用准确率",
  citationRecall: "引用召回率",
  faithfulness: "引用原文支持率",
  abstentionAccuracy: "拒答规则通过率",
};
interface SuiteInfo {
  version: number; caseCount: number; documentCount: number; snapshotDate: string;
  datasetHash: string; snapshotHash: string; scorerVersion: string; policyVersion: string;
}
interface History { id: string; createdAt: string; strategies: [Policy, Policy]; datasetVersion: number }
type Source = { name: string; version: number; startLine: number; endLine: number; match?: { startLine: number; endLine: number } | null };
interface CaseResult {
  id: string; kind: string; question: string; factDate: string | null; expected: [string, number][];
  caseScope?: { factDate?: string } | null;
  usedReferences: KnowledgeReference[];
  detectedConflict: boolean;
  graph?: RetrievalReport["graph"];
  candidates: { nodeId: string; title: string; version: number; sourceLocation: Source | null; included: boolean; reason: string }[];
  replay: { text: string; checks: { claim: string; marker: string; reference: [string, number] | null; locationError: string | null; grounded: boolean; supportsExpected: boolean }[] };
}
interface Result {
  policy: Policy;
  summary: Record<string, { value: number | null; cases: number }>;
  cases: CaseResult[];
}
interface Report { id: string; createdAt: string; dataset: SuiteInfo; strategies: [Policy, Policy]; results: [Result, Result] }
interface Catalog { dataset: SuiteInfo; history: History[]; report: Report | null }

const percent = new Intl.NumberFormat("zh-CN", { style: "percent", maximumFractionDigits: 1 });
const milliseconds = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 });
const reasons: Record<string, string> = {
  included: "已选用", auto_limit: "超过条数上限", auto_budget: "超过容量上限",
};

export function RagEvaluation({ desktop, workspaceId }: { desktop: boolean; workspaceId: string | null }) {
  if (!desktop) return <p>请在桌面应用中运行固定检索评测。</p>;
  if (!workspaceId) return <p>请先选择工作区。</p>;
  return <EvaluationTabs key={workspaceId} workspaceId={workspaceId} />;
}

function EvaluationTabs({ workspaceId }: { workspaceId: string }) {
  const [runtime, setRuntime] = useState(false);
  return <>
    <div className="benchmark-tabs rag-evaluation-tabs" role="group" aria-label="检索评测方式">
      <button type="button" aria-pressed={!runtime} onClick={() => setRuntime(false)}>固定回放</button>
      <button type="button" aria-pressed={runtime} onClick={() => setRuntime(true)}>模型组合对照</button>
    </div>
    {runtime ? <RagRuntimeEvaluation workspaceId={workspaceId} /> : <WorkspaceEvaluation workspaceId={workspaceId} />}
  </>;
}

function WorkspaceEvaluation({ workspaceId }: { workspaceId: string }) {
  const id = useId();
  const [a, setA] = useState<Policy>("fts5");
  const [b, setB] = useState<Policy>("graph");
  const [catalog, setCatalog] = useState<Catalog>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let current = true;
    void invoke<Catalog>("get_rag_evaluation", { workspaceId }).then(value => {
      if (current) {
        setCatalog(value);
        if (value.report) { setA(value.report.strategies[0]); setB(value.report.strategies[1]); }
      }
    }).catch(error => { if (current) setError(errorText(error)); });
    return () => { current = false; };
  }, [workspaceId]);

  async function load(evaluationId?: string) {
    setBusy(true);
    setError("");
    try {
      const value = await invoke<Catalog>("get_rag_evaluation", { workspaceId, evaluationId });
      setCatalog(value);
      if (value.report) { setA(value.report.strategies[0]); setB(value.report.strategies[1]); }
    }
    catch (error) { setError(errorText(error)); }
    finally { setBusy(false); }
  }

  async function run() {
    if (!catalog || busy || a === b) return;
    setBusy(true);
    setError("");
    try {
      const report = await invoke<Report>("run_rag_evaluation", { workspaceId, policyA: a, policyB: b });
      const entry: History = { id: report.id, createdAt: report.createdAt, strategies: report.strategies, datasetVersion: report.dataset.version };
      setCatalog({ ...catalog, report, history: [entry, ...catalog.history] });
    } catch (error) { setError(errorText(error)); }
    finally { setBusy(false); }
  }

  const report = catalog?.report;
  const options = Object.entries(policies).map(([value, label]) => <option key={value} value={value}>{label}</option>);
  return <div className="personal-workbench rag-evaluation">
    <h2>策略对照</h2>
    <p>固定知识与题目，使用固定向量和回答回放。真实模型与模型判分尚未运行。</p>
    <div className="rag-evaluation-controls">
      <label htmlFor={`${id}-a`}>策略 A</label>
      <SelectMenu id={`${id}-a`} value={a} disabled={busy} onChange={event => setA(event.target.value as Policy)}>{options}</SelectMenu>
      <label htmlFor={`${id}-b`}>策略 B</label>
      <SelectMenu id={`${id}-b`} value={b} disabled={busy} onChange={event => setB(event.target.value as Policy)}>{options}</SelectMenu>
      <button type="button" className="button primary" disabled={!catalog || busy || a === b} onClick={() => void run()}>运行对照</button>
    </div>
    {a === b && <p>请选择两种不同的策略。</p>}
    {busy && <p role="status">正在处理评测…</p>}
    {!catalog && !error && !busy && <p role="status">正在读取题库与历史记录…</p>}
    {error && <div className="cloud-error" role="alert"><p>{error}</p><button type="button" className="button secondary" disabled={busy} onClick={() => void load(report?.id)}>重新读取</button></div>}
    {!!catalog?.history.length && <div className="rag-evaluation-controls">
      <label htmlFor={`${id}-history`}>历史对照</label>
      <SelectMenu id={`${id}-history`} value={report?.id ?? ""} disabled={busy} onChange={event => void load(event.target.value)}>
        {catalog.history.map(entry => <option key={entry.id} value={entry.id}>
          {new Date(entry.createdAt).toLocaleString("zh-CN")}，{entry.strategies.map(policy => policies[policy] ?? policy).join(" / ")}，题库 v{entry.datasetVersion}
        </option>)}
      </SelectMenu>
    </div>}
    {catalog && <DatasetDetails dataset={report?.dataset ?? catalog.dataset} />}
    {report && <section key={report.id} aria-label="对照结果">
      <h3>检索结果</h3>
      <MetricsTable results={report.results} labels={retrievalMetrics} name="检索结果" />
      <details className="knowledge-disclosure">
        <summary>固定回答回放评分</summary>
        <MetricsTable results={report.results} labels={replayMetrics} name="固定回答回放评分" />
      </details>
      <details className="knowledge-disclosure">
        <summary>逐题检查（{report.dataset.caseCount}）</summary>
        {report.results[0].cases.map((item, index) => <details className="knowledge-disclosure" key={item.id}>
          <summary>{item.kind}　{item.question}</summary>
          <p>标准资料 {item.expected.length ? item.expected.map(([id, version]) => `${id} v${version}`).join("，") : "无"}</p>
          {item.factDate && <p>题目指定日期 {item.factDate}</p>}
          <div className="rag-evaluation-pair">
            {report.results.map((result, side) => <CaseDetails key={result.policy} title={`${side ? "B" : "A"} ${policies[result.policy] ?? result.policy}`} result={result.cases[index]} />)}
          </div>
        </details>)}
      </details>
    </section>}
  </div>;
}

function DatasetDetails({ dataset }: { dataset: SuiteInfo }) {
  return <details className="knowledge-disclosure">
    <summary>题库 v{dataset.version}，{dataset.caseCount} 题</summary>
    <p>知识基准日 {dataset.snapshotDate}，{dataset.documentCount} 份文件。</p>
    <dl className="rag-evaluation-identity">
      <dt>题库指纹</dt><dd><code>{dataset.datasetHash}</code></dd>
      <dt>固定知识指纹</dt><dd><code>{dataset.snapshotHash}</code></dd>
      <dt>评分器</dt><dd>{dataset.scorerVersion}</dd>
      <dt>策略版本</dt><dd>{dataset.policyVersion}</dd>
    </dl>
  </details>;
}

function MetricsTable({ results, labels, name }: { results: [Result, Result]; labels: Record<string, string>; name: string }) {
  return <div className="rag-evaluation-table" tabIndex={0} role="region" aria-label={`${name}表`}>
    <table>
      <thead><tr><th scope="col">指标</th>{results.map((result, index) => <th key={result.policy} scope="col">{index ? "B" : "A"} {policies[result.policy] ?? result.policy}</th>)}</tr></thead>
      <tbody>{Object.entries(labels).map(([key, label]) => <tr key={key}>
        <th scope="row">{label}</th>{results.map(result => {
          const measure = result.summary[key];
          return <td key={result.policy} className="numeric">{measure?.value == null ? "无适用题目" : `${key === "retrievalLatencyMs"
            ? `${milliseconds.format(measure.value)} 毫秒` : percent.format(measure.value)}（${measure.cases} 题）`}</td>;
        })}
      </tr>)}</tbody>
    </table>
  </div>;
}

function CaseDetails({ title, result }: { title: string; result?: CaseResult }) {
  if (!result) return <p>该侧没有保存此题的结果。</p>;
  return <section>
    <h4>{title}</h4>
    {result.caseScope?.factDate && <p>使用日期 {result.caseScope.factDate}</p>}
    {result.detectedConflict && <p>找到已登记的冲突。</p>}
    <p>选用 {result.usedReferences.length} 条资料，候选 {result.candidates.length} 条。</p>
    <ol className="rag-evaluation-candidates">
      {result.candidates.map(item => <li key={`${item.nodeId}:${item.version}`}>
        <strong>{item.title} v{item.version}</strong>，{reasons[item.reason] ?? "未选用"}
        {item.sourceLocation && <p>{item.sourceLocation.name} v{item.sourceLocation.version}，第 {(item.sourceLocation.match ?? item.sourceLocation).startLine}–{(item.sourceLocation.match ?? item.sourceLocation).endLine} 行</p>}
      </li>)}
    </ol>
    {!!result.usedReferences.length && <details>
      <summary>实际选用的资料文字</summary>
      <ul>{result.usedReferences.map(item => <li key={`${item.nodeId}:${item.version}`}>
        <strong>{item.citationId} {item.title} v{item.version}</strong>
        <p className="context-content">{item.content}</p>
      </li>)}</ul>
    </details>}
    {!!result.graph?.paths.length && <details>
      <summary>关系查找路径</summary>
      <ul>{result.graph.paths.map(path => <li key={path.steps.map(step => step.edgeId).join("/")}>
        {path.nodes.map(node => node.title).join(" → ")}
      </li>)}</ul>
    </details>}
    <details>
      <summary>固定回答与引用核对</summary>
      <p className="context-content">{result.replay.text}</p>
      <ul>{result.replay.checks.map((check, index) => <li key={index}>
        {check.marker} {check.reference && `${check.reference[0]} v${check.reference[1]}，`}
        {check.locationError ?? (check.supportsExpected ? "支持标准结论" : check.grounded ? "有原文依据，未命中预期资料" : "引用原文未提供这项内容")}
      </li>)}</ul>
    </details>
  </section>;
}
