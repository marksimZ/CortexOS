import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "../studio";

type Policy = {
  singleRunMicros: number | null;
  rollingDayMicros: number | null;
  requestsPerMinute: number | null;
  retryLimit: number;
};
const fields = [
  { name: "singleRunMicros", label: "单次任务费用上限（美元）", scale: 1_000_000, max: 1_000_000, step: 0.000001 },
  { name: "rollingDayMicros", label: "过去 24 小时费用上限（美元）", scale: 1_000_000, max: 1_000_000, step: 0.000001 },
  { name: "requestsPerMinute", label: "每个连接每分钟请求上限", scale: 1, max: 1000, step: 1 },
] as const;

export function RuntimeBudgetSettings({ workspaceId }: { workspaceId: string }) {
  const [policy, setPolicy] = useState<Policy>();
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");

  async function save(input?: Policy) {
    setBusy(true);
    setNotice("");
    setError("");
    try {
      const result = await invoke<Policy>("model_runtime_policy", { workspaceId, input: input ?? null });
      setPolicy(result);
      if (input) setNotice("已保存请求限制。");
    } catch (failure) { setError(errorText(failure)); }
    finally { setBusy(false); }
  }

  return <details className="advanced-settings" onToggle={event => {
    if (event.currentTarget.open && !policy && !busy) void save();
  }}>
    <summary>云端预算与请求限制</summary>
    {policy && <form className="settings-fields" onSubmit={event => {
      event.preventDefault();
      const data = new FormData(event.currentTarget);
      const limits = fields.map(({ name, scale }) => {
        const value = String(data.get(name) ?? "");
        return value ? Math.round(Number(value) * scale) : null;
      });
      void save({
        singleRunMicros: limits[0], rollingDayMicros: limits[1], requestsPerMinute: limits[2],
        retryLimit: Number(data.get("retryLimit") ?? 0),
      });
    }}>
      <div className="parameter-fields">
        {fields.map(({ name, label, scale, max, step }) => <div className="field" key={name}>
          <label htmlFor={name}>{label}</label>
          <input id={name} name={name} type="number" min={name === "requestsPerMinute" ? 1 : 0} max={max} step={step}
            disabled={busy} placeholder="不限" defaultValue={policy[name] == null ? "" : policy[name] / scale} />
        </div>)}
        <div className="field">
          <label htmlFor="runtime-retry-limit">云端限流后重试次数</label>
          <input id="runtime-retry-limit" name="retryLimit" type="number" min={0} max={2} step={1} defaultValue={policy.retryLimit} required disabled={busy} />
        </div>
      </div>
      <p>填写费率后可启用预算。请求前按登记费率估算，服务返回用量后更新费用估算。</p>
      <button type="submit" className="button secondary" disabled={busy}>保存请求限制</button>
    </form>}
    {!policy && !busy && error && <button type="button" className="button text-button" onClick={() => void save()}>重新读取</button>}
    {busy && <p role="status">正在读取或保存请求限制…</p>}
    {notice && <p role="status">{notice}</p>}
    {error && <p className="form-error" role="alert">{error}</p>}
  </details>;
}
