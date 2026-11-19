import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity } from "lucide-react";
import { SelectMenu } from "./SelectMenu";
import { errorText } from "../studio";
import { runtimeRoles, type RuntimeRole } from "../runtime-profiles";

type Usage = {
  provider: string; model: string; capability: RuntimeRole; requests: number; completed: number;
  inputTokens: number | null; outputTokens: number | null; vectors: number | null;
  averageMs: number | null; totalMs: number | null; systemCpuPeak: number | null;
  settledMicros: number | null; reservedMicros: number | null; unpriced: number;
  inputReported: number; outputReported: number;
};
type Report = {
  rows: Usage[]; retrieval: { count: number; averageMs: number | null };
  budget: { singleRunMicros: number | null; rollingDayMicros: number | null; accountedMicros: number; unpriced: number };
};
type Resources = {
  sampledAt: string; cpuPercent: number | null; systemMemory: { usedBytes: number; totalBytes: number } | null;
  gpus: { uuid: string; usedBytes: number | null; totalBytes: number | null; utilizationPercent: number | null }[] | null;
};
const providers: Record<string, string> = {
  ollama: "本机 Ollama", local: "本机", openai: "OpenAI", anthropic: "Anthropic", gemini: "Google Gemini", compatible: "兼容服务",
};
const integer = new Intl.NumberFormat("zh-CN");
const decimal = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 1 });
const currency = new Intl.NumberFormat("zh-CN", { style: "currency", currency: "USD", minimumFractionDigits: 2, maximumFractionDigits: 6 });
const cost = (value: number | null) => value == null ? "未知" : currency.format(value / 1_000_000);
const milliseconds = (value: number | null) => value == null ? "未知" : decimal.format(value) + " 毫秒";
const usagePercent = (value: number | null) => value == null ? "未报告" : decimal.format(value) + "%";
const gib = (value: number | null) => value == null ? "未报告" : decimal.format(value / 1024 ** 3) + " GiB";

