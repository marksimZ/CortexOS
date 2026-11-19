import { useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "../studio";
import { SelectMenu } from "./SelectMenu";

type Preferences = { device: "auto" | "cpu"; keepAliveSeconds: number };
type LoadedModel = { name: string; digest: string | null; sizeBytes: number | null; sizeVramBytes: number | null; contextLength: number | null };
type Observation = {
  catalog: { loaded: LoadedModel[] | null };
  inspection: { model: string; local?: boolean; generation?: boolean | null; embedding?: boolean | null; contextCapacity?: number | null; error?: string } | null;
};
const capabilities = (value?: boolean | null) => value == null ? "未报告" : value ? "支持" : "不支持";
const size = (bytes: number) => `${(bytes / 1024 ** 3).toFixed(2)} GiB`;

export function LocalRuntimeSettings({ workspaceId, model, running }: { workspaceId: string; model: string; running: boolean }) {
  const id = useId();
  const [saved, setSaved] = useState<Preferences>();
  const [draft, setDraft] = useState<Preferences>();
  const [observed, setObserved] = useState<Observation>();
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [confirm, setConfirm] = useState<string>();
  const inspection = observed?.inspection?.model === model ? observed.inspection : null;

  async function run(action: () => Promise<void>) {
    if (loading) return;
    setLoading(true);
    setMessage("");
    setError("");
    try { await action(); }
    catch (failure) { setError(errorText(failure)); }
    finally { setLoading(false); }
  }

  async function readPreferences() {
    const value = await invoke<Preferences>("local_model_preferences", { input: null });
    setSaved(value);
    setDraft(value);
  }

  async function refresh() {
    setConfirm(undefined);
    setObserved(await invoke<Observation>("inspect_local_model_runtime", { model }));
  }

  return <details className="advanced-settings" onToggle={event => {
    if (event.currentTarget.open && !draft) void run(readPreferences);
  }}>
    <summary>本机运行与资源</summary>
    {draft ? <>
      <div className="parameter-fields">
        <div className="field">
          <label htmlFor={`${id}-device`}>对话计算设备</label>
          <SelectMenu id={`${id}-device`} value={draft.device} disabled={loading}
            onChange={event => setDraft({ ...draft, device: event.target.value as Preferences["device"] })}>
            <option value="auto">自动分配</option><option value="cpu">仅用 CPU</option>
          </SelectMenu>
        </div>
        <div className="field">
          <label htmlFor={`${id}-keep`}>回答后保留模型（秒）</label>
          <input id={`${id}-keep`} type="number" min={0} max={300} step={1} value={draft.keepAliveSeconds}
            disabled={loading} onChange={event => setDraft({ ...draft, keepAliveSeconds: Number(event.target.value) })} />
        </div>
      </div>
      <button type="button" className="button secondary" disabled={loading || JSON.stringify(draft) === JSON.stringify(saved)
        || !Number.isInteger(draft.keepAliveSeconds) || draft.keepAliveSeconds < 0 || draft.keepAliveSeconds > 300}
        onClick={() => void run(async () => {
          const value = await invoke<Preferences>("local_model_preferences", { input: draft });
          setSaved(value); setDraft(value); setMessage("下次对话将使用新的运行设置。");
        })}>保存本机运行设置</button>
    </> : <button type="button" className="button secondary" disabled={loading} onClick={() => void run(readPreferences)}>读取本机运行设置</button>}
    <button type="button" className="button text-button" disabled={loading} onClick={() => void run(refresh)}>刷新模型状态</button>
    {loading && <p role="status">正在处理本机模型设置…</p>}
    {message && <p role="status">{message}</p>}
    {error && <p className="form-error" role="alert">{error}</p>}
    {inspection && (inspection.error ? <p>{inspection.error}</p> : <p>
      {inspection.model}，生成{capabilities(inspection.generation)}，向量{capabilities(inspection.embedding)}
      {inspection.contextCapacity != null && `，总容量 ${inspection.contextCapacity.toLocaleString()} Token`}{inspection.local === false && "，来源未通过本机检查"}
    </p>)}
    {observed && (observed.catalog.loaded == null ? <p>Ollama 未报告已加载模型。</p> : observed.catalog.loaded.length === 0
      ? <p>当前没有已加载模型。</p> : <ul className="personal-list">
        {observed.catalog.loaded.map(item => <li key={item.name}>
          <div><strong>{item.name}</strong>
            <p>{item.sizeBytes == null ? "占用未报告" : `已加载 ${size(item.sizeBytes)}`}
              {item.sizeVramBytes != null && `，显存 ${size(item.sizeVramBytes)}`}
              {item.sizeVramBytes === 0 && "，使用系统内存"}
              {item.contextLength != null && `，当前容量 ${item.contextLength.toLocaleString()} Token`}</p>
          </div>
          <button type="button" className="button secondary" disabled={loading || running || !item.digest}
            aria-label={`${confirm === item.name ? "确认卸载" : "卸载"} ${item.name}`}
            onClick={() => {
              if (confirm !== item.name) { setConfirm(item.name); return; }
              void run(async () => {
                setConfirm(undefined);
                const result = await invoke<{ status: string }>("release_local_runtime_model", { workspaceId, model: item.name, digest: item.digest });
                await refresh();
                setMessage(result.status === "released" ? `已卸载 ${item.name}。` : `${item.name} 已处于未加载状态。`);
              });
            }}>{confirm === item.name ? "确认卸载" : "卸载模型"}</button>
          {confirm === item.name && <button type="button" className="button text-button" onClick={() => setConfirm(undefined)}>取消</button>}
        </li>)}
      </ul>)}
  </details>;
}
