import { useEffect, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Cloud } from "lucide-react";
import { cloudPresetForAddress, cloudPresets } from "../cloud-presets";
import { emitCloudConnectionsChanged } from "../model-usage-preference";
import { errorText } from "../studio";
import { SelectMenu } from "./SelectMenu";
import { CloudKnowledgeSettings } from "./CloudKnowledgeSettings";
import { CloudIndexSettings } from "./CloudIndexSettings";
import { CloudPrivacySettings } from "./CloudPrivacySettings";

const providers = { openai: "OpenAI", anthropic: "Anthropic", gemini: "Google Gemini", compatible: "OpenAI 兼容服务" };
const protocols = { openai: "Responses / Embeddings", anthropic: "Messages", gemini: "Generate Content / Batch Embed Contents", compatible: "Chat Completions / Embeddings" };
const roles = { generation: "生成回答", embedding: "生成向量", reranking: "重排结果" };
const statuses: Record<string, string> = {
  unconfigured: "未读取", verified: "已验证", invalid_credential: "密钥无效", model_denied: "无权访问或接口不可用",
  quota_exceeded: "额度不足", rate_limited: "请求过于频繁", network_unavailable: "连接失败或已超时", service_error: "服务返回异常",
};
const declared: Record<string, string> = { listed: "服务声明支持", unknown: "服务未报告", unsupported: "当前接口不支持" };
const limitFields = {
  contextTokens: "总上下文上限（Token）", maxInputTokens: "输入上限（Token）", maxOutputTokens: "输出上限（Token）",
  inputMicroUsdPerMillion: "每百万输入 Token（美元）", outputMicroUsdPerMillion: "每百万输出 Token（美元）",
  microUsdPerRequest: "每次请求固定费用（美元）",
};
const manual = "!manual";
type Provider = keyof typeof providers;
type ProviderChoice = Provider | typeof cloudPresets[number]["id"];
type Capability = keyof typeof roles;
type LimitKey = keyof typeof limitFields;
type Limits = { model: string } & Record<LimitKey, number | null>;
type Model = { id: string; maxInputTokens: number | null; maxOutputTokens: number | null } & Record<Capability, string>;
type Probe = { model: string; capability: Capability; state: string; checkedAt: string; dimensions: number | null };
type Connection = {
  id: string; label: string; provider: Provider; address: string | null; compatibleRerank: boolean;
  catalogState: string; catalogCheckedAt: string | null; models: Model[]; probes: Probe[]; limits: Limits[];
  credentialAvailable?: boolean;
};
const time = (value: string) => new Date(value).toLocaleString("zh-CN", { hour12: false });
const stateLabel = (value: string) => statuses[value] ?? "状态未知";

