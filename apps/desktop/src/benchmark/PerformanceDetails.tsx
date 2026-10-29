import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorText, type ModelReport, type Measurement, type Distribution } from "./personalized";

export const duration = (n: number | null | undefined) => n == null ? "不可用" : n < 1000 ? `${n.toFixed(0)} ms` : `${(n / 1000).toFixed(2)} s`;
const bytes = (n: number | null | undefined) => n == null ? "不可用" : `${(n / 1024 ** 3).toFixed(2)} GiB`;
const speed = (n: number | null | undefined) => n == null ? "不可用" : `${n.toFixed(1)} Token/s`;

export function PerformanceDetails({ report }: { report: ModelReport }) {
  const p = report.performance;
  const rows: [string, Distribution | undefined, (n: number | null | undefined) => string][] = [
    ["首个可见文字", p.firstTextMs, duration], ["首个工具响应", p.firstToolMs, duration],
    ["请求到响应结束", p.responseMs, duration], ["模型加载", p.loadMs, duration],
    ["处理输入", p.promptMs, duration], ["模型生成", p.generationMs, duration],
    ["生成速度", p.generationSpeed, speed], ["单题全程（含评分保存）", p.logicalMs, duration],
  ];
  return <details className="personal-details benchmark-measurements"><summary>速度、Token 与资源</summary>
    <div className="personal-table-wrap" tabIndex={0} role="region" aria-label="耗时与资源指标表"><table className="personal-table"><caption>完成样本的耗时分布；单题全程包含已记录的停止与失败</caption><thead><tr><th>指标</th><th>中位数</th><th>P95</th><th>有效 / 计划</th></tr></thead>
      <tbody>{rows.map(([label, d, format]) => <tr key={label}><th>{label}</th><td>{format(d?.p50)}</td><td>{format(d?.p95)}</td><td>{d?.n ?? 0} / {report.statistics.planned}</td></tr>)}</tbody></table></div>
    <p>生成速度按模型返回的输出 Token 与生成时长计算。首字指可见文字，工具响应单独记录；少量样本的 P95 仅供本次参考。</p>
    <dl className="benchmark-measurement-facts">
      <dt>输入 Token</dt><dd>{p.inputTokensObserved ?? "不可用"}（{p.inputTokenCases ?? 0} 次有记录）</dd>
      <dt>输出 Token</dt><dd>{p.outputTokensObserved ?? "不可用"}（{p.outputTokenCases ?? 0} 次有记录）</dd>
      <dt>缓存输入 Token</dt><dd>{p.cachedInputTokensObserved ?? "不可用"}（{p.cachedInputTokenCases ?? 0} 次有记录）</dd>
      <dt>整机 CPU 采样峰值</dt><dd>{p.resources?.systemCpuPercentPeak == null ? "不可用" : `${p.resources.systemCpuPercentPeak.toFixed(1)}%`}</dd>
      <dt>整机内存采样峰值</dt><dd>{bytes(p.resources?.systemMemoryBytesPeak)}</dd>
      <dt>整卡显存采样峰值</dt><dd>{p.resources?.gpus?.length ? p.resources.gpus.map(g => <div key={g.uuid}>{bytes(g.usedBytesPeak)} · {g.samples} 次读数 · {g.uuid}</div>) : "不可用"}</dd>
      <dt>资源采样</dt><dd>{p.resources?.sampleCount ?? 0} 次，覆盖 {p.resources?.measuredCases ?? 0} / {report.statistics.planned} 次</dd>
    </dl><p>资源范围为整机与整张显卡，包含其他程序。默认每秒采样，峰值为已采到的最大读数；短请求样本较少。不同模型的 Token 不能视为相同文字量。</p>
    {!!p.byRepetition?.length && <div className="personal-table-wrap" tabIndex={0} role="region" aria-label="重复轮次指标表"><table className="personal-table"><caption>按重复轮次查看</caption><thead><tr><th>轮次</th><th>首字中位数</th><th>用时中位数</th><th>生成速度</th><th>速度样本</th></tr></thead><tbody>{p.byRepetition.map(r => <tr key={r.repetition}><th>第 {r.repetition} 次</th><td>{duration(r.performance.firstTextMs?.p50)}</td><td>{duration(r.performance.responseMs?.p50)}</td><td>{speed(r.performance.tokensPerSecond)}</td><td>{r.performance.speedCases ?? 0}</td></tr>)}</tbody></table></div>}
    <details><summary>请求前模型是否已加载</summary><ul>{p.byLoadState?.map(s => <li key={s.state}>{s.state === "loaded" ? "已加载" : s.state === "not_loaded" ? "未加载" : "状态未知"}：{s.n} 个完成样本，首字中位数 {duration(s.firstTextMs.p50)}</li>)}</ul><p>这是请求前的服务状态记录；不代表已经清空系统、磁盘或输入缓存。</p></details>
  </details>;
}

