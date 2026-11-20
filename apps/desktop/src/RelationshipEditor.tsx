import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { AppDialog, openDialog } from "./components/AppDialog";
import { SelectMenu } from "./components/SelectMenu";
import { closeDraftWindow, draftStore, registerDraftEditor } from "./draft-store";
import { knowledge } from "./api/knowledge";
import { relations, type GraphData, type GraphEdge } from "./graph-data";
import type { KnowledgeNode } from "./knowledge-types";
import { errorText } from "./studio";

export interface EditEdge {
  workspaceId: string;
  graph: GraphData;
  selected: string;
  edge?: GraphEdge;
}

type RelationDraft = { from: string; to: string; relation: string; active: boolean; reason: string };
const relationDrafts = draftStore<RelationDraft>("relation", value => {
  if (!value || typeof value !== "object") return false;
  const row = value as Record<string, unknown>;
  return typeof row.active === "boolean" && ["from", "to", "relation", "reason"].every(key => typeof row[key] === "string");
});

export function RelationshipEditor({
  request,
  onClose,
  onSaved,
}: {
  request: EditEdge;
  onClose: () => void;
  onSaved: () => void;
}) {
  const original = request.edge?.versions[0];
  const [nodeOptions, setNodeOptions] = useState<KnowledgeNode[]>(
    request.graph.nodes,
  );
  const [loadingOptions, setLoadingOptions] = useState(true);
  const [optionsError, setOptionsError] = useState<string | null>(null);
  const [optionsRetry, setOptionsRetry] = useState(0);
  const draftKey = JSON.stringify([request.workspaceId, request.edge?.id ?? request.selected, original?.version ?? null]);
  const [cached] = useState(() => relationDrafts.get(draftKey));
  const [from, setFrom] = useState(cached?.from ?? original?.fromNodeId ?? request.selected);
  const [to, setTo] = useState(
    cached?.to ?? original?.toNodeId ??
      request.graph.nodes.find((node) => node.id !== request.selected)?.id ??
      "",
  );
  const [relation, setRelation] = useState(cached?.relation ?? original?.relation ?? "supports");
  const [active, setActive] = useState(cached?.active ?? original?.active ?? true);
  const [reason, setReason] = useState(cached?.reason ?? "");
  const [expected, setExpected] = useState(original?.version ?? null);
  const [latest, setLatest] = useState<GraphEdge | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const leaving = useRef<HTMLDialogElement>(null);
  const lastEditorFocus = useRef<HTMLElement | null>(null);
  const form = useRef<HTMLFormElement>(null);
  const busy = useRef(false);
  const dirty = useRef(false);
  const exitWindow = useRef(false);
  dirty.current =
    reason !== "" ||
    from !== (original?.fromNodeId ?? request.selected) ||
    to !==
      (original?.toNodeId ??
        request.graph.nodes.find((node) => node.id !== request.selected)?.id ??
        "") ||
    relation !== (original?.relation ?? "supports") ||
    active !== (original?.active ?? true);
  const [durable, setDurable] = useState(() => relationDrafts.persisted(draftKey));
  useLayoutEffect(() => {
    if (dirty.current) setDurable(relationDrafts.set(draftKey, { from, to, relation, active, reason }));
    else if (relationDrafts.has(draftKey)) relationDrafts.delete(draftKey);
  }, [draftKey, from, to, relation, active, reason]);
  const finishClose = async () => {
    if (exitWindow.current) await closeDraftWindow();
    else onClose();
  };
  const askClose = (window = false) => {
    if (busy.current) return;
    exitWindow.current = window;
    if (dirty.current) openDialog(leaving.current, () => lastEditorFocus.current ?? dialog.current);
    else void finishClose().catch((e) => setError(errorText(e)));
  };
  useEffect(() => {
    let current = true;
    setLoadingOptions(true);
    setOptionsError(null);
    void knowledge("list_knowledge", {
      workspaceId: request.workspaceId,
    })
      .then((nodes) => {
        if (current) setNodeOptions(nodes);
      })
      .catch((e) => {
        if (current) setOptionsError(errorText(e));
      })
      .finally(() => {
        if (current) setLoadingOptions(false);
      });
    return () => {
      current = false;
    };
  }, [request.workspaceId, optionsRetry]);
  useEffect(() => {
    openDialog(dialog.current);
    return registerDraftEditor(() => {
      if (busy.current) return true;
      if (!dirty.current) return false;
      askClose(true);
      return true;
    });
  }, []);
  const save = async (closing = false) => {
    if (
      busy.current ||
      loadingOptions ||
      optionsError ||
      !form.current?.reportValidity()
    )
      return;
    busy.current = true;
    setSaving(true);
    setError(null);
    try {
      await knowledge("save_knowledge_edge", {
        input: {
          workspaceId: request.workspaceId,
          edgeId: request.edge?.id ?? null,
          expectedVersion: expected,
          fromNodeId: from,
          toNodeId: to,
          relation,
          active,
          changeReason: reason,
        },
      });
      relationDrafts.delete(draftKey);
      dirty.current = false;
      if (closing && exitWindow.current) await finishClose();
      else onSaved();
    } catch (e) {
      setError(errorText(e));
    } finally {
      busy.current = false;
      setSaving(false);
    }
  };
  return (
    <>
      <AppDialog
        ref={dialog}
        onFocusCapture={event => { if (event.target instanceof HTMLElement) lastEditorFocus.current = event.target; }}
        className="form-dialog relation-editor panel-dialog"
        aria-labelledby="edge-edit-title"
        onCancel={(event) => {
          event.preventDefault();
          askClose();
        }}
      >
        <form
          ref={form}
          className="panel-form"
          onSubmit={(event) => {
            event.preventDefault();
            void save();
          }}
        >
          <header className="drawer-heading panel-header">
            <h2 id="edge-edit-title">{original ? "修改关系" : "添加关系"}</h2>
            <button
              type="button"
              className="icon-button"
              aria-label="关闭关系编辑"
              disabled={saving}
              onClick={() => askClose()}
            >
              <X size={19} />
            </button>
          </header>
          <div className="panel-body">
          <div className="field">
            <label htmlFor="edge-from">起点</label>
            {loadingOptions && <p role="status">正在读取节点选项…</p>}
            {optionsError && (
              <p className="form-error" role="alert">
                {optionsError}
                <button
                  type="button"
                  className="button text-button"
                  onClick={() => setOptionsRetry((n) => n + 1)}
                >
                  重新读取节点选项
                </button>
              </p>
            )}
            <SelectMenu
              id="edge-from"
              data-initial-focus
              value={from}
              disabled={saving || loadingOptions}
              onChange={(e) => setFrom(e.target.value)}
            >
              {nodeOptions.map((node) => (
                <option key={node.id} value={node.id}>
                  {node.title}
                </option>
              ))}
            </SelectMenu>
          </div>
          <div className="field">
            <label htmlFor="edge-relation">关系类型</label>
            <SelectMenu
              id="edge-relation"
              value={relation}
              disabled={saving}
              onChange={(e) => setRelation(e.target.value)}
            >
              {Object.entries(relations).map(([key, label]) => (
                <option key={key} value={key}>
                  {label}
                </option>
              ))}
            </SelectMenu>
          </div>
          <div className="field">
            <label htmlFor="edge-to">终点</label>
            <SelectMenu
              id="edge-to"
              value={to}
              disabled={saving || loadingOptions}
              required
              onChange={(e) => setTo(e.target.value)}
            >
              <option value="">请选择</option>
              {nodeOptions
                .filter((node) => node.id !== from)
                .map((node) => (
                  <option key={node.id} value={node.id}>
                    {node.title}
                  </option>
                ))}
            </SelectMenu>
          </div>
          {original && (
            <label className="edge-active">
              <input
                type="checkbox"
                checked={active}
                disabled={saving}
                onChange={(e) => setActive(e.target.checked)}
              />
              使用这条关系（取消勾选会保存一个停用版本）
            </label>
          )}
          <div className="field">
            <label htmlFor="edge-reason">修改说明</label>
            <input
              id="edge-reason"
              value={reason}
              maxLength={500}
              required
              readOnly={saving}
              onChange={(e) => setReason(e.target.value)}
            />
          </div>
          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}
          {error && request.edge && (
            <button
              type="button"
              className="button text-button"
              disabled={saving}
              onClick={() => {
                void knowledge("load_graph_edge_history", {
                  workspaceId: request.workspaceId,
                  edgeId: request.edge!.id,
                  beforeVersion: null,
                  limit: 1,
                })
                  .then((data) => {
                    setLatest({
                      id: request.edge!.id,
                      versions: data.versions,
                    });
                    setNodeOptions((previous) => [
                      ...previous,
                      ...data.nodes.filter(
                        (n) => !previous.some((p) => p.id === n.id),
                      ),
                    ]);
                  })
                  .catch((e) => setError(errorText(e)));
              }}
            >
              读取最新关系
            </button>
          )}
          {latest && (
            <div className="conflict-comparison">
              <h3>最新关系 v{latest.versions[0].version}</h3>
              <p>
                {
                  nodeOptions.find(
                    (node) => node.id === latest.versions[0].fromNodeId,
                  )?.title
                }{" "}
                → {relations[latest.versions[0].relation]} →{" "}
                {
                  nodeOptions.find(
                    (node) => node.id === latest.versions[0].toNodeId,
                  )?.title
                }{" "}
                ，{latest.versions[0].active ? "有效" : "停用"}
              </p>
              <p>{latest.versions[0].changeReason}</p>
              <button
                type="button"
                className="button secondary"
                onClick={() => {
                  setExpected(latest.versions[0].version);
                  setLatest(null);
                  setError(null);
                }}
              >
                已比较，保留我的关系编辑
              </button>
            </div>
          )}
          </div>
          <footer className="dialog-actions panel-footer">
            <button
              type="button"
              className="button secondary"
              disabled={saving}
              onClick={() => askClose()}
            >
              取消
            </button>
            <button
              className="button primary"
              disabled={saving || loadingOptions || !!optionsError || !to}
            >
              {saving ? "保存中…" : "保存关系"}
            </button>
          </footer>
        </form>
      </AppDialog>
      <AppDialog
        ref={leaving}
        className="form-dialog"
        aria-labelledby="leave-edge-title"
        onCancel={() => {
          exitWindow.current = false;
        }}
      >
        <h2 id="leave-edge-title">保留未保存的关系修改？</h2>
        <div className="dialog-actions">
          <button
            className="button text-button"
            onClick={() => {
              relationDrafts.delete(draftKey);
              dirty.current = false;
              void finishClose().catch((e) => setError(errorText(e)));
            }}
          >
            放弃修改
          </button>
          {durable && <button type="button" className="button secondary" onClick={() => void finishClose().catch(e => setError(errorText(e)))}>
            保留本机草稿
          </button>}
          <button data-initial-focus
            className="button secondary"
            onClick={() => {
              exitWindow.current = false;
              leaving.current?.close();
            }}
          >
            继续编辑
          </button>
          <button
            className="button primary"
            onClick={() => {
              leaving.current?.close();
              void save(true);
            }}
          >
            保存关系
          </button>
        </div>
      </AppDialog>
    </>
  );
}
