import { invoke } from "@tauri-apps/api/core";
import { lazy, Suspense, useEffect, useId, useRef, useState } from "react";
import { AppDialog, openDialog } from "../components/AppDialog";
import { MarkdownContent } from "../components/MarkdownContent";
import { SelectMenu } from "../components/SelectMenu";
import { runtimeModes, runtimeRoles, type Profile, type Profiles } from "../runtime-profiles";
import { errorText, type ModelSettings, type RunDetail } from "../studio";
import type { Policy } from "./RagEvaluation";

const Trace = lazy(() => import("./RagRuntimeTrace"));
const policies: Record<Policy, string> = {
  off: "关闭查找", fts5: "按词语查找", hybrid: "词语与语义",
  hybrid_rerank: "混合查找并重排", graph: "词语与知识关系", version_aware: "按题目指定的日期",
};
const statuses: Record<string, string> = {
  running: "运行中", completed: "已完成", failed: "有运行错误", cancelled: "已停止",
  interrupted: "已中断", not_run: "未运行", execution_error: "执行出错", scoring_error: "评分出错",
};
const sideLabel = (i: number) => i === 0 ? "A" : "B";
const currency = (micros: number | null) => micros === null ? "—" : `US$ ${(micros / 1e6).toFixed(6)}`;
const format = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 });
const percent = new Intl.NumberFormat("zh-CN", { style: "percent", maximumFractionDigits: 1 });

type ArmInput = { profileId: string | null; policy: Policy };
type Input = { arms: ArmInput[]; caseId: string | null; maxCostMicros: number | null };
type Cloud = { capability: keyof typeof runtimeRoles; provider: string; model: string; destination: string };
type Arm = { profile: Profile; strategy: Policy; settings: ModelSettings; cloud: Cloud[]; localGenerationDigest: string | null; localEmbedding: { model: string; digest: string } | null };
type Plan = {
  arms: Arm[]; caseIds: string[]; containsCloudCalls: boolean; costCapMicros: number | null; usesLocalModels: boolean;
  dataset: { version: number; snapshotHash: string }; scorer: string;
  localGenerationPreferences?: { device: "cpu" | "auto"; keepAliveSeconds: number };
};
type Preview = { id: string; planHash: string; plan: Plan };
type Case = {
  id: string; question: string; status: string; error?: string; answer?: string; execution?: RunDetail;
  effectiveDate?: string; responseScoring?: {
    facts: { expected: string; matched: boolean }[];
    citations: { location: { label: string; error?: string | null }; supportsExpectedFact: boolean }[];
  };
  route?: { fallbackReason?: string | null };
};
type Result = {
  arm: Arm; cases: Case[]; setupError?: string; indexBuildMs?: number | null;
  summary?: Record<string, { value: number | null; cases: number }>;
  usage: {
    cloudCalls: number; settledMicros: number | null; unknownChargeCalls: number; unsettledReservedMicros: number | null;
    knownCloudInputTokens: number | null; knownCloudOutputTokens: number | null;
    localInputTokens: number | null; localOutputTokens: number | null; localSeconds: number | null;
    systemCpuPeak: number | null; gpuUsedPeakBytes: number | null;
  };
};
type History = { id: string; createdAt: string; status: string };
type Report = History & { plan: Plan; results: Result[] };
type Catalog = { cases: { id: string; question: string }[]; history: History[]; report: Report | null };