export function CaseMeasurements({ value, workspaceId, runId, stored }: { value?: Measurement; workspaceId: string; runId: string | null; stored?: { hardware: unknown; resources: unknown[] } }) {
  const [raw, setRaw] = useState<unknown>(), [error, setError] = useState(""), [busy, setBusy] = useState(false);
  if (!value) return null;
  const load = async () => {
    setBusy(true); setError("");
    try { const result = stored ?? await invoke<{ resources: unknown[]; hardware: unknown }>("get_benchmark_run", { workspaceId, experimentId: runId }); setRaw({ hardware: result.hardware, samples: result.resources }); }
    catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  return <details className="benchmark-measurements"><summary>本题耗时与资源</summary><dl className="benchmark-measurement-facts">
    <dt>首个可见文字</dt><dd>{duration(value.firstTextMs)}</dd><dt>首个工具响应</dt><dd>{duration(value.firstToolMs)}</dd>
    <dt>{value.completed ? "请求到响应结束" : "请求到停止或出错"}</dt><dd>{duration(value.responseMs)}</dd>
    <dt>模型加载 / 处理输入</dt><dd>{duration(value.loadMs)} / {duration(value.promptMs)}</dd>
    <dt>模型生成速度</dt><dd>{speed(value.tokensPerSecond)}</dd><dt>输入 / 输出 Token</dt><dd>{value.inputTokens ?? "不可用"} / {value.outputTokens ?? "不可用"}</dd>
    <dt>单题全程</dt><dd>{duration(value.logicalTiming?.totalMs)}</dd><dt>整机内存采样峰值</dt><dd>{bytes(value.resources.systemMemoryBytesPeak)}</dd>
    <dt>整卡显存采样峰值</dt><dd>{value.resources.gpus?.length ? value.resources.gpus.map(g => bytes(g.usedBytesPeak)).join(" / ") : "不可用"}</dd>
    <dt>有效 CPU / 内存读数</dt><dd>{value.resources.systemCpuSamples ?? 0} / {value.resources.systemMemorySamples ?? 0}</dd>
  </dl>{value.collection?.error && <p role="alert">资源采样未完整保存：{value.collection.error}</p>}
    {!!value.resources.gpuReasons?.length && <p>显卡读数不可用：{value.resources.gpuReasons.map(r => gpuReason[r] ?? r).join("；")}</p>}
    {runId && <button className="button text-button" disabled={busy} onClick={() => void load()}>{busy ? "正在读取…" : "查看原始采样记录"}</button>}
    {error && <p role="alert" className="cloud-error">{error}</p>}{raw !== undefined && <pre>{JSON.stringify(raw, null, 2)}</pre>}
  </details>;
}
const gpuReason: Record<string, string> = { nvidia_smi_not_installed: "未找到 NVIDIA 驱动读数工具", nvidia_smi_start_failed: "无法启动显卡读数工具", nvidia_smi_timeout: "读取显卡状态超时", nvidia_smi_failed: "显卡驱动未返回有效读数", nvidia_smi_no_devices: "未检测到可读取的 NVIDIA 显卡" };
