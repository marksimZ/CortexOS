import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { KeyRound, ShieldCheck } from "lucide-react";
import { SelectMenu } from "../components/SelectMenu";
import { notify } from "../components/Toast";
import { type Connection, type Provider, providers, statuses } from "./cloud";

export function CloudConnection({ desktop, onContinue }: { desktop: boolean; onContinue?: () => void }) {
  const [connection, setConnection] = useState<Connection | null>(null);
  const [provider, setProvider] = useState<Provider>("openai");
  const [key, setKey] = useState("");
  const [model, setModel] = useState("");
  const [models, setModels] = useState<Connection["models"]>([]);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(desktop);
  const [error, setError] = useState<string | null>(null);
  const [entered, setEntered] = useState(false);
  const mounted = useRef(true);
  const working = useRef(false);
  const cancelled = useRef(false);
  const apply = (next: Connection) => {
    setConnection(next);
    if (next.provider) setProvider(next.provider);
    setModel(next.model ?? "");
    setModels(next.models);
    setEntered(false);
  };
  useEffect(() => {
    mounted.current = true;
    if (desktop) void invoke<Connection>("cloud_connection").then(next => {
      if (mounted.current) apply(next);
    }).catch(() => { if (mounted.current) setError("无法读取系统连接状态，请重试或检查 Windows 凭据存储。"); })
      .finally(() => { if (mounted.current) setLoading(false); });
    return () => {
      mounted.current = false;
      cancelled.current = true;
      if (working.current) void invoke("cancel_cloud_verification").catch(() => {});
    };
  }, [desktop]);
  const run = async (operation: () => Promise<void>) => {
    working.current = true; setBusy(true); setError(null);
    try { await operation(); }
    catch { if (mounted.current) { setEntered(false); setError("操作未完成，请重新验证连接；如仍失败，请检查系统凭据存储。"); } }
    finally { working.current = false; if (mounted.current) setBusy(false); }
  };
  const verify = () => {
    cancelled.current = false;
    // No localStorage, analytics, logging, error echo or retained request object.
    const request = invoke<Connection>("verify_cloud_connection", { input: { provider, apiKey: key || null, model: key ? null : model || null } });
    setKey(""); setEntered(false);
    void run(async () => { const next = await request; if (mounted.current && !cancelled.current) { apply(next); if (next.allowed) notify("连接已验证"); } });
  };
  const saved = connection?.provider === provider && connection.state !== "unconfigured";
  const changed = !!key || provider !== connection?.provider || model !== (connection?.model ?? "");
  return <div className="cloud-onboarding">
    <div className="cloud-intro">
      <KeyRound size={28} aria-hidden="true" />
      <h2>为你的任务设计评测</h2>
      <p>先连接你自己的云端账号。之后可按工作需求和项目资料设计专属题库。</p>
      <a className="button text-button" href="#/benchmark">返回普通评测</a>
    </div>
    <div className="cloud-setup">
      {loading ? <p role="status">正在读取连接状态…</p> : <>
        {!desktop && <p role="status">请在桌面应用中配置连接。浏览器预览不会保存密钥。</p>}
        <form onSubmit={event => { event.preventDefault(); verify(); }}>
          <fieldset disabled={!desktop || busy}>
            <legend className="sr-only">云端账号连接</legend>
            <label>服务商<SelectMenu aria-label="服务商" value={provider} onChange={event => {
              setProvider(event.target.value as Provider); setKey(""); setModels([]); setModel(""); setEntered(false); setError(null);
            }}>{Object.entries(providers).map(([id, label]) => <option key={id} value={id}>{label}</option>)}</SelectMenu></label>
            <label>API Key<input type="password" value={key} maxLength={1024} autoComplete="off" spellCheck={false} autoCapitalize="none" placeholder={saved ? "已安全保存；输入新密钥可替换" : "输入服务商提供的 API Key"} onChange={event => { setKey(event.target.value); setEntered(false); }} /></label>
            <label>题目生成模型<SelectMenu aria-label="题目生成模型" value={model} disabled={!models.length || !!key} onChange={event => { setModel(event.target.value); setEntered(false); }}>
              <option value="">{models.length ? "请选择目录中的模型" : connection?.model ?? "验证密钥后读取模型目录"}</option>
              {models.map(item => <option key={item.id} value={item.id}>{item.id}</option>)}
              {model && !models.some(m => m.id === model) && <option value={model}>{model}</option>}
            </SelectMenu></label>
            <div className="cloud-notice"><ShieldCheck size={18} aria-hidden="true" /><div><p>密钥由 Windows 凭据管理器保存在本机。连接不会上传项目、知识或对话。</p><p>本次只查询模型目录，不发送推理请求。后续生成会使用你的 API 额度，启动前需另行确认费用和发送内容。</p></div></div>
            <button className="button primary" type="submit" disabled={!key && !saved}>{busy ? "正在验证…" : models.length && model ? "验证模型并连接" : saved ? "重新验证 / 读取模型" : "验证密钥并读取模型"}</button>
          </fieldset>
        </form>
        <p className="cloud-connection-state" role="status" aria-live="polite">{busy ? "正在验证连接…" : statuses[connection?.state ?? "unconfigured"] ?? "连接状态未知，请重新验证"}</p>
        {error && <p role="alert" className="cloud-error">{error}</p>}
        {connection?.allowed && <p>目录访问已验证。模型的生成能力、推理权限和剩余额度仍需在实际请求时检查。</p>}
        <div className="cloud-actions">
          {busy && <button className="button secondary" onClick={() => { cancelled.current = true; void invoke<Connection>("cancel_cloud_verification").then(next => { if (mounted.current) apply(next); }).catch(() => setError("取消失败，请等待验证超时。")); }}>取消验证</button>}
          {connection && connection.state !== "unconfigured" && <button className="button text-button" disabled={busy} onClick={() => {
            if (!window.confirm("解除此云端连接？本地题库和历史评测记录会保留。")) return;
            setKey(""); void run(async () => { const next = await invoke<Connection>("remove_cloud_connection"); if (mounted.current) { apply(next); notify("连接已解除"); } });
          }}>解除绑定</button>}
          {connection?.allowed && <button className="button secondary" disabled={busy || changed} onClick={() => { void run(async () => {
            const next = await invoke<Connection>("personalized_access"); if (mounted.current) { apply(next); setEntered(next.allowed); if(next.allowed) onContinue?.(); }
          }); }}>继续</button>}
        </div>
        {entered && <div className="cloud-ready" role="status"><h3>连接准备就绪</h3><p>账号连接已确认。用户画像和题目生成将在下一阶段开放；当前不会创建评测任务或产生推理费用。</p></div>}
      </>}
    </div>
  </div>;
}
