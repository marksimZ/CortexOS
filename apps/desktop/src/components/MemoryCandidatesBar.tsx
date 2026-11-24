import { useEffect, useState } from "react";
import { Check, Sparkles, X } from "lucide-react";
import { memoryApi, type MemoryCandidate } from "../api/memory";
import { errorText } from "../frontend-error";
import { StatusLabel } from "./StatusLabel";
import { notify } from "./Toast";

const scopeLabel: Record<MemoryCandidate["scope"], string> = {
  global_rule: "长期规则",
  global_preference: "个人偏好",
  workspace_fact: "项目记忆",
};

export function MemoryCandidatesBar({
  workspaceId,
  conversationId,
  refreshKey,
  onResolved,
}: {
  workspaceId: string | null;
  conversationId?: string | null;
  refreshKey: number;
  onResolved?: () => void;
}) {
  const [items, setItems] = useState<MemoryCandidate[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    if (!workspaceId) {
      setItems([]);
      return;
    }
    let current = true;
    void memoryApi("list_memory_candidates", {
      workspaceId,
      conversationId: conversationId ?? null,
    })
      .then((rows) => {
        if (current) {
          setItems(rows);
          setError("");
        }
      })
      .catch(() => {});
    return () => {
      current = false;
    };
  }, [workspaceId, conversationId, refreshKey]);

  if (!workspaceId || items.length === 0) return null;

  async function resolve(
    item: MemoryCandidate,
    action: "accept" | "dismiss",
    editedContent?: string,
  ) {
    if (busyId || !workspaceId) return;
    setBusyId(item.id);
    setError("");
    try {
      await memoryApi("resolve_memory_candidate", {
        workspaceId,
        candidateId: item.id,
        action,
        editedContent: editedContent ?? null,
      });
      setItems((prev) => prev.filter((row) => row.id !== item.id));
      if (editingId === item.id) setEditingId(null);
      onResolved?.();
      if (action === "accept") {
        notify(
          item.scope === "workspace_fact"
            ? "已记住为当前项目记忆，后续对话可自动召回"
            : "已沉淀到「关于我」全局画像，跨工作区生效",
        );
      }
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusyId(null);
    }
  }

  return (
    <section
      className="memory-candidates-bar"
      aria-label="发现可记住的偏好与事实"
      style={{
        margin: "8px 0",
        padding: "10px 12px",
        borderRadius: "var(--radius-md, 8px)",
        border: "1px solid var(--border-subtle, var(--border))",
        background: "var(--surface-elevated, var(--surface))",
      }}
    >
      <div className="task-line" style={{ marginBottom: "6px" }}>
        <strong style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "0.875rem" }}>
          <Sparkles size={15} aria-hidden="true" />
          发现可记住的信息（{items.length}）
        </strong>
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <ul className="task-list" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
        {items.map((item) => {
          const editing = editingId === item.id;
          return (
            <li key={item.id} className="task-line" style={{ alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
              <div style={{ flex: "1 1 240px", display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
                <StatusLabel tone={item.scope === "workspace_fact" ? "neutral" : "success"}>
                  {scopeLabel[item.scope]}
                </StatusLabel>
                {editing ? (
                  <input
                    value={draft}
                    maxLength={240}
                    style={{ flex: "1 1 200px" }}
                    onChange={(e) => setDraft(e.target.value)}
                  />
                ) : (
                  <span>{item.content}</span>
                )}
              </div>
              <div style={{ display: "inline-flex", gap: "6px" }}>
                {editing ? (
                  <button
                    type="button"
                    className="button primary"
                    disabled={busyId === item.id || !draft.trim()}
                    onClick={() => void resolve(item, "accept", draft)}
                  >
                    保存记住
                  </button>
                ) : (
                  <>
                    <button
                      type="button"
                      className="button primary"
                      disabled={busyId === item.id}
                      onClick={() => void resolve(item, "accept")}
                    >
                      <Check size={14} aria-hidden="true" />
                      记住
                    </button>
                    <button
                      type="button"
                      className="button secondary"
                      disabled={busyId === item.id}
                      onClick={() => {
                        setEditingId(item.id);
                        setDraft(item.content);
                      }}
                    >
                      编辑
                    </button>
                  </>
                )}
                <button
                  type="button"
                  className="button text-button"
                  aria-label="忽略这条记忆"
                  disabled={busyId === item.id}
                  onClick={() => void resolve(item, "dismiss")}
                >
                  <X size={14} aria-hidden="true" />
                  忽略
                </button>
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
