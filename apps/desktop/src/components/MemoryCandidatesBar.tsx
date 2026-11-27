import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, GitCompareArrows, Sparkles, X } from "lucide-react";
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
  const [leavingIds, setLeavingIds] = useState<Set<string>>(() => new Set());
  const [busyId, setBusyId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState("");

  const loadCandidates = useCallback(
    async (ws: string, conv?: string | null) => {
      const rows = await memoryApi("list_memory_candidates", {
        workspaceId: ws,
        conversationId: conv ?? null,
      });
      return rows;
    },
    [],
  );

  useEffect(() => {
    if (!workspaceId) {
      setItems([]);
      return;
    }
    let current = true;
    void loadCandidates(workspaceId, conversationId)
      .then((rows) => {
        if (current) {
          setItems(rows);
          setError("");
        }
      })
      .catch(() => {});

    const unlistenPromise = listen<{ workspaceId: string; conversationId?: string }>(
      "memory-candidates-updated",
      ({ payload }) => {
        if (!current || payload.workspaceId !== workspaceId) return;
        if (conversationId && payload.conversationId && payload.conversationId !== conversationId) {
          return;
        }
        void loadCandidates(workspaceId, conversationId)
          .then((rows) => {
            if (current) setItems(rows);
          })
          .catch(() => {});
      },
    ).catch(() => undefined);

    return () => {
      current = false;
      void unlistenPromise.then((unlisten) => unlisten?.()).catch(() => {});
    };
  }, [workspaceId, conversationId, refreshKey, loadCandidates]);

  if (!workspaceId || items.length === 0) return null;

  async function resolve(
    item: MemoryCandidate,
    action: "accept" | "dismiss",
    editedContent?: string,
  ) {
    if (busyId || !workspaceId) return;
    setBusyId(item.id);
    setError("");
    const isEvolution = Boolean(item.replacesNodeId || item.replacesValue);
    try {
      await memoryApi("resolve_memory_candidate", {
        workspaceId,
        candidateId: item.id,
        action,
        editedContent: editedContent ?? null,
      });
      const animate = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      if (animate) {
        setLeavingIds((prev) => new Set(prev).add(item.id));
        window.setTimeout(() => {
          setItems((prev) => prev.filter((row) => row.id !== item.id));
          setLeavingIds((prev) => {
            const next = new Set(prev);
            next.delete(item.id);
            return next;
          });
        }, 180);
      } else {
        setItems((prev) => prev.filter((row) => row.id !== item.id));
      }
      if (editingId === item.id) setEditingId(null);
      onResolved?.();
      if (action === "accept") {
        if (isEvolution) {
          notify(
            item.scope === "workspace_fact"
              ? `已演进更新记忆《${item.replacesTitle || item.title}》，旧版本已归档至历史`
              : "已更新「关于我」中的既有偏好/规则",
          );
        } else {
          notify(
            item.scope === "workspace_fact"
              ? "已记住为当前项目记忆，后续对话可自动召回"
              : "已沉淀到「关于我」全局画像，跨工作区生效",
          );
        }
      }
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusyId(null);
    }
  }

  const allLeaving = items.length > 0 && items.every((row) => leavingIds.has(row.id));

  return (
    <section
      className="memory-candidates-bar"
      data-leaving={allLeaving ? "true" : undefined}
      aria-label="发现可记住的偏好与事实"
      style={{
        margin: "8px 0",
        padding: "10px 12px",
        borderRadius: "var(--radius-control)",
        border: "1px solid var(--line)",
        background: "var(--surface)",
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
          const isLeaving = leavingIds.has(item.id);
          const hasConflict = Boolean(item.replacesNodeId || item.replacesValue);
          return (
            <li
              key={item.id}
              className="collapsible-row"
              data-leaving={isLeaving ? "true" : undefined}
            >
              <div
                className="collapsible-row-inner task-line"
                style={{
                  alignItems: "flex-start",
                  gap: "8px",
                  flexWrap: "wrap",
                  padding: hasConflict ? "8px 10px" : undefined,
                  borderRadius: hasConflict ? "var(--radius-inset)" : undefined,
                  background: hasConflict ? "var(--warning-bg)" : undefined,
                }}
              >
              <div style={{ flex: "1 1 260px", display: "flex", flexDirection: "column", gap: "4px" }}>
                <div style={{ display: "flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
                  <StatusLabel tone={hasConflict ? "warning" : item.scope === "workspace_fact" ? "neutral" : "success"}>
                    {hasConflict ? `记忆演进 · ${scopeLabel[item.scope]}` : scopeLabel[item.scope]}
                  </StatusLabel>
                  {hasConflict && item.conflictReason && (
                    <small style={{ color: "var(--warning)", display: "inline-flex", alignItems: "center", gap: "4px" }}>
                      <GitCompareArrows size={13} aria-hidden="true" />
                      {item.conflictReason}
                    </small>
                  )}
                </div>
                {hasConflict && (item.replacesValue || item.replacesTitle) && (
                  <small style={{ color: "var(--muted)", lineHeight: 1.45 }}>
                    旧记录：{item.replacesTitle ? `《${item.replacesTitle}》 ` : ""}
                    <del style={{ opacity: 0.85 }}>{item.replacesValue || "既有版本"}</del>
                  </small>
                )}
                {editing ? (
                  <input
                    value={draft}
                    maxLength={240}
                    style={{ width: "100%", marginTop: "2px" }}
                    onChange={(e) => setDraft(e.target.value)}
                  />
                ) : (
                  <span style={{ fontSize: "var(--text-control)", lineHeight: 1.5 }}>
                    {hasConflict ? `新记录：${item.content}` : item.content}
                  </span>
                )}
              </div>
              <div style={{ display: "inline-flex", gap: "6px", alignSelf: "center" }}>
                {editing ? (
                  <button
                    type="button"
                    className="button primary"
                    disabled={busyId === item.id || !draft.trim()}
                    onClick={() => void resolve(item, "accept", draft)}
                  >
                    {hasConflict ? "保存并更新版本" : "保存记住"}
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
                      {hasConflict ? "更新并保留历史" : "记住"}
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
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
