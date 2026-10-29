import { SelectMenu } from "../components/SelectMenu";
import { ruleName, stateName, type Batch, type ModelReport } from "./personalized";

const capabilityName: Record<string,string> = { completion: "文本回答", json_schema: "JSON 结构化输出", tool_calls: "原生工具调用" };

export interface ResultFilter { state?: string; evaluator?: string; difficulty?: string; repetition?: number }
export type ReportOrder = "plan" | "correctness" | "coverage" | "speed";
export function sortedReports(reports: ModelReport[], order: ReportOrder) {
  const value = (r: ModelReport) => order === "correctness" ? r.statistics.correctness : order === "coverage" ? r.statistics.scoringCoverage : r.performance.tokensPerSecond;
  return [...reports].sort((a, b) => {
    if (order === "plan") return a.modelIndex - b.modelIndex;
    const x = value(a), y = value(b);
    const hasX = x != null && Number.isFinite(x), hasY = y != null && Number.isFinite(y);
    return hasX && hasY ? y! - x! || a.modelIndex - b.modelIndex : Number(hasY) - Number(hasX) || a.modelIndex - b.modelIndex;
  });
}
export function ResultFilters({ batch, filter, onFilter, model, onModel, onReset }: {
  batch: Batch; filter: ResultFilter; onFilter: (value: ResultFilter) => void; model: string; onModel: (value: string) => void; onReset: () => void;
}) {
  const choose = (key: keyof ResultFilter, value: string) => onFilter({ ...filter, [key]: value ? key === "repetition" ? Number(value) : value : undefined });
  return <div className="benchmark-filters" role="group" aria-label="逐题筛选">
    <label>查看模型<SelectMenu aria-label="查看模型" value={model} onChange={e => onModel(e.target.value)}><option value="all">全部模型</option>{batch.reports?.map(r => <option key={r.modelIndex} value={r.modelIndex}>{r.modelIndex + 1}. {r.name}</option>)}</SelectMenu></label>
    <label>结果状态<SelectMenu aria-label="结果状态" value={filter.state ?? ""} onChange={e => choose("state", e.target.value)}><option value="">全部状态</option>{["Passed", "Failed", "Unsupported", "Not Run", "Execution Error", "Scoring Error", "Needs Review", "In Progress", "Cancelled"].map(s => <option key={s} value={s}>{stateName[s]}</option>)}</SelectMenu></label>
    <label>测试能力<SelectMenu aria-label="测试能力" value={filter.evaluator ?? ""} onChange={e => choose("evaluator", e.target.value)}><option value="">全部能力</option>{Object.entries(ruleName).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</SelectMenu></label>
    <label>题目难度<SelectMenu aria-label="题目难度" value={filter.difficulty ?? ""} onChange={e => choose("difficulty", e.target.value)}><option value="">全部难度</option><option value="easy">基础</option><option value="medium">中等</option><option value="hard">困难</option></SelectMenu></label>
    <label>重复轮次<SelectMenu aria-label="重复轮次" value={filter.repetition ?? ""} onChange={e => choose("repetition", e.target.value)}><option value="">全部轮次</option>{Array.from({ length: batch.plan.input.repetitions ?? 1 }, (_, i) => <option key={i} value={i + 1}>第 {i + 1} 次</option>)}</SelectMenu></label>
    <button className="button text-button" onClick={onReset} disabled={model === "all" && !Object.values(filter).some(v => v != null)}>清除筛选</button>
  </div>;
}
export function ComparisonConditions({ batch }: { batch: Batch }) {
  const different = new Set(batch.plan.input.models.map(m => JSON.stringify(m.local?.parameters ?? null))).size > 1;
  return <details className="personal-details"><summary>比较条件{different ? " · 模型参数有差异" : ""}</summary>
    {different && <p>各模型的请求参数不同，速度和正确率需结合下列条件比较。</p>}
    <p>固定题库：{batch.plan.input.datasetId} v{batch.plan.input.datasetVersion}</p><p className="benchmark-config-hash">题库内容标识：{batch.plan.input.datasetHash}</p>
    {batch.plan.input.models.map((m, i) => <div key={i}><h4>{i + 1}. {m.local?.model ?? batch.plan.binding?.model ?? "未记录模型名称"}</h4>{m.local ? <>
      <p>Ollama · 请求上下文 {m.local.parameters.numCtx} Token · 回答上限 {m.local.parameters.numPredict} Token · 温度 {m.local.parameters.temperature} · 超时 {m.local.parameters.timeoutSeconds} 秒</p>
      <p className="benchmark-config-hash">计划锁定的模型标识：{m.local.expectedDigest ?? "未记录"}</p><p>请求能力：{m.local.requirements.map(r => capabilityName[r] ?? r).join("、")}</p>
    </> : <p>{batch.plan.binding?.provider ?? "未记录 Provider"} · 云端参数以冻结请求为准</p>}</div>)}
    <p>请求上下文与回答上限是本次设置，实际用量见各题记录；未记录的量化方式和硬件信息不作推断。</p>
  </details>;
}
