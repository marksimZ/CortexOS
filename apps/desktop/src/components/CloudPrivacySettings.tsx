import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "../studio";

type Rules = { blockEmail: boolean; blockPhone: boolean };

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

  return <details className="advanced-settings" onToggle={event => {
    if (event.currentTarget.open && !rules && !busy) void load();
  }}>
    <summary>发送隐私检查</summary>
    <p>发送前检查常见密钥和访问令牌。可按工作区增加下列检查，命中后在本机提示修改。</p>
    {rules && <form className="rag-index-options" onSubmit={event => {
      event.preventDefault();
      void load(rules);
    }}>
      <label><input type="checkbox" checked={rules.blockEmail} disabled={busy} onChange={event => {
        setSaved(false);
        setRules({ ...rules, blockEmail: event.target.checked });
      }} />拦截常见邮箱地址</label>
      <label><input type="checkbox" checked={rules.blockPhone} disabled={busy} onChange={event => {
        setSaved(false);
        setRules({ ...rules, blockPhone: event.target.checked });
      }} />拦截中国大陆连续 11 位手机号</label>
      <button type="submit" className="button secondary" disabled={busy}>保存隐私检查</button>
    </form>}
    {busy && <p role="status">正在处理隐私检查设置…</p>}
    {saved && <p role="status">已保存隐私检查。</p>}
    {error && <p className="form-error" role="alert">{error}</p>}
  </details>;
}
