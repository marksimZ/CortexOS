import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "../studio";

type Rules = { blockEmail: boolean; blockPhone: boolean; syncMemoryAndHistory: boolean };

export function CloudPrivacySettings({ workspaceId }: { workspaceId: string }) {
  const [rules, setRules] = useState<Rules>();
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");

  async function load(input?: Rules) {
    setBusy(true);
    setSaved(false);
    setError("");
    try {
      setRules(await invoke<Rules>("model_cloud_privacy_rules", { workspaceId, input: input ?? null }));
      setSaved(!!input);
    } catch (failure) { setError(errorText(failure)); }
    finally { setBusy(false); }
  }

  return <details className="advanced-settings" open onToggle={event => {
    if (event.currentTarget.open && !rules && !busy) void load();
  }}>
    <summary>云端隐私与同步策略</summary>
    <p>默认同步当前工作区的助手人设、多轮对话上下文与未标“仅限本地”的记忆资料，发送前自动拦截常见密钥与访问令牌。</p>
    {rules && <form className="rag-index-options" onSubmit={event => {
      event.preventDefault();
      void load(rules);
    }}>
      <label><input type="checkbox" checked={rules.syncMemoryAndHistory} disabled={busy} onChange={event => {
        setSaved(false);
        setRules({ ...rules, syncMemoryAndHistory: event.target.checked });
      }} />云端对话默认同步助手设定、多轮历史与允许上云的记忆（免逐次弹窗确认）</label>
      <label><input type="checkbox" checked={rules.blockEmail} disabled={busy} onChange={event => {
        setSaved(false);
        setRules({ ...rules, blockEmail: event.target.checked });
      }} />拦截常见邮箱地址</label>
      <label><input type="checkbox" checked={rules.blockPhone} disabled={busy} onChange={event => {
        setSaved(false);
        setRules({ ...rules, blockPhone: event.target.checked });
      }} />拦截中国大陆连续 11 位手机号</label>
      <button type="submit" className="button secondary" disabled={busy}>保存隐私策略</button>
    </form>}
    {busy && <p role="status">正在处理隐私检查设置…</p>}
    {saved && <p role="status">已保存隐私检查。</p>}
    {error && <p className="form-error" role="alert">{error}</p>}
  </details>;
}
