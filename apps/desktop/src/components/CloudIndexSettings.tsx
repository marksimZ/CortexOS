import { useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { AppDialog, openDialog } from "./AppDialog";
import type { CloudKnowledgeGrant } from "../runtime-profiles";
import { errorText } from "../studio";

type IndexStatus = {
  index: { dimensions: number } | null;
  readyVersions: number;
  build?: { vectorCount: number; requests: number; inputTokens: number | null };
};
const key = (grant: CloudKnowledgeGrant) => grant.nodeId + "/" + grant.version;

export function CloudIndexSettings({ workspaceId, connectionId, model }: {
  workspaceId: string; connectionId: string; model: string;
}) {
  const [status, setStatus] = useState<IndexStatus>();
  const [grants, setGrants] = useState<CloudKnowledgeGrant[]>([]);
  const [selected, setSelected] = useState(new Set<string>());
  const [busy, setBusy] = useState(false);
  const request = useRef<{ workspaceId: string; requestId: string } | null>(null);
  const [phase, setPhase] = useState("");
  const [building, setBuilding] = useState(false);
  const [error, setError] = useState("");
  const confirmation = useRef<HTMLDialogElement>(null);
  const args = { workspaceId, connectionId, model };

  async function read() {
    setBusy(true);
    setError("");
    try {
      const [index, data] = await Promise.all([
        invoke<IndexStatus>("get_cloud_rag_index", args),
        invoke<{ grants: CloudKnowledgeGrant[] }>("list_model_cloud_knowledge", { workspaceId, connectionId }),
      ]);
      const allowed = data.grants.filter(item => item.available);
      setStatus(index);
      setGrants(allowed);
      setSelected(new Set(allowed.map(key)));
    } catch (failure) { setError(errorText(failure)); }
    finally { setBusy(false); }
  }

  async function build() {
    confirmation.current?.close();
    const owner = { workspaceId, requestId: crypto.randomUUID() };
    request.current = owner;
    const statusChannel = new Channel<string>();
    statusChannel.onmessage = message => { if (request.current === owner) setPhase(message); };
    setPhase("正在申请执行…");
    setBuilding(true);
    setError("");
    try {
      const versions = grants.filter(item => selected.has(key(item))).map(({ nodeId, version }) => ({ nodeId, version }));
      const result = await invoke<IndexStatus>("build_cloud_rag_index", { ...owner, statusChannel, input: { connectionId, model, versions } });
      setStatus(result);
    } catch (failure) { setError(errorText(failure)); }
    finally { setBuilding(false); }
  }

  return <details className="advanced-settings rag-settings" onToggle={event => {
    if (event.currentTarget.open && !status && !busy) void read();
  }}>
    <summary>云端向量索引</summary>
    {status && <div className="rag-index-meta">
      <strong>{status.index ? status.readyVersions + " 个知识版本可用于向量查找" : "尚未建立索引"}</strong>
      {status.index && <span>{status.index.dimensions} 维{status.build && "，已保存 " + status.build.vectorCount + " 个向量"}</span>}
    </div>}
    <button type="button" className="button text-button" disabled={busy || building} onClick={() => void read()}>刷新索引和授权</button>
    {!!grants.length && <details className="field-help">
      <summary>选中 {selected.size} / {grants.length} 个授权版本</summary>
      <ul className="cloud-runtime-capabilities">
        {grants.map(grant => <li key={key(grant)}><label className="retrieval-switch">
          <input type="checkbox" checked={selected.has(key(grant))} disabled={busy || building} onChange={event => {
            const next = new Set(selected);
            if (event.target.checked) next.add(key(grant)); else next.delete(key(grant));
            setSelected(next);
          }} />
          {grant.title}（版本 {grant.version}）
        </label></li>)}
      </ul>
    </details>}
    {status && !grants.length && <p>请先在「此连接可使用的知识」中授权版本。</p>}
    <button type="button" className="button secondary" disabled={busy || building || !selected.size || selected.size > 2000}
      onClick={() => openDialog(confirmation.current)}>{status?.index ? "重建云端索引…" : "构建云端索引…"}</button>
    {building && <button type="button" className="button text-button" onClick={() => {
      if (request.current) void invoke("rag_cancel_index", request.current).catch(failure => setError(errorText(failure)));
    }}>停止构建</button>}
    {busy || building ? <p role="status">{building ? phase : "正在读取索引…"}</p> : null}
    {error && <p role="alert" className="form-error">{error}</p>}
    <AppDialog ref={confirmation} className="form-dialog" aria-labelledby="cloud-index-title">
      <h2 id="cloud-index-title">确认构建云端向量索引</h2>
      <p>将选中的 {selected.size} 个授权知识版本的标题和正文片段发送给 {model}。服务商可能按请求收费。</p>
      <p>向量保存在本机。全部构建成功后替换这个模型的旧索引。</p>
      <div className="dialog-actions">
        <button type="button" className="button text-button" onClick={() => confirmation.current?.close()}>取消</button>
        <button type="button" className="button primary" disabled={busy || building || !selected.size} onClick={() => void build()}>确认发送并构建</button>
      </div>
    </AppDialog>
  </details>;
}
