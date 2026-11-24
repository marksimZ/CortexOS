import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { AppDialog, openDialog } from "./components/AppDialog";
import { SelectMenu } from "./components/SelectMenu";
import { closeDraftWindow, draftStore, registerDraftEditor } from "./draft-store";
import { kinds, type EditKnowledge, type KnowledgeDetail } from "./knowledge-types";
import { knowledge } from "./api/knowledge";
import { errorText } from "./studio";

type KnowledgeDraft = { title: string; kind: string; content: string; validFrom: string; validTo: string; reason: string; excerpt: string };
const knowledgeDrafts = draftStore<KnowledgeDraft>("knowledge", value => {
  if (!value || typeof value !== "object") return false;
  const row = value as Record<string, unknown>;
  return ["title", "kind", "content", "validFrom", "validTo", "reason", "excerpt"].every(key => typeof row[key] === "string");
});

export function KnowledgeEditor({
  request,
  onClose,
  onSaved,
}: {
  request: EditKnowledge;
  onClose: () => void;
  onSaved: (workspaceId: string, nodeId: string) => void;
}) {
  const original = request.detail?.versions[0];
  const draftKey = JSON.stringify([request.workspaceId, request.detail?.node.id ?? null,
    request.detail?.node.revision ?? null, request.source?.id ?? null, request.sourceDocument ?? null]);
  const [cached] = useState(() => knowledgeDrafts.get(draftKey));
  const [title, setTitle] = useState(
    cached?.title ?? original?.title ??
      request.sourceName?.replace(/\.(md|txt)$/i, "").slice(0, 80) ??
      "",
  );
  const [kind, setKind] = useState(cached?.kind ?? original?.kind ?? "knowledge");
  const [content, setContent] = useState(
    cached?.content ?? original?.content ?? request.excerpt ?? "",
  );
  const [validFrom, setValidFrom] = useState(cached?.validFrom ?? original?.validity?.validFrom ?? "");
  const [validTo, setValidTo] = useState(cached?.validTo ?? original?.validity?.validTo ?? "");
  const initialReason = original ? "" : request.source ? "从回答保存草稿" : request.sourceDocument ? "根据文件文字整理" : "手动记录";
  const [reason, setReason] = useState(cached?.reason ?? initialReason);
  const [excerpt, setExcerpt] = useState(cached?.excerpt ?? request.excerpt ?? "");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [latest, setLatest] = useState<KnowledgeDetail | null>(null);
  const [expected, setExpected] = useState(
    request.detail?.node.revision ?? null,
  );
  const dialog = useRef<HTMLDialogElement>(null);
  const leaving = useRef<HTMLDialogElement>(null);
  const lastEditorFocus = useRef<HTMLElement | null>(null);
  const form = useRef<HTMLFormElement>(null);
  const exitWindow = useRef(false);
  const busy = useRef(false);
  const changed =
    excerpt !== (request.excerpt ?? "") ||
    title !== (original?.title ?? request.sourceName?.replace(/\.(md|txt)$/i, "").slice(0, 80) ?? "") ||
    kind !== (original?.kind ?? "knowledge") ||
    content !== (original?.content ?? request.excerpt ?? "") ||
    validFrom !== (original?.validity?.validFrom ?? "") ||
    validTo !== (original?.validity?.validTo ?? "") ||
    reason !== initialReason;
  const dirty = useRef(changed);
  dirty.current = changed;
  const [durable, setDurable] = useState(() => knowledgeDrafts.persisted(draftKey));
  useLayoutEffect(() => {
    if (changed) setDurable(knowledgeDrafts.set(draftKey, { title, kind, content, validFrom, validTo, reason, excerpt }));
    else if (knowledgeDrafts.has(draftKey)) knowledgeDrafts.delete(draftKey);
  }, [changed, draftKey, title, kind, content, validFrom, validTo, reason, excerpt]);
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
    openDialog(dialog.current);
    return registerDraftEditor(() => {
      if (busy.current) return true;
      if (!dirty.current) return false;
      askClose(true);
      return true;
    });
  }, []);
  const save = async (closeAfter = false) => {
    if (!form.current?.reportValidity() || busy.current) return;
    busy.current = true;
    setSaving(true);
    setError(null);
    try {
      const fields = {
          validity: { validFrom: validFrom || null, validTo: validTo || null },
          input: {
            workspaceId: request.workspaceId,
            nodeId: request.detail?.node.id ?? null,
            expectedRevision: expected,
            title,
            kind,
            content,
            changeReason: reason,
            sourceMessageId: request.source?.id ?? null,
            sourceExcerpt: request.source ? excerpt : null,
          },
      };
      const id = request.sourceDocument
        ? await knowledge("save_source_knowledge", { ...fields, source: request.sourceDocument })
        : await knowledge("save_knowledge", fields);
      knowledgeDrafts.delete(draftKey);
      dirty.current = false;
      if (closeAfter && exitWindow.current) await finishClose();
      else onSaved(request.workspaceId, id);
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
        className="knowledge-editor form-dialog panel-dialog"
        aria-labelledby="knowledge-edit-title"
        onCancel={(e) => {
          e.preventDefault();
          askClose();
        }}
      >
        <form
          ref={form}
          className="panel-form"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <header className="drawer-heading panel-header">
            <h2 id="knowledge-edit-title">
              {original ? "修改知识草稿" : "新建知识草稿"}
            </h2>
            <button
              type="button"
              className="icon-button"
              aria-label="关闭知识编辑"
              disabled={saving}
              onClick={() => askClose()}
            >
              <X size={20} />
            </button>
          </header>
          <div className="panel-body">
          <div className="knowledge-edit-fields">
            <div className="field">
              <label htmlFor="knowledge-title">标题</label>
              <input
                id="knowledge-title"
                data-initial-focus
                value={title}
                maxLength={80}
                required
                readOnly={saving}
                onChange={(e) => setTitle(e.target.value)}
                autoFocus
              />
            </div>
            <div className="field">
              <label htmlFor="knowledge-kind">类型</label>
              <SelectMenu
                id="knowledge-kind"
                value={kind}
                disabled={saving}
                onChange={(e) => setKind(e.target.value)}
              >
                {Object.entries(kinds).map(([value, label]) => (
                  <option value={value} key={value}>
                    {label}
                  </option>
                ))}
              </SelectMenu>
            </div>
          </div>
          <div className="field">
            <label htmlFor="knowledge-content">正文</label>
            <textarea
              id="knowledge-content"
              rows={9}
              value={content}
              required
              maxLength={20000}
              readOnly={saving}
              onChange={(e) => setContent(e.target.value)}
            />
          </div>
          {request.sourceDocument && (
            <p>
              来源：{request.sourceName}，v{request.sourceDocument.version}，
              第 {request.sourceDocument.startLine}–
              {request.sourceDocument.endLine} 行
            </p>
          )}
          {request.source && (
            <details className="knowledge-disclosure">
              <summary>引用的原文（可缩短）</summary>
              <label className="sr-only" htmlFor="knowledge-excerpt">
                来源摘录
              </label>
              <textarea
                id="knowledge-excerpt"
                rows={4}
                value={excerpt}
                maxLength={20000}
                readOnly={saving}
                onChange={(e) => setExcerpt(e.target.value)}
              />
              <p>
                引用文字须保持原样，知识正文可以改写。有引用不代表内容一定正确。
              </p>
            </details>
          )}
          <details className="knowledge-disclosure">
            <summary>事实有效日期（选填）</summary>
            <div className="knowledge-edit-fields">
              <div className="field">
                <label htmlFor="knowledge-valid-from">生效日期</label>
                <input id="knowledge-valid-from" type="date" value={validFrom} readOnly={saving}
                  onChange={(event) => setValidFrom(event.target.value)} />
              </div>
              <div className="field">
                <label htmlFor="knowledge-valid-to">失效日期</label>
                <input id="knowledge-valid-to" type="date" value={validTo} readOnly={saving}
                  onChange={(event) => setValidTo(event.target.value)} />
              </div>
            </div>
            <p>失效日期当天起不再用于查找。留空表示未记录该日期。</p>
          </details>
          <div className="field">
            <label htmlFor="knowledge-reason">
              {original ? "修改说明" : "记录说明"}
            </label>
            <input
              id="knowledge-reason"
              value={reason}
              required
              maxLength={500}
              readOnly={saving}
              onChange={(e) => setReason(e.target.value)}
            />
          </div>
          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}
          {error && request.detail && (
            <button
              type="button"
              className="button text-button"
              disabled={saving}
              onClick={() => {
                void knowledge("get_knowledge", { workspaceId: request.workspaceId, nodeId: request.detail!.node.id })
                  .then(setLatest)
                  .catch((e) => setError(errorText(e)));
              }}
            >
              查看最新版本
            </button>
          )}
          {latest && (
            <div className="conflict-comparison">
              <h3>
                最新版本 v{latest.node.latestVersion}：
                {latest.versions[0].title}
              </h3>
              <div className="knowledge-prose">
                {latest.versions[0].content}
              </div>
              <button
                type="button"
                className="button secondary"
                onClick={() => {
                  setExpected(latest.node.revision);
                  setLatest(null);
                  setError(null);
                }}
              >
                已比较，保留我的编辑并重新保存
              </button>
            </div>
          )}
          </div>
          <footer className="dialog-actions panel-footer">
            <span className="editor-status">
              保存后直接生效并纳入对话检索
            </span>
            <button
              type="button"
              className="button secondary"
              disabled={saving}
              onClick={() => askClose()}
            >
              取消
            </button>
            <button className="button primary" disabled={saving}>
              {saving ? "正在保存…" : "保存并生效"}
            </button>
          </footer>
        </form>
      </AppDialog>
      <AppDialog
        ref={leaving}
        className="form-dialog"
        aria-labelledby="leave-knowledge-title"
        onCancel={() => {
          exitWindow.current = false;
        }}
      >
        <h2 id="leave-knowledge-title">保留未保存的修改？</h2>
        <p>保存后将新增待确认草稿。</p>
        <div className="dialog-actions">
          <button
            className="button text-button"
            onClick={() => {
              knowledgeDrafts.delete(draftKey);
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
            保存草稿
          </button>
        </div>
      </AppDialog>
    </>
  );
}
