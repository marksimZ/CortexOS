import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SelectMenu } from "./SelectMenu";
import { errorText, type ContextSelection } from "../studio";
import type { CloudKnowledgeGrant } from "../runtime-profiles";

type Entry = ContextSelection & { title: string };

export function CloudKnowledgeSettings({ workspaceId, connectionId }: { workspaceId: string; connectionId: string }) {
  const [grants, setGrants] = useState<CloudKnowledgeGrant[]>();
  const [entries, setEntries] = useState<Entry[]>([]);
  const [selected, setSelected] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const input = { workspaceId, connectionId };

  const choice = entries.find(entry => entry.nodeId === selected);

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    try { await action(); }
    catch (failure) { setError(errorText(failure)); }
    finally { setBusy(false); }
  }

  async function read() {
    const result = await invoke<{ grants: CloudKnowledgeGrant[]; candidates: Entry[] }>("list_model_cloud_knowledge", { ...input, includeCandidates: true });
    setGrants(result.grants);
    setEntries(result.candidates);
  }

  function update(selection: ContextSelection, allowed: boolean) {
    void run(async () => {
      await invoke("set_model_cloud_knowledge", { ...input, selection, allowed });
      if (allowed) setSelected("");
      await read();
    });
  }

  return <details className="advanced-settings" onToggle={event => {
    if (event.currentTarget.open && !grants && !busy) void run(read);
  }}>
    <summary>此连接可使用的知识</summary>
    <p>按生效版本授权。生成回答前仍需确认发送范围。</p>
    {grants && <>
      {entries.length > 0 && <div className="cloud-runtime-actions">
        <div className="field"><label htmlFor="cloud-knowledge-version">添加可用版本</label>
          <SelectMenu id="cloud-knowledge-version" value={selected} disabled={busy} onChange={event => setSelected(event.target.value)}>
            <option value="">选择已生效的知识</option>
            {entries.map(entry => <option key={entry.nodeId} value={entry.nodeId}>{entry.title}（版本 {entry.version}）</option>)}
          </SelectMenu></div>
        <button type="button" className="button secondary" disabled={busy || !choice} onClick={() => {
          if (choice) update({ nodeId: choice.nodeId, version: choice.version }, true);
        }}>允许使用此版本</button>
      </div>}
      {grants.length ? <ul className="cloud-runtime-capabilities">
        {grants.map(grant => <li key={grant.nodeId + "/" + grant.version}>
          <span>{grant.title}（版本 {grant.version}）{!grant.available && "　已失效"}</span>
          <button type="button" className="button text-button" disabled={busy}
            onClick={() => update({ nodeId: grant.nodeId, version: grant.version }, false)}>撤销授权</button>
        </li>)}
      </ul> : <p>此连接还没有授权的知识版本。</p>}
    </>}
    {busy && <p role="status">正在读取或保存授权…</p>}
    {error && <p className="form-error" role="alert">{error}</p>}
  </details>;
}
