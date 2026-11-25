import { useEffect, useState } from "react";
import { Link2, Sparkles } from "lucide-react";
import { knowledge, type SuggestedKnowledgeLink } from "../api/knowledge";
import { relations } from "../graph-data";
import { kinds } from "../knowledge-types";
import { notify } from "./Toast";
import { errorText } from "../studio";

export function SuggestedLinksPanel({
  workspaceId,
  nodeId,
  revision,
  onLinked,
  onSelectNode,
}: {
  workspaceId: string | null;
  nodeId: string;
  revision: number;
  onLinked: () => void;
  onSelectNode: (id: string) => void;
}) {
  const [items, setItems] = useState<SuggestedKnowledgeLink[]>([]);
  const [loading, setLoading] = useState(false);
  const [linkingId, setLinkingId] = useState<string | null>(null);

  useEffect(() => {
    if (!workspaceId || !nodeId) {
      setItems([]);
      return;
    }
    let active = true;
    setLoading(true);
    void knowledge("suggest_knowledge_links", { workspaceId, nodeId })
      .then((res) => {
        if (active) setItems(res);
      })
      .catch(() => {
        if (active) setItems([]);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [workspaceId, nodeId, revision]);

  if (!workspaceId || (!loading && items.length === 0)) return null;

  const quickConnect = async (item: SuggestedKnowledgeLink) => {
    if (!workspaceId || linkingId) return;
    setLinkingId(item.nodeId);
    try {
      await knowledge("save_knowledge_edge", {
        input: {
          workspaceId,
          edgeId: null,
          expectedVersion: null,
          fromNodeId: nodeId,
          toNodeId: item.nodeId,
          relation: item.suggestedRelation,
          active: true,
          changeReason: `采纳推荐关联：${item.reason}`,
        },
      });
      setItems((prev) => prev.filter((x) => x.nodeId !== item.nodeId));
      notify(`已建立「${relations[item.suggestedRelation] ?? item.suggestedRelation}」连线`);
      onLinked();
    } catch (err) {
      notify(errorText(err));
    } finally {
      setLinkingId(null);
    }
  };

  return (
    <section className="suggested-links-panel" aria-label="推荐建立的知识关联">
      <div className="suggested-links-header">
        <Sparkles size={14} aria-hidden="true" />
        <strong>智能推荐关联</strong>
      </div>
      {loading ? (
        <p className="small-note">正在分析潜在关联…</p>
      ) : (
        <ul className="suggested-links-list">
          {items.map((item) => (
            <li key={item.nodeId} className="suggested-link-item">
              <div className="suggested-link-info">
                <button
                  type="button"
                  className="suggested-link-title"
                  onClick={() => onSelectNode(item.nodeId)}
                >
                  <i className={`node-key node-${item.kind}`} aria-hidden="true" />
                  <span>{item.title}</span>
                </button>
                <small>
                  {kinds[item.kind] ?? item.kind} · 建议关系：
                  <strong>{relations[item.suggestedRelation] ?? item.suggestedRelation}</strong> · {item.reason}
                </small>
              </div>
              <button
                type="button"
                className="button secondary suggested-link-btn"
                disabled={linkingId === item.nodeId}
                onClick={() => void quickConnect(item)}
              >
                <Link2 size={13} aria-hidden="true" />
                {linkingId === item.nodeId ? "连接中…" : "一键连线"}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
