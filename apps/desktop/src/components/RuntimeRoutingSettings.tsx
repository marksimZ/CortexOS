import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SelectMenu } from "./SelectMenu";
import { errorText } from "../studio";
import type { Profile } from "../runtime-profiles";

type Rule = { preferred: string | null; alternative: string | null };
type Routing = {
  preference: "preferred" | "lower_cost";
  question: Rule;
  knowledge: Rule;
  localBackup: string | null;
};
const tasks = { question: "普通问答", knowledge: "知识问答" } as const;

export function RuntimeRoutingSettings({ workspaceId, profiles, localModels }: {
  workspaceId: string; profiles: Profile[]; localModels: string[];
}) {
  const [routing, setRouting] = useState<Routing>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);

  async function load(input?: Routing) {
    setBusy(true);
    setError("");
    setSaved(false);
    try {
      setRouting(await invoke<Routing>("model_chat_routing", { workspaceId, input: input ?? null }));
      setSaved(!!input);
    } catch (failure) { setError(errorText(failure)); }
    finally { setBusy(false); }
  }

  function profileOptions(value: string | null, empty: string) {
    return <>
      <option value="">{empty}</option>
      {value && !profiles.some(profile => profile.id === value) && <option value={value}>方案已不可用</option>}
      {profiles.map(profile => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
    </>;
  }

  return <details className="advanced-settings" onToggle={event => {
    if (event.currentTarget.open && !routing && !busy) void load();
  }}>
    <summary>任务路由与备用模型</summary>
    {routing && <form className="settings-fields" onSubmit={event => {
      event.preventDefault();
      void load(routing);
    }}>
      <div className="parameter-fields">
        <div className="field">
          <label htmlFor="chat-route-preference">路由偏好</label>
          <SelectMenu id="chat-route-preference" value={routing.preference} disabled={busy} onChange={event => {
            setSaved(false);
            setRouting({ ...routing, preference: event.target.value as Routing["preference"] });
          }}>
            <option value="preferred">使用首选方案</option>
            <option value="lower_cost">优先较低估算费用</option>
          </SelectMenu>
        </div>
        <div className="field">
          <label htmlFor="chat-route-backup">本机备用回答模型</label>
          <SelectMenu id="chat-route-backup" value={routing.localBackup ?? ""} disabled={busy} onChange={event => {
            setSaved(false);
            setRouting({ ...routing, localBackup: event.target.value || null });
          }}>
            <option value="">关闭自动切换</option>
            {routing.localBackup && !localModels.includes(routing.localBackup)
              && <option value={routing.localBackup}>{routing.localBackup}（当前不可用）</option>}
            {localModels.map(model => <option key={model} value={model}>{model}</option>)}
          </SelectMenu>
        </div>
        {(Object.keys(tasks) as (keyof typeof tasks)[]).flatMap(task =>
          (["preferred", "alternative"] as const).map(field => {
            const id = "chat-route-" + task + "-" + field;
            const value = routing[task][field];
            return <div className="field" key={id}>
              <label htmlFor={id}>{tasks[task]}{field === "preferred" ? "首选方案" : "费用对照方案"}</label>
              <SelectMenu id={id} value={value ?? ""} disabled={busy} onChange={event => {
                setSaved(false);
                setRouting({ ...routing, [task]: { ...routing[task], [field]: event.target.value || null } });
              }}>
                {profileOptions(value, field === "preferred" ? "使用当前方案" : "不比较费用")}
              </SelectMenu>
            </div>;
          }),
        )}
      </div>
      <p>启用自动查找、选择资料或指定知识范围时，使用知识问答规则。首选方案可按你的质量要求配置。</p>
      <button type="submit" className="button secondary" disabled={busy}>保存任务路由</button>
    </form>}
    {!routing && error && <button type="button" className="button text-button" disabled={busy} onClick={() => void load()}>重新读取</button>}
    {busy && <p role="status">正在读取或保存任务路由…</p>}
    {saved && <p role="status">已保存任务路由。</p>}
    {error && <p className="form-error" role="alert">{error}</p>}
  </details>;
}
