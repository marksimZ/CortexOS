import { chat, type ContextCursor, type ContextOption } from "./api/chat";
import { useEffect, useRef, useState } from "react";
import { errorText, localDay, type ContextManifest, type Studio } from "./studio";
import { RetrievalSummary } from "./components/RetrievalSummary";
import { KnowledgeScopePicker } from "./components/KnowledgeScopePicker";
import { KnowledgeGraphOptions } from "./components/KnowledgeGraphOptions";

export function ContextPicker({ studio, cloudAnswer = false }: { studio: Studio; cloudAnswer?: boolean }) {
  const previewGeneration = useRef(0);
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<ContextOption[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [listLoading, setListLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [listRequest, setListRequest] = useState<{ after: ContextCursor | null }>({ after: null });
  const loadedRequest = useRef<typeof listRequest | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<ContextManifest | null>(null);
  const [busy, setBusy] = useState(false);
  const manual = !studio.knowledgeScope;
  useEffect(() => {
    if (!open || !manual || !studio.workspaceId || !studio.desktop || loadedRequest.current === listRequest) return;
    let current = true;
    setListLoading(true);
    setListError(null);
    void chat("list_context_options", {
      workspaceId: studio.workspaceId,
      day: localDay(),
      after: listRequest.after,
    })
      .then((page) => {
        if (!current) return;
        loadedRequest.current = listRequest;
        setItems((previous) => {
          if (!listRequest.after) return page.items;
          const incoming = new Set(page.items.map((item) => item.id));
          return [...previous.filter((item) => !incoming.has(item.id)), ...page.items];
        });
        setHasMore(page.hasMore);
      })
      .catch((value) => {
        if (current) setListError(errorText(value));
      })
      .finally(() => {
        if (current) setListLoading(false);
      });
    return () => {
      current = false;
    };
  }, [open, manual, studio.workspaceId, studio.desktop, listRequest]);
  useEffect(() => {
    previewGeneration.current++;
    setPreview(null);
  }, [
    studio.draft,
    studio.contextSelection,
    studio.settings,
    studio.autoRetrieval,
    studio.knowledgeScope,
    studio.useGraph,
  ]);
  if (!studio.desktop || !studio.workspaceId) return null;
  return (
    <details className="context-picker" onToggle={(event) => {
      if (event.target !== event.currentTarget) return;
      setOpen(event.currentTarget.open);
    }}>
      <summary>{studio.knowledgeScope
        ? studio.knowledgeScope.kind === "branch" ? "在实验分支查找"
          : studio.knowledgeScope.kind === "snapshot" ? "按知识快照查找" : `按 ${studio.knowledgeScope.date || "事实日期"} 查找`
        : `手动选择：${studio.contextSelection.length} 项知识`}</summary>
      <KnowledgeScopePicker studio={studio} />
      <KnowledgeGraphOptions key={JSON.stringify(studio.knowledgeScope)} studio={studio} />
      {!studio.knowledgeScope ? <>
      <div className="context-toolbar">
        <span>
          可选择已确认的知识、背景或决定。发送时会保存本次使用的版本。
        </span>
        <button
          type="button"
          className="button text-button"
              disabled={listLoading}
              onClick={() => setListRequest({ after: null })}
        >
          刷新知识列表
        </button>
      </div>
      {listLoading && <p role="status">正在读取知识列表…</p>}
      {listError && <p role="alert" className="form-error">{listError}</p>}
      {!listLoading && !listError && items.length === 0 && <p>尚无已确认知识。可先在知识页保存并确认。</p>}
      <div className="context-options">
        {items.map((item) => {
          const selected = studio.contextSelection.find(
            (x) => x.nodeId === item.id,
          );
          return (
            <label key={item.id}>
              <input
                type="checkbox"
                checked={!!selected}
                disabled={busy || studio.sending}
                onChange={(event) => {
                  studio.setContextSelection(
                    event.target.checked
                      ? [
                          ...studio.contextSelection,
                          { nodeId: item.id, version: item.effectiveVersion },
                        ]
                      : studio.contextSelection.filter(
                          (x) => x.nodeId !== item.id,
                        ),
                  );
                }}
              />
              <span>
                {item.title} <strong>当前使用 v{item.effectiveVersion}</strong>
                {item.latestVersion !== item.effectiveVersion && "，有新草稿"}
                {selected &&
                  selected.version !== item.effectiveVersion &&
                  "，所选版本已变化，请取消后重新选择"}
              </span>
            </label>
          );
        })}
      </div>
      {hasMore && <button
        type="button"
        className="button text-button"
        disabled={listLoading}
        onClick={() => {
          const last = items.at(-1);
          if (last) setListRequest({ after: { id: last.id, updatedAt: last.updatedAt } });
        }}
      >加载更多知识</button>}
      </> : !!studio.contextSelection.length && <p>切回当前知识后会恢复已手动选择的资料。</p>}
      <button
        type="button"
        className="button secondary"
        disabled={busy || !studio.draft.trim() || studio.savingRetrieval || !studio.knowledgeScopeReady}
        onClick={() => {
          const workspaceId = studio.workspaceId;
          if (!workspaceId) return;
          setBusy(true);
          setError(null);
          const generation = ++previewGeneration.current;
          void chat("preview_context", {
            workspaceId,
            conversationId: studio.conversationId,
            content: studio.draft,
            contextSelection: studio.knowledgeScope ? [] : studio.contextSelection,
            knowledgeScope: studio.knowledgeScope,
            useGraph: studio.useGraph,
          })
            .then((value) => {
              if (generation === previewGeneration.current) setPreview(value);
            })
            .catch((value) => {
              if (generation === previewGeneration.current)
                setError(errorText(value));
            })
            .finally(() => setBusy(false));
        }}
      >
        {busy ? "正在准备…" : cloudAnswer ? "查看可发送的所选资料" : "查看本次会用的资料"}
      </button>
      {error && (
        <p role="alert" className="form-error">
          {error}
        </p>
      )}
      {preview && (
        <div>
          {preview.assistant && <p>
            预计使用{preview.assistant.kind === "workspace"
              ? `「${preview.assistant.name}」版本 ${preview.assistant.version}`
              : "内置默认助手"}。
          </p>}
          {preview.knowledgeScope?.kind === "snapshot" && <p>使用快照「{preview.knowledgeScope.name}」。
            {!!preview.knowledgeScope.unavailableCount && `有 ${preview.knowledgeScope.unavailableCount} 项的来源已停用。`}</p>}
          {preview.knowledgeScope?.kind === "branch" && <p>使用实验分支「{preview.knowledgeScope.name}」。
            {!!preview.knowledgeScope.unavailableCount && `有 ${preview.knowledgeScope.unavailableCount} 项的来源已停用。`}</p>}
          {preview.knowledgeScope?.kind === "factDate" && <p>事实日期 {preview.knowledgeScope.factDate}。
            {!!preview.knowledgeScope.unknownDateCount && `已跳过 ${preview.knowledgeScope.unknownDateCount} 个日期未记录的知识版本。`}</p>}
          <p>
            预计发送内容用量 {preview.estimatedUnits} / {preview.budgetUnits}
            （按 UTF-8 字节估算，不是模型实际的 Token 数）；包含{" "}
            {preview.messageRefs.length} 条对话消息。
          </p>
          <RetrievalSummary report={preview.retrieval} references={preview.knowledgeRefs} workspaceId={studio.workspaceId} />
          {preview.knowledgeRefs?.map((item) => (
            <details key={item.nodeId}>
              <summary>
                {item.title}，v{item.version}
              </summary>
              <p className="context-content">{item.content}</p>
            </details>
          ))}
        </div>
      )}
    </details>
  );
}
