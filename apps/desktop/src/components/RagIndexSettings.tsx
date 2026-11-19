import { Channel, invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { errorText } from "../studio";
import { AppDialog, openDialog } from "./AppDialog";
import { SelectMenu } from "./SelectMenu";

interface Model { name: string; digest: string }
interface IndexStatus {
  index: null | { model: string; digest: string; dimensions: number; enabled: boolean; rerank: boolean };
  indexed: number;
  eligible: number;
}

export function RagIndexSettings({ workspaceId }: { workspaceId: string }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [status, setStatus] = useState<IndexStatus>();
  const [models, setModels] = useState<Model[]>();
  const [selected, setSelected] = useState("");
  const [busy, setBusy] = useState(false);
  const request = useRef<{ workspaceId: string; requestId: string } | null>(null);
  const [phase, setPhase] = useState("");
  const [building, setBuilding] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const read = () => invoke<IndexStatus>("rag_index_status", { workspaceId });
  useEffect(() => {
    let live = true;
    void read().then(result => { if (live) setStatus(result); })
      .catch(reason => { if (live) setError(errorText(reason)); });
    return () => { live = false; };
  }, [workspaceId]);
  const picked = models?.find(model => model.name === selected);
  async function act(operation: () => Promise<void>) {
    setBusy(true);
    setError("");
    setNotice("");
    try { await operation(); }
    catch (reason) { setError(errorText(reason)); }
    finally { setBusy(false); }
  }
  const index = status?.index;
  function setOptions(enabled: boolean, rerank: boolean) {
    if (!status || !index || busy) return;
    setStatus({ ...status, index: { ...index, enabled, rerank } });
    void act(async () => {
      try {
        setStatus(await invoke<IndexStatus>("rag_set_index_options", { workspaceId, enabled, rerank }));
      } catch (reason) {
        setStatus(status);
        throw reason;
      }
    });
  }
  async function build() {
    if (!picked || busy || building) return;
    dialog.current?.close();
    const owner = { workspaceId, requestId: crypto.randomUUID() };
    request.current = owner;
    const statusChannel = new Channel<string>();
    statusChannel.onmessage = message => { if (request.current === owner) setPhase(message); };
    setPhase("正在申请执行…");
    setBuilding(true);
    await act(async () => {
      setStatus(await invoke<IndexStatus>("rag_build_index", {
        ...owner, statusChannel, input: { model: picked.name, digest: picked.digest, confirmed: true },
      }));
      setNotice("索引构建完成");
    });
    setBuilding(false);
  }
  return <details className="advanced-settings rag-settings">
    <summary>知识检索</summary>
    <div className="rag-index-meta">
      <strong>{status && index ? `${status.indexed} / ${status.eligible} 项已建立向量索引` : "按词语查找"}</strong>
      {index && <span>{index.model}，{index.dimensions} 维</span>}
    </div>
    <div className="cloud-actions">
      <button type="button" className="button secondary" disabled={busy} onClick={() => void act(async () => {
        const found = await invoke<Model[]>("rag_embedding_models");
        setModels(found);
        setSelected(previous => found.some(item => item.name === previous) ? previous : found[0]?.name ?? "");
      })}>读取本地向量模型</button>
      {building && <button type="button" className="button secondary" onClick={() => {
        if (request.current) void invoke("rag_cancel_index", request.current).catch(reason => setError(errorText(reason)));
      }}>停止构建</button>}
    </div>
    {models?.length === 0 && <p>没有检测到支持向量检索的本地模型。</p>}
    {!!models?.length && <div className="field">
      <label htmlFor="rag-embedding-model">向量模型</label>
      <SelectMenu id="rag-embedding-model" value={selected} disabled={busy} onChange={event => setSelected(event.target.value)}>
        {models.map(item => <option key={item.name} value={item.name}>{item.name}</option>)}
      </SelectMenu>
      <button type="button" className="button secondary" disabled={busy || !picked || !status?.eligible} onClick={() => openDialog(dialog.current)}>
        {index ? "重建索引…" : "构建索引…"}
      </button>
    </div>}
    {status && index && <div className="rag-index-options">
      <label><input type="checkbox" checked={index.enabled} disabled={busy} onChange={event => setOptions(event.target.checked, index.rerank)} /> 按语义与词语查找</label>
      <p>启用后，发送问题时会用本地模型生成问题向量。</p>
      <label><input type="checkbox" checked={index.rerank} disabled={busy || !index.enabled} onChange={event => setOptions(index.enabled, event.target.checked)} /> 按关键词覆盖重新排序</label>
      {status.indexed < status.eligible && <p>部分已批准知识尚未索引。它们仍可通过词语查找。</p>}
    </div>}
    <p role="status">{building ? phase : notice}</p>
    {error && <p className="form-error" role="alert">{error}</p>}
    <AppDialog ref={dialog} className="panel-dialog form-dialog" aria-labelledby="rag-build-title">
      <header className="panel-header">
        <h2 id="rag-build-title">确认构建向量索引</h2>
        <button type="button" className="icon-button" aria-label="关闭构建确认" onClick={() => dialog.current?.close()}><X size={18} /></button>
      </header>
      <div className="panel-body">
        <p>使用 {picked?.name} 为当前工作区的已批准知识和文件片段建立索引，最多 2000 条。向量生成在本机 Ollama 上使用 CPU。</p>
        <p>向量全部保存后切换索引。你可以在设置页停止构建。</p>
      </div>
      <footer className="dialog-actions">
        <button type="button" className="button secondary" onClick={() => dialog.current?.close()}>返回</button>
        <button type="button" className="button primary" disabled={!picked || busy} onClick={() => void build()}>确认并构建</button>
      </footer>
    </AppDialog>
  </details>;
}