export function RagRuntimeEvaluation({ workspaceId }: { workspaceId: string }) {
  const id = useId();
  const confirm = useRef<HTMLDialogElement>(null);
  const [arms, setArms] = useState<ArmInput[]>([{ profileId: null, policy: "fts5" }, { profileId: null, policy: "graph" }]);
  const [caseId, setCaseId] = useState("");
  const [budget, setBudget] = useState("");
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [catalog, setCatalog] = useState<Catalog>();
  const [preview, setPreview] = useState<(Preview & { input: Input }) | null>(null);
  const [trace, setTrace] = useState<Case | null>(null);
  const [active, setActive] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const report = catalog?.report;
  const cloud = arms.some(arm => {
    const profile = profiles.find(item => item.id === arm.profileId);
    return profile && (profile.generation.kind === "cloud"
      || (arm.policy === "hybrid" || arm.policy === "hybrid_rerank") && profile.embedding.kind === "cloud"
      || arm.policy === "hybrid_rerank" && profile.reranking.kind === "cloud");
  });
  useEffect(() => {
    let current = true;
    void Promise.all([
      invoke<Catalog>("get_rag_runtime_experiment", { workspaceId }),
      invoke<Profiles>("list_model_runtime_profiles", { workspaceId }),
    ]).then(([data, options]) => {
      if (current) { setCatalog(data); setProfiles(options.profiles); }
    }).catch(value => { if (current) setError(errorText(value)); });
    return () => { current = false; };
  }, [workspaceId]);
  useEffect(() => {
    if (preview) openDialog(confirm.current);
  }, [preview]);
  const running = active ?? (report?.status === "running" ? report.id : null);
  useEffect(() => {
    if (!running) return;
    let current = true;
    let timer: number;
    async function poll() {
      try {
        const next = await invoke<Catalog>("get_rag_runtime_experiment", { workspaceId, id: running });
        if (!current) return;
        setCatalog(previous => previous?.report?.id === next.report?.id && previous?.report?.status !== "running" && next.report?.status === "running" ? previous : next);
        if (next.report?.id === running && next.report.status !== "running") return;
      } catch { /* The run may still be checking its plan before creating a row. */ }
      if (current) timer = window.setTimeout(poll, 1800);
    }
    timer = window.setTimeout(poll, 1800);
    return () => { current = false; window.clearTimeout(timer); };
  }, [workspaceId, running]);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(""), 2500);
    return () => window.clearTimeout(timer);
  }, [notice]);
  const locked = busy || !!running;

  function edit(i: number, change: Partial<ArmInput>) {
    setArms(all => all.map((arm, index) => index === i ? { ...arm, ...change } : arm));
  }
  async function read(selected?: string) {
    setBusy(true);
    setError("");
    try { setCatalog(await invoke<Catalog>("get_rag_runtime_experiment", { workspaceId, id: selected })); }
    catch (value) { setError(errorText(value)); }
    finally { setBusy(false); }
  }
  async function prepare() {
    const maxCostMicros = cloud ? Math.round(Number(budget) * 1e6) : null;
    if (cloud && (maxCostMicros === null || !Number.isSafeInteger(maxCostMicros) || maxCostMicros < 1)) {
      setError("请填写正数费用上限，精确到小数点后六位。");
      return;
    }
    const input = { arms, caseId: caseId || null, maxCostMicros };
    setBusy(true);
    setError("");
    try {
      const value = await invoke<Preview>("preview_rag_runtime_experiment", { workspaceId, input });
      setPreview({ ...value, input });
    } catch (value) { setError(errorText(value)); }
    finally { setBusy(false); }
  }
  async function run(pending: Preview & { input: Input }) {
    confirm.current?.close();
    setActive(pending.id);
    setError("");
    try {
      await invoke<Report>("run_rag_runtime_experiment", {
        workspaceId, id: pending.id, input: pending.input, planHash: pending.planHash,
      });
      setCatalog(await invoke<Catalog>("get_rag_runtime_experiment", { workspaceId, id: pending.id }));
    } catch (value) { setError(errorText(value)); }
    finally { setActive(null); setStopping(false); }
  }
  async function stop() {
    setStopping(true);
    try { await invoke("cancel_rag_runtime_experiment", { workspaceId, id: running }); }
    catch (value) { setError(errorText(value)); setStopping(false); }
  }
  async function exportReport() {
    setBusy(true);
    setError("");
    try {
      if (await invoke<boolean>("export_rag_runtime_experiment", { workspaceId, id: report?.id })) setNotice("报告已保存");
    } catch (value) { setError(errorText(value)); }
    finally { setBusy(false); }
  }
  return <div className="personal-workbench rag-evaluation rag-runtime">
    <div className="rag-evaluation-pair">
      {arms.map((arm, i) => <fieldset key={i} disabled={locked} className="rag-runtime-arm">
        <legend>方案 {sideLabel(i)}</legend>
        <label htmlFor={`${id}-profile-${i}`}>运行方案</label>
        <SelectMenu id={`${id}-profile-${i}`} value={arm.profileId ?? ""} disabled={locked} onChange={e => edit(i, { profileId: e.target.value || null })}>
          <option value="">默认本地模型</option>
          {profiles.map(profile => <option key={profile.id} value={profile.id}>{profile.name}（{runtimeModes[profile.mode]}）</option>)}
        </SelectMenu>
        <label htmlFor={`${id}-policy-${i}`}>查找策略</label>
        <SelectMenu id={`${id}-policy-${i}`} value={arm.policy} disabled={locked} onChange={e => edit(i, { policy: e.target.value as Policy })}>
          {Object.entries(policies).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
        </SelectMenu>
      </fieldset>)}
    </div>
    <div className="rag-evaluation-controls">
      <label htmlFor={`${id}-cases`}>题目</label>
      <SelectMenu id={`${id}-cases`} value={caseId} disabled={locked} onChange={e => setCaseId(e.target.value)}>
        <option value="">全部固定题目</option>
        {catalog?.cases.map(item => <option key={item.id} value={item.id}>{item.id} {item.question}</option>)}
      </SelectMenu>
      {cloud && <label className="rag-runtime-budget">费用上限（美元）<input inputMode="decimal" type="number" min="0.000001" step="0.000001" value={budget} disabled={locked} onChange={e => setBudget(e.target.value)} /></label>}
      {running ? <button className="button secondary" type="button" onClick={() => void stop()} disabled={stopping}>{stopping ? "正在停止…" : "停止对照"}</button>
        : <button className="button primary" type="button" onClick={() => void prepare()} disabled={!catalog || locked}>查看运行计划</button>}
    </div>
    {running && <p role="status">正在运行模型组合对照…</p>}
    {error && <p className="form-error" role="alert">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    {!!catalog?.history.length && <div className="rag-evaluation-controls">
      <label htmlFor={`${id}-history`}>历史对照</label>
      <SelectMenu id={`${id}-history`} value={report?.id ?? ""} disabled={locked} onChange={e => void read(e.target.value)}>
        {catalog.history.map(item => <option key={item.id} value={item.id}>{new Date(item.createdAt).toLocaleString("zh-CN")}（{statuses[item.status] ?? item.status}）</option>)}
      </SelectMenu>
      <button className="button secondary" type="button" disabled={!report || locked} onClick={() => void exportReport()}>导出完整报告</button>
    </div>}
    {report && <ReportView report={report} onTrace={setTrace} />}
    {preview && <AppDialog ref={confirm} className="form-dialog rag-runtime-confirm" aria-labelledby={`${id}-confirm`} onClose={() => setPreview(null)}>
      <h2 id={`${id}-confirm`}>确认运行对照</h2>
      <p>固定题库 v{preview.plan.dataset.version}，每组 {preview.plan.caseIds.length} 题，按 A、B 顺序运行。</p>
      {preview.plan.usesLocalModels && <p>运行会加载下列本地模型并占用内存或显存。</p>}
      {preview.plan.usesLocalModels && preview.plan.localGenerationPreferences && <p>本地回答使用{preview.plan.localGenerationPreferences.device === "cpu" ? " CPU" : "自动设备选择"}，回答后保留 {preview.plan.localGenerationPreferences.keepAliveSeconds} 秒。本地向量使用 CPU。</p>}
      {preview.plan.containsCloudCalls && <p>云端环节仅接收内置题目和合成资料，可能计费。两组的费用预留总额上限为 {currency(preview.plan.costCapMicros)}，同时受工作区预算限制。</p>}
      <div className="rag-evaluation-pair">{preview.plan.arms.map((arm, i) => <section key={i}><h3>{sideLabel(i)} {arm.profile.name}</h3><Targets arm={arm} /></section>)}</div>
      <div className="dialog-actions">
        <button className="button secondary" type="button" onClick={() => confirm.current?.close()}>取消</button>
        <button className="button primary" type="button" onClick={() => void run(preview)}>确认并运行</button>
      </div>
    </AppDialog>}
    {trace?.execution && <Suspense fallback={null}><Trace execution={trace.execution} answer={trace.answer ?? ""} onClose={() => setTrace(null)} /></Suspense>}
  </div>;
}

function Targets({ arm }: { arm: Arm }) {
  return <>
    <p>{runtimeModes[arm.profile.mode]}，{policies[arm.strategy]}</p>
    <dl className="rag-evaluation-identity">{Object.entries(runtimeRoles).map(([key, label]) => {
      const role = key as keyof typeof runtimeRoles;
      const target = arm.profile[role];
      const cloud = arm.cloud.find(item => item.capability === role);
      return <div className="rag-runtime-target" key={key}><dt>{label}</dt><dd>{target.kind === "off" ? "未启用"
        : target.kind === "local" ? `${target.model ?? "内置重排"}（本地）` : `${target.model}（${cloud?.destination ?? "云端"}）`}</dd></div>;
    })}</dl>
    <p>{arm.settings.numCtx} 上下文 / {arm.settings.numPredict} 输出 Token，超时 {arm.settings.timeoutSeconds} 秒</p>
  </>;
}

function ReportView({ report, onTrace }: { report: Report; onTrace: (item: Case) => void }) {
  const metrics: Record<string, string> = {
    recallAt5: "前 5 名召回率", mrr: "首个命中排名（MRR@5）", ndcg: "排序质量（nDCG@5）",
    contextRecall: "上下文召回率", answerQuality: "答案要点命中率", citationPrecision: "引用准确率",
    citationRecall: "引用召回率", retrievalLatencyMs: "平均查找时间", endToEndMs: "平均单题用时",
  };
  return <section aria-label="模型组合结果">
    <h2>对照结果</h2>
    <p>题库 v{report.plan.dataset.version}，{report.plan.caseIds.length} 题。回答按固定要点和有依据的引用评分。</p>
    <div className="rag-evaluation-table" role="region" tabIndex={0} aria-label="模型组合指标">
      <table><thead><tr><th scope="col">指标</th>{report.results.map((row, i) => <th scope="col" key={i}>{sideLabel(i)} {row.arm.profile.name}</th>)}</tr></thead>
        <tbody>
          <tr><th scope="row">完成题数</th>{report.results.map((row, i) => <td key={i}>{row.cases.filter(item => item.status === "completed").length} / {row.cases.length}</td>)}</tr>
          {Object.entries(metrics).map(([key, label]) => <tr key={key}><th scope="row">{label}</th>{report.results.map((row, i) => {
            const measure = row.summary?.[key];
            return <td className="numeric" key={i}>{measure?.value == null ? "—" : `${key.endsWith("Ms") ? `${format.format(measure.value)} 毫秒` : percent.format(measure.value)}（${measure.cases} 题）`}</td>;
          })}</tr>)}
          <tr><th scope="row">建索引用时</th>{report.results.map((row, i) => <td key={i}>{row.indexBuildMs == null ? "—" : `${row.indexBuildMs} 毫秒`}</td>)}</tr>
          <tr><th scope="row">云端输入 / 输出 Token</th>{report.results.map((row, i) => <td key={i}>{row.usage.knownCloudInputTokens ?? "—"} / {row.usage.knownCloudOutputTokens ?? "—"}</td>)}</tr>
          <tr><th scope="row">本地输入 / 输出 Token</th>{report.results.map((row, i) => <td key={i}>{row.usage.localInputTokens ?? "—"} / {row.usage.localOutputTokens ?? "—"}</td>)}</tr>
          <tr><th scope="row">本地模型请求用时合计</th>{report.results.map((row, i) => <td key={i}>{row.usage.localSeconds == null ? "—" : `${format.format(row.usage.localSeconds)} 秒`}</td>)}</tr>
          <tr><th scope="row">云端调用 / 已计算费用</th>{report.results.map((row, i) => <td key={i}>{row.usage.cloudCalls} / {currency(row.usage.settledMicros)}{row.usage.unknownChargeCalls > 0 && <div>{row.usage.unknownChargeCalls} 次费用待确认，预留 {currency(row.usage.unsettledReservedMicros)}</div>}</td>)}</tr>
          <tr><th scope="row">整机 CPU / GPU 显存采样</th>{report.results.map((row, i) => <td key={i}>{row.usage.systemCpuPeak == null ? "—" : `${format.format(row.usage.systemCpuPeak)}%`} / {row.usage.gpuUsedPeakBytes == null ? "—" : `${format.format(row.usage.gpuUsedPeakBytes / 2 ** 30)} GiB`}</td>)}</tr>
        </tbody>
      </table>
    </div>
    <div className="rag-evaluation-pair">{report.results.map((row, i) => <details className="knowledge-disclosure" key={i}>
      <summary>方案 {sideLabel(i)} 配置</summary><Targets arm={row.arm} />{row.setupError && <p className="form-error">{row.setupError}</p>}
    </details>)}</div>
    {report.results[0]?.cases.map((item, position) => <details className="knowledge-disclosure" key={item.id}>
      <summary>{item.id} {item.question}</summary>
      <div className="rag-evaluation-pair">{report.results.map((row, i) => {
        const result = row.cases[position];
        return <section key={i}><h3>{sideLabel(i)} {statuses[result.status] ?? result.status}</h3>
          {result.error && <p className="form-error">{result.error}</p>}
          {result.route?.fallbackReason && <p>查找采用了备用策略，原计划与实际策略见执行记录。</p>}
          {result.answer && <MarkdownContent content={result.answer} />}
          {result.responseScoring && <details>
            <summary>答案要点与引用评分</summary>
            <ul>{result.responseScoring.facts.map(fact => <li key={fact.expected}>{fact.matched ? "命中" : "未命中"}　{fact.expected}</li>)}</ul>
            {result.responseScoring.citations.length === 0 ? <p>未找到引用标记。</p> : <ul>
              {result.responseScoring.citations.map(({ location, supportsExpectedFact }, index) => <li key={index}>
                {location.label}　{location.error ?? (supportsExpectedFact ? "原文支持该要点" : "该位置未支持本题预期要点")}
              </li>)}
            </ul>}
          </details>}
          {result.execution && <button className="button secondary" type="button" onClick={() => onTrace(result)}>查看 {sideLabel(i)} 的 3D 执行记录</button>}
        </section>;
      })}</div>
    </details>)}
  </section>;
}
