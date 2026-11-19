import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { relations } from "../graph-data";
import { navigate } from "../navigation";
import { errorText, type KnowledgeClusters, type Studio } from "../studio";

export function KnowledgeGraphOptions({ studio }: { studio: Studio }) {
  const [overview, setOverview] = useState<KnowledgeClusters>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const workspace = studio.workspaceId;

  async function load() {
    if (busy || !workspace) return;
    setBusy(true);
    setError("");
    try {
      setOverview(await invoke<KnowledgeClusters>("get_knowledge_clusters", {
        workspaceId: workspace,
        knowledgeScope: studio.knowledgeScope,
      }));
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  }

  return <details className="field-help knowledge-scope">
    <summary>图谱查找</summary>
    <label>
      <input type="checkbox" checked={studio.useGraph}
        disabled={!studio.autoRetrieval || studio.savingRetrieval || studio.sending || !!studio.activeRun}
        onChange={(event) => studio.setUseGraph(event.target.checked)} />
      每次都沿知识关系查找（最多两跳）
    </label>
    {!studio.autoRetrieval && <p>开启自动查找知识后可沿关系扩展。</p>}
    <button type="button" className="button secondary" aria-busy={busy}
      disabled={busy || !studio.knowledgeScopeReady} onClick={() => void load()}>
      {busy ? "正在读取…" : overview ? "刷新知识簇概览" : "查看知识簇概览"}
    </button>
    {error && <p role="alert" className="form-error">{error}</p>}
    {overview?.groups.length === 0 && <p role="status">当前范围内还没有相互关联的知识。</p>}
    {overview?.truncated && <p>此处展示部分关联组。</p>}
    {overview?.groups.map((group) => (
      <details key={group.id} className="knowledge-disclosure">
        <summary>{group.name}（{group.nodeCount} 项知识，{group.fileCount} 份文件）</summary>
        <p>{group.edgeCount} 条关系，{Object.entries(group.relationCounts).map(([kind,count]) => `${relations[kind] ?? kind} ${count}`).join("，")}。</p>
        <ul>
          {group.examples.map(({ node, excerpt, excerptTruncated }) => (
            <li key={node.nodeId}>
              <button type="button" className="button text-button" onClick={() => navigate("knowledge", {
                workspace, node:node.nodeId, version:node.version, back:window.location.hash,
              })}>{node.title} v{node.version}</button>
              {node.source && <p>{node.source.name}，文件 v{node.source.version}</p>}
              <p className="context-content">{excerpt}{excerptTruncated ? "…" : ""}</p>
            </li>
          ))}
        </ul>
      </details>
    ))}
  </details>;
}