export function RuntimeResources({ workspaceId }: { workspaceId: string }) {
  const [hours, setHours] = useState(24);
  const [revision, setRevision] = useState(0);
  const [report, setReport] = useState<Report>();
  const [resources, setResources] = useState<Resources>();
  const [error, setError] = useState("");
  const [resourceError, setResourceError] = useState("");
  const [loading, setLoading] = useState(true);
  const [refreshingDevice, setRefreshingDevice] = useState(false);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError("");
    void invoke<Report>("model_runtime_usage", { workspaceId, hours })
      .then(value => { if (active) setReport(value); })
      .catch(failure => { if (active) setError(errorText(failure)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [workspaceId, hours, revision]);

  async function sample() {
    setRefreshingDevice(true);
    setResourceError("");
    try { setResources(await invoke<Resources>("model_runtime_resources")); }
    catch (failure) { setResourceError(errorText(failure)); }
    finally { setRefreshingDevice(false); }
  }

  return <>
    <div className="section-description"><h2 id="runtime-usage-settings"><Activity size={18} aria-hidden="true" />资源与用量</h2></div>
    <div className="settings-fields">
      <div className="cloud-runtime-actions">
        <div className="field">
          <label htmlFor="runtime-usage-period">记录范围</label>
          <SelectMenu id="runtime-usage-period" value={String(hours)} disabled={loading}
            onChange={event => { setReport(undefined); setHours(Number(event.target.value)); }}>
            <option value="24">最近 24 小时</option>
            <option value="168">最近 7 天</option>
          </SelectMenu>
        </div>
        <button type="button" className="button secondary" disabled={loading} onClick={() => setRevision(revision + 1)}>刷新记录</button>
      </div>
      {loading && <p role="status">正在读取用量…</p>}
      {error && <p className="form-error" role="alert">{error}</p>}
      {report && <>
        <dl className="runtime-usage-facts">
          <div><dt>单次任务预算</dt><dd>{report.budget.singleRunMicros == null ? "不限" : cost(report.budget.singleRunMicros)}</dd></div>
          <div><dt>过去 24 小时预算占用</dt><dd>{cost(report.budget.accountedMicros)}
            {report.budget.rollingDayMicros != null && " / " + cost(report.budget.rollingDayMicros)}</dd></div>
          <div><dt>查找知识均时</dt><dd>{report.retrieval.count ? milliseconds(report.retrieval.averageMs) + "（" + report.retrieval.count + " 次）" : "暂无记录"}</dd></div>
        </dl>
        {report.budget.unpriced > 0 && <p>过去 24 小时有 {report.budget.unpriced} 次云端请求费用未知。</p>}
        {!report.rows.length ? <p>当前范围暂无模型用量。</p> : <ul className="runtime-usage-list" aria-label="按模型和能力的用量">
          {report.rows.map(row => <li key={[row.provider, row.model, row.capability].join("/")}>
            <strong>{row.model}</strong>
            <p>{providers[row.provider] ?? row.provider}　{runtimeRoles[row.capability]}　{integer.format(row.completed)} / {integer.format(row.requests)} 次完成</p>
            <dl className="runtime-usage-facts">
              {row.capability !== "reranking" && <div><dt>输入 Token</dt><dd>{row.inputTokens == null ? "未报告" : integer.format(row.inputTokens)}
                {row.inputReported > 0 && row.inputReported < row.requests && "（" + row.inputReported + " 次已报告）"}</dd></div>}
              {row.capability === "generation" && <div><dt>输出 Token</dt><dd>{row.outputTokens == null ? "未报告" : integer.format(row.outputTokens)}
                {row.outputReported > 0 && row.outputReported < row.requests && "（" + row.outputReported + " 次已报告）"}</dd></div>}
              {row.capability === "embedding" && <div><dt>生成向量</dt><dd>{row.vectors == null ? "未报告" : integer.format(row.vectors)}</dd></div>}
              {row.capability === "reranking" && row.inputTokens != null && <div><dt>输入 Token</dt><dd>{integer.format(row.inputTokens)}</dd></div>}
              <div><dt>平均 / 总用时</dt><dd>{milliseconds(row.averageMs)} / {milliseconds(row.totalMs)}</dd></div>
              {row.provider !== "ollama" && row.provider !== "local" && <>
                <div><dt>按返回用量估算</dt><dd>{cost(row.settledMicros)}</dd></div>
                {row.reservedMicros != null && <div><dt>尚未核算的预留额</dt><dd>{cost(row.reservedMicros)}</dd></div>}
              </>}
              {row.systemCpuPeak != null && <div><dt>最高单次平均 CPU 占用</dt><dd>{usagePercent(row.systemCpuPeak)}</dd></div>}
            </dl>
            {row.unpriced > 0 && <p>{row.unpriced} 次费用未知</p>}
          </li>)}
        </ul>}
      </>}
      <div className="cloud-runtime-actions">
        <h3>本机占用</h3>
        <button type="button" className="button secondary" disabled={refreshingDevice} onClick={() => void sample()}>
          {refreshingDevice ? "正在读取占用…" : "读取当前占用"}
        </button>
      </div>
      {resourceError && <p className="form-error" role="alert">{resourceError}</p>}
      {resources && <>
        <p>系统与整张显卡的采样　{new Date(resources.sampledAt).toLocaleTimeString("zh-CN", { hour12: false })}</p>
        <dl className="runtime-usage-facts">
          <div><dt>CPU</dt><dd>{usagePercent(resources.cpuPercent)}</dd></div>
          <div><dt>内存</dt><dd>{resources.systemMemory ? gib(resources.systemMemory.usedBytes) + " / " + gib(resources.systemMemory.totalBytes) : "未报告"}</dd></div>
          {resources.gpus?.length ? resources.gpus.map((gpu, index) => <div key={gpu.uuid}><dt>GPU {index + 1}</dt>
            <dd>{usagePercent(gpu.utilizationPercent)}　{gib(gpu.usedBytes)} / {gib(gpu.totalBytes)}</dd></div>)
            : <div><dt>GPU</dt><dd>暂无驱动读数</dd></div>}
        </dl>
      </>}
    </div>
  </>;
}