export function CloudRuntimeSettings({ workspaceId }: { workspaceId: string }) {
  const [connections, setConnections] = useState<Connection[]>([]);
  const [selected, setSelected] = useState("");
  const [provider, setProvider] = useState<ProviderChoice>("openai");
  const [adding, setAdding] = useState(false);
  const [model, setModel] = useState("");
  const [manualModel, setManualModel] = useState("");
  const [pending, setPending] = useState<Capability | "remove">();
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [limitError, setLimitError] = useState<Partial<Record<LimitKey, string>>>({});
  const [notice, setNotice] = useState("");
  const connection = connections.find(item => item.id === selected);
  const newPreset = cloudPresets.find(item => item.id === provider);
  const connectionPreset = cloudPresetForAddress(connection?.address);
  const catalogFailed = Boolean(connection?.catalogCheckedAt && connection.catalogState !== "verified");
  const modelId = model === manual ? manualModel.trim() : model;
  const modelInfo = connection?.models.find(item => item.id === modelId);
  const saved = connection?.limits.find(item => item.model === modelId);

  useEffect(() => {
    let active = true;
    void invoke<Connection[]>("list_runtime_cloud_connections", { workspaceId })
      .then(rows => { if (active) { setConnections(rows); selectConnection(rows[0]); } })
      .catch(value => { if (active) setError(errorText(value)); })
      .finally(() => { if (active) setBusy(false); });
    return () => { active = false; };
  }, [workspaceId]);

  function selectConnection(row?: Connection) {
    setSelected(row?.id ?? "");
    setModel(cloudPresetForAddress(row?.address) && !row?.models.length ? manual : "");
    setManualModel("");
    setPending(undefined);
    setLimitError({});
    setNotice("");
    setError("");
  }

  async function run(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError("");
    setNotice("");
    setPending(undefined);
    try { await action(); }
    catch (value) { setError(errorText(value)); }
    finally { setBusy(false); }
  }

  function update(row: Connection) {
    setConnections(current => current.map(item => item.id === row.id ? { ...item, ...row } : item));
  }

  function bindKey(event: FormEvent<HTMLFormElement>, connection: Connection) {
    event.preventDefault();
    const form = event.currentTarget;
    const apiKey = new FormData(form).get("apiKey");
    void run(async () => {
      await invoke("bind_runtime_cloud_key", { workspaceId, connectionId: connection.id, apiKey });
      form.reset();
      update({ ...connection, credentialAvailable: true });
      emitCloudConnectionsChanged(workspaceId);
      setNotice("密钥已在本机绑定。可以刷新模型列表检查连接。");
    });
  }

  function add(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const fields = new FormData(form);
    void run(async () => {
      const row = await invoke<Connection>("add_runtime_cloud_connection", {
        workspaceId, input: {
          provider: newPreset ? "compatible" : provider,
          label: String(fields.get("label")).trim() || newPreset?.label || "",
          apiKey: fields.get("apiKey"),
          address: newPreset?.address ?? (provider === "compatible" ? String(fields.get("address")).trim() : null),
          compatibleRerank: newPreset?.reranking ?? fields.has("rerank"),
        },
      });
      setConnections(current => [...current, row]);
      selectConnection(row);
      form.reset();
      setProvider("openai");
      setAdding(false);
      emitCloudConnectionsChanged(workspaceId);
      setNotice("已保存连接。");
    });
  }

  function saveLimits(event: FormEvent<HTMLFormElement>, connectionId: string) {
    event.preventDefault();
    const form = event.currentTarget;
    const keys = Object.keys(limitFields) as LimitKey[];
    const errors: Partial<Record<LimitKey, string>> = {};
    const limits = Object.fromEntries(keys.map(field => {
      const input = form.elements.namedItem(field);
      const value = input instanceof HTMLInputElement && input.value !== "" ? input.valueAsNumber : null;
      if (value !== null && !Number.isFinite(value)) errors[field] = "请输入有效数字。";
      return [field, value === null ? null : Math.round(value * (field.endsWith("Tokens") ? 1 : 1_000_000))];
    })) as Record<LimitKey, number | null>;
    const context = limits.contextTokens;
    for (const field of ["maxInputTokens", "maxOutputTokens"] as const) {
      const value = limits[field];
      if (context !== null && value !== null && value > context) errors[field] = "此上限不能高于总上下文上限。";
    }
    setLimitError(errors);
    const first = keys.find(field => errors[field]);
    if (first) {
      const input = form.elements.namedItem(first);
      if (input instanceof HTMLInputElement) input.focus();
      return;
    }
    void run(async () => {
      update(await invoke<Connection>("set_runtime_cloud_model_limits", { workspaceId, connectionId, input: { model: modelId, ...limits } }));
      setNotice("已保存模型上限和费率。");
    });
  }

  return <>
    <div className="section-description"><h2 id="runtime-cloud-settings"><Cloud size={18} aria-hidden="true" />云端连接</h2></div>
    <div className="settings-fields">
      <CloudPrivacySettings workspaceId={workspaceId} />
      <div className="cloud-runtime-actions">
        {connection && <div className="field">
          <label htmlFor="runtime-cloud-connection">当前工作区的连接</label>
          <SelectMenu id="runtime-cloud-connection" value={selected} disabled={busy}
            onChange={event => selectConnection(connections.find(item => item.id === event.target.value))}>
            {connections.map(item => <option value={item.id} key={item.id}>{item.label}</option>)}
          </SelectMenu>
        </div>}
        {!busy && !connection && !adding && <p>当前工作区还没有云端连接。</p>}
        <button type="button" className="button secondary" disabled={busy} onClick={() => { setAdding(!adding); setPending(undefined); }}>
          {adding ? "取消添加" : "添加连接"}
        </button>
      </div>
      {adding && <form className="settings-fields" onSubmit={add}>
        <div className="parameter-fields">
          <div className="field"><label htmlFor="runtime-cloud-provider">服务商</label>
            <SelectMenu id="runtime-cloud-provider" value={provider} disabled={busy} onChange={event => setProvider(event.target.value as ProviderChoice)}>
              {cloudPresets.map(preset => <option key={preset.id} value={preset.id}>{preset.label}</option>)}
              {Object.entries(providers).map(([id, name]) => <option key={id} value={id}>{name}</option>)}
            </SelectMenu>
          </div>
          <div className="field"><label htmlFor="runtime-cloud-label">连接名称</label>
            <input id="runtime-cloud-label" name="label" placeholder={newPreset?.label} required={!newPreset}
              maxLength={80} disabled={busy} /></div>
        </div>
        {newPreset && <div className="field"><label htmlFor="runtime-cloud-preset-address">HTTPS 服务地址</label>
          <input id="runtime-cloud-preset-address" value={newPreset.address} readOnly /></div>}
        {provider === "compatible" && <>
          <div className="field"><label htmlFor="runtime-cloud-address">HTTPS 服务地址</label>
            <input id="runtime-cloud-address" name="address" type="url" placeholder="https://example.com/v1" required disabled={busy} /></div>
          <label><input name="rerank" type="checkbox" disabled={busy} /> 服务提供兼容的 /rerank 接口</label>
        </>}
        <div className="field"><label htmlFor="runtime-cloud-key">API 密钥</label>
          <input key={provider} id="runtime-cloud-key" name="apiKey" type="password" autoComplete="off" spellCheck={false} maxLength={1024} required disabled={busy} />
          {newPreset && <a href={newPreset.keyUrl} target="_blank" rel="noopener noreferrer">获取 API 密钥</a>}
        </div>
        <button className="button primary" disabled={busy}>保存连接</button>
      </form>}
      {connection && !adding && <>
        <div className="cloud-runtime-actions">
          <span>{connectionPreset?.label ?? providers[connection.provider]} {connection.address}</span>
          <button type="button" className="button text-button" disabled={busy} onClick={() => setPending("remove")}>移除连接</button>
        </div>
        {connection.credentialAvailable === false && <form className="settings-fields" onSubmit={event => bindKey(event, connection)}>
          <p>此连接需要在本机绑定密钥。</p>
          <div className="field"><label htmlFor="runtime-cloud-bind-key">API 密钥</label>
            <input key={connection.id} id="runtime-cloud-bind-key" name="apiKey" type="password" autoComplete="off"
              spellCheck={false} maxLength={1024} required disabled={busy} />
            {connectionPreset && <a href={connectionPreset.keyUrl} target="_blank" rel="noopener noreferrer">获取 API 密钥</a>}
          </div>
          <button className="button secondary" disabled={busy}>绑定密钥</button>
        </form>}
        {pending === "remove" && <div className="cloud-runtime-confirm">
          <p>移除「{connection.label}」及其保存在 Windows 中的密钥？</p>
          <button type="button" className="button secondary" onClick={() => void run(async () => {
            await invoke("remove_runtime_cloud_connection", { workspaceId, connectionId: connection.id });
            const remaining = connections.filter(item => item.id !== connection.id);
            setConnections(remaining);
            selectConnection(remaining[0]);
            emitCloudConnectionsChanged(workspaceId);
            setNotice("已移除连接。");
          })}>确认移除</button>
          <button type="button" className="button text-button" onClick={() => setPending(undefined)}>取消</button>
        </div>}
        <CloudKnowledgeSettings key={connection.id} workspaceId={workspaceId} connectionId={connection.id} />
        <div className="cloud-runtime-actions">
          <button type="button" className="button secondary" disabled={busy || connection.credentialAvailable === false} onClick={() => void run(async () => {
            const row = await invoke<Connection>("refresh_runtime_cloud_catalog", { workspaceId, connectionId: connection.id });
            update(row);
            emitCloudConnectionsChanged(workspaceId);
            if (model && model !== manual && !row.models.some(item => item.id === model)) { setManualModel(model); setModel(manual); }
          })}>刷新模型列表</button>
          <span>{connection.credentialAvailable === false ? "等待绑定密钥" : <>目录{connection.catalogState === "verified" ? "已读取" : stateLabel(connection.catalogState)}{connection.catalogCheckedAt && "，" + time(connection.catalogCheckedAt)}</>}</span>
        </div>
        <div className="field">
          <label htmlFor="runtime-cloud-model">模型</label>
          <SelectMenu id="runtime-cloud-model" value={model} disabled={busy}
            aria-describedby={catalogFailed ? "runtime-cloud-model-help" : undefined}
            onChange={event => { setModel(event.target.value); setPending(undefined); setLimitError({}); }}>
            <option value="">选择模型</option>
            {connection.models.map(item => <option value={item.id} key={item.id}>{item.id}</option>)}
            <option value={manual}>手动输入模型标识</option>
          </SelectMenu>
          {catalogFailed && <small id="runtime-cloud-model-help">
            目录读取失败时，可选择「手动输入模型标识」，再验证该模型的能力。
          </small>}
        </div>
        {model === manual && <div className="field"><label htmlFor="runtime-cloud-manual-model">模型标识</label>
          <input id="runtime-cloud-manual-model" value={manualModel} maxLength={200} disabled={busy}
            onChange={event => { setManualModel(event.target.value); setPending(undefined); setLimitError({}); }} /></div>}
        {modelId && <>
          {connection.probes.some(probe => probe.model === modelId && probe.capability === "embedding" && probe.state === "verified")
            && <CloudIndexSettings key={JSON.stringify(["index", connection.id, modelId])} workspaceId={workspaceId} connectionId={connection.id} model={modelId} />}
          <div className="cloud-runtime-facts">
            <span>接口 {connectionPreset?.embedding === false ? "Chat Completions" : protocols[connection.provider]}{connection.compatibleRerank && " / Rerank"}</span>
            {modelInfo && <span>服务报告上限　输入 {modelInfo.maxInputTokens?.toLocaleString() ?? "未知"} / 输出 {modelInfo.maxOutputTokens?.toLocaleString() ?? "未知"}</span>}
          </div>
          <ul className="cloud-runtime-capabilities">
            {(Object.keys(roles) as Capability[]).map(role => {
              const probe = connection.probes.find(item => item.model === modelId && item.capability === role);
              const unavailable = modelInfo?.[role] === "unsupported"
                || (role === "embedding" && (connection.provider === "anthropic" || connectionPreset?.embedding === false))
                || (role === "reranking" && !connection.compatibleRerank);
              return <li key={role}>
                <span><strong>{roles[role]}</strong> {unavailable ? declared.unsupported : declared[modelInfo?.[role] ?? "unknown"]}
                  {probe && <><br /><time dateTime={probe.checkedAt}>{stateLabel(probe.state)}，{time(probe.checkedAt)}</time>
                    {probe.dimensions != null && "，" + probe.dimensions + " 维"}</>}
                </span>
                {!unavailable && <button type="button" className="button text-button" disabled={busy || connection.credentialAvailable === false}
                  onClick={() => setPending(role)} aria-label={"验证" + roles[role]}>验证</button>}
              </li>;
            })}
          </ul>
          {pending && pending !== "remove" && <div className="cloud-runtime-confirm">
            <p>用固定测试文本验证「{roles[pending]}」。服务商可能收费。</p>
            <button type="button" className="button secondary" disabled={busy} onClick={() => void run(async () => {
              update(await invoke<Connection>("probe_runtime_cloud_model", { workspaceId, connectionId: connection.id, model: modelId, capability: pending }));
              emitCloudConnectionsChanged(workspaceId);
            })}>开始验证</button>
            <button type="button" className="button text-button" onClick={() => setPending(undefined)}>取消</button>
          </div>}
          <details className="advanced-settings" key={JSON.stringify(["limits", selected, modelId])}>
            <summary>模型上限与费率</summary>
            <form className="settings-fields" onSubmit={event => saveLimits(event, connection.id)}>
              <div className="parameter-fields">
                {(Object.keys(limitFields) as LimitKey[]).map(key => {
                  const tokens = key.endsWith("Tokens");
                  const value = saved?.[key];
                  return <div className="field" key={key}><label htmlFor={key}>{limitFields[key]}</label>
                    <input id={key} name={key} type="number" min={tokens ? 1 : 0} max={tokens ? 10_000_000 : 1_000_000}
                      step={tokens ? 1 : 0.000001} disabled={busy} placeholder="未知" defaultValue={value == null ? "" : value / (tokens ? 1 : 1_000_000)}
                      aria-invalid={!!limitError[key] || undefined} aria-describedby={limitError[key] ? `${key}-error` : undefined}
                      onInput={() => setLimitError({})} onInvalid={() => setLimitError(current => ({ ...current, [key]: tokens ? "请输入 1 至 10000000 之间的整数，或留空。" : "请输入 0 至 1000000 之间的金额，最多六位小数，或留空。" }))} />
                    {limitError[key] && <small id={`${key}-error`} className="form-error">{limitError[key]}</small>}
                  </div>;
                })}
              </div>
              <button className="button secondary" disabled={busy}>保存模型参数</button>
            </form>
          </details>
        </>}
      </>}
      {busy && <p role="status">正在处理云端连接…</p>}
      {notice && <p role="status">{notice}</p>}
      {error && <p className="form-error" role="alert">{error}</p>}
    </div>
  </>;
}
