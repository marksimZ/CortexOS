import { formatFolderSync, knowledge } from "./api/knowledge";
import { webSearch } from "./api/web-search";
import { AppDialog, openDialog } from "./components/AppDialog";
import { SelectMenu } from "./components/SelectMenu";
import { useEffect, useRef, useState } from "react";
import { notify } from "./components/Toast";
import { FileText, FolderSync, Globe, Plus, X } from "lucide-react";
import { errorText } from "./studio";
import type { EditKnowledge } from "./knowledge-types";
import { FileName } from "./components/FileName";
import { PaneDivider, splitStyle } from "./components/PaneDivider";
export interface SourceRef {
  sourceId: string;
  version: number;
  startLine: number;
  endLine: number;
}
export interface SourceEvidence extends SourceRef {
  name: string;
  enabled: boolean;
  excerpt: string;
  contentHash: string;
  knowledgeVersion: number;
}
export interface SourceDocument {
  id: string;
  name: string;
  latestVersion: number;
  enabled: boolean;
  revision: number;
}
export interface Snapshot {
  document: SourceDocument;
  version: number;
  content: string;
  contentHash: string;
  createdAt: string;
  structure?: { parentCount: number; childCount: number; sections: [string, number][] };
}
export interface Preview {
  ticket: string;
  name: string;
  content: string;
  contentHash: string;
  previousVersion: number | null;
}
export function SourceBrowser({
  workspace,
  initial,
  onClose,
  onChanged,
  onDraft,
}: {
  workspace: string;
  initial?: SourceRef;
  onClose: () => void;
  onChanged?: () => void;
  onDraft?: (request: EditKnowledge) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const splitRoot = useRef<HTMLDivElement>(null);
  const readerToggle = useRef<HTMLButtonElement>(null);
  const [splitRatio, setSplitRatio] = useState<number | null>(null);
  const [readerCollapsed, setReaderCollapsed] = useState(false);
  const revoke = useRef<HTMLDialogElement>(null);
  const [items, setItems] = useState<SourceDocument[]>([]);
  const [watchedFolders, setWatchedFolders] = useState<
    { id: string; workspaceId: string; path: string; lastSyncedAt: string | null; createdAt: string }[]
  >([]);
  const [selected, setSelected] = useState<Snapshot | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [webUrl, setWebUrl] = useState("");
  const [linePage, setLinePage] = useState(0);
  const [start, setStart] = useState(initial?.startLine ?? 1);
  const [end, setEnd] = useState(initial?.endLine ?? 1);
  const live = useRef(true);
  const generation = useRef(0);
  const read = async (id: string, version: number, range?: SourceRef) => {
    const task = ++generation.current;
    setError(null);
    setLoading(true);
    try {
      const next = await knowledge("get_source", {
        workspaceId: workspace,
        sourceId: id,
        version,
      });
      if (live.current && task === generation.current) {
        setSelected(next);
        setReaderCollapsed(false);
        setLinePage(Math.floor(((range?.startLine ?? 1) - 1) / 200) * 200);
        setStart(range?.startLine ?? 1);
        setEnd(range?.endLine ?? Math.min(10, next.content.split("\n").length));
      }
    } catch (value) {
      if (live.current && task === generation.current)
        setError(errorText(value));
    } finally {
      if (live.current && task === generation.current) setLoading(false);
    }
  };
  const refresh = async () => {
    const [list, folders] = await Promise.all([
      knowledge("list_sources", { workspaceId: workspace }),
      knowledge("list_watched_folders", { workspaceId: workspace }).catch(() => []),
    ]);
    if (live.current) {
      setItems(list);
      setWatchedFolders(folders);
    }
    return list;
  };
  useEffect(() => {
    openDialog(dialog.current);
    live.current = true;
    void refresh()
      .then(() =>
        initial
          ? read(initial.sourceId, initial.version, initial)
          : setLoading(false),
      )
      .catch((value) => {
        if (live.current) {
          setError(errorText(value));
          setLoading(false);
        }
      });
    return () => {
      live.current = false;
      generation.current++;
    };
  }, []);
  const perform = async (action: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (value) {
      if (live.current) setError(errorText(value));
    } finally {
      if (live.current) setBusy(false);
    }
  };
  const cancelPreview = async () => {
    if (preview)
      await knowledge("discard_source", {
        workspaceId: workspace,
        ticket: preview.ticket,
      });
    if (live.current) setPreview(null);
  };
  const close = () => {
    if (!busy)
      void perform(async () => {
        await cancelPreview();
        onClose();
      });
  };
  const lines = selected?.content.split("\n") ?? [];
  const valid =
    !!selected &&
    Number.isInteger(start) &&
    Number.isInteger(end) &&
    start >= 1 &&
    end >= start &&
    end <= lines.length;
  const excerpt = valid ? lines.slice(start - 1, end).join("\n") : "";
  return (
    <>
      <AppDialog
        ref={dialog}
        className="source-browser form-dialog panel-dialog"
        aria-labelledby="sources-title"
        onCancel={(event) => {
          event.preventDefault();
          if (!busy) close();
        }}
      >
        <header className="source-toolbar panel-header">
          <h2 id="sources-title">资料文件</h2>
          <div className="source-actions">
            {readerCollapsed && <button ref={readerToggle} className="button secondary" onClick={() => {
              setReaderCollapsed(false);
              requestAnimationFrame(() => splitRoot.current?.querySelector<HTMLElement>('[role="separator"]')?.focus());
            }}>显示文件预览</button>}
            <button
              className="button secondary"
              disabled={busy}
              onClick={() =>
                void perform(async () => {
                  const report = await knowledge("import_source_folder", {
                    workspaceId: workspace,
                  });
                  if (report && live.current) {
                    if (report.imported || report.updated) onChanged?.();
                    await refresh();
                    notify(`同步完成：${formatFolderSync(report)}`);
                  }
                })
              }
            >
              <FolderSync size={16} />
              同步文件夹
            </button>
            <button
              className="button secondary"
              disabled={busy}
              onClick={() =>
                void perform(async () => {
                  const next = await knowledge("choose_source", {
                    workspaceId: workspace,
                  });
                  if (live.current) setPreview(next);
                })
              }
            >
              <Plus size={16} />
              {busy ? "正在处理…" : "添加文件"}
            </button>
            <button
              className="icon-button"
              aria-label="关闭资料文件"
              disabled={busy}
              onClick={close}
            >
              <X size={20} />
            </button>
          </div>
        </header>
        <div className="panel-body">
        {error && (
          <p role="alert" className="form-error">
            {error}
          </p>
        )}
        {!preview && (
          <form
            className="web-url-capture"
            style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 10 }}
            onSubmit={(event) => {
              event.preventDefault();
              const targetUrl = webUrl.trim();
              if (!targetUrl) return;
              void perform(async () => {
                const page = await webSearch("fetch_web_page", {
                  workspaceId: workspace,
                  url: targetUrl,
                  saveToKnowledge: true,
                });
                if (live.current) {
                  setWebUrl("");
                  onChanged?.();
                  notify(`已抓取网页并存入知识：${page.title}`);
                }
              });
            }}
          >
            <Globe size={16} aria-hidden="true" style={{ flexShrink: 0, opacity: 0.7 }} />
            <input
              type="url"
              value={webUrl}
              onChange={(event) => setWebUrl(event.target.value)}
              placeholder="粘贴公开网页链接（https://...），抓取正文并直接存入知识库…"
              disabled={busy}
              style={{ flex: 1, minWidth: 0 }}
            />
            <button className="button secondary" type="submit" disabled={busy || !webUrl.trim()}>
              {busy ? "正在抓取…" : "抓取网页"}
            </button>
          </form>
        )}
        {!preview && watchedFolders.length > 0 && (
          <section className="watched-folders" aria-label="已绑定的同步文件夹" style={{ marginBottom: 10 }}>
            <div style={{ fontSize: 12, opacity: 0.75, marginBottom: 4 }}>
              已绑定的同步文件夹（打开工作区时自动增量同步）：
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              {watchedFolders.map((folder) => (
                <div
                  key={folder.id}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    fontSize: 12,
                    padding: "4px 8px",
                    borderRadius: 6,
                    background: "var(--surface-muted, rgba(127,127,127,0.08))",
                  }}
                >
                  <FolderSync size={14} aria-hidden="true" style={{ flexShrink: 0, opacity: 0.7 }} />
                  <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={folder.path}>
                    {folder.path}
                  </span>
                  <button
                    type="button"
                    className="button text-button"
                    disabled={busy}
                    onClick={() =>
                      void perform(async () => {
                        const report = await knowledge("sync_watched_folder", {
                          workspaceId: workspace,
                          folderId: folder.id,
                        });
                        if (live.current) {
                          if (report.imported || report.updated) onChanged?.();
                          await refresh();
                          notify(`同步完成：${formatFolderSync(report)}`);
                        }
                      })
                    }
                  >
                    重新同步
                  </button>
                  <button
                    type="button"
                    className="button text-button"
                    disabled={busy}
                    onClick={() =>
                      void perform(async () => {
                        await knowledge("remove_watched_folder", {
                          workspaceId: workspace,
                          folderId: folder.id,
                        });
                        if (live.current) {
                          await refresh();
                          notify("已移除同步文件夹绑定");
                        }
                      })
                    }
                  >
                    解绑
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}
        {preview ? (
          <section className="source-preview">
            <h3><FileName name={preview.name} /></h3>
            <p>
              {preview.previousVersion
                ? `已保存版本 v${preview.previousVersion}。内容有变化会另存一个版本；文件已停用时，会恢复使用。`
                : "会在本机保存一份文字副本，不修改原文件。"}
            </p>
            <pre className="source-text">{preview.content}</pre>
            <details>
              <summary>会保存哪些内容</summary>
              <p>
                只保存这个文件的 UTF-8 文字，不读取文件夹。最多2
                MiB。保存的是文字内容，不包含原文件的其他属性；行号以这里显示的内容为准。添加后可选择文字保存为知识草稿，确认后才用于回答。原文件修改后，需重新添加。
              </p>
              <code className="source-hash">SHA-256 {preview.contentHash}</code>
            </details>

          </section>
        ) : (
          <div ref={splitRoot} className={`source-workbench ${readerCollapsed ? "preview-collapsed" : ""}`} style={!readerCollapsed ? splitStyle(splitRatio, 20) : undefined}>
            <aside className="source-list" aria-label="已添加的文件">
              {items.map((item) => (
                <button
                  key={item.id}
                  disabled={busy}
                  className="source-row"
                  aria-current={
                    selected?.document.id === item.id ? "true" : undefined
                  }
                  onClick={() => void read(item.id, item.latestVersion)}
                >
                  <FileText size={16} />
                  <span>
                    <FileName name={item.name} />
                    <span>
                      v{item.latestVersion}
                      {item.enabled ? "" : "，已停用"}
                    </span>
                  </span>
                </button>
              ))}
              {!items.length && !loading && (
                <p>还没有文件，点击“添加文件”选择 TXT 或 Markdown 文件。</p>
              )}
            </aside>
            <section hidden={readerCollapsed} className="source-reader" aria-label="文件内容">
              {loading ? (
                <p role="status">正在读取文件…</p>
              ) : selected ? (
                <>
                  <header className="source-toolbar">
                    <h3>
                      <FileName name={selected.document.name} />
                      {!selected.document.enabled && "，已停用"}
                    </h3>
                    <label>
                      文件版本
                      <SelectMenu
                        aria-label="文件版本"
                        disabled={busy}
                        value={selected.version}
                        onChange={(event) =>
                          void read(
                            selected.document.id,
                            Number(event.target.value),
                          )
                        }
                      >
                        {Array.from(
                          { length: selected.document.latestVersion },
                          (_, i) => i + 1,
                        )
                          .reverse()
                          .map((version) => (
                            <option key={version} value={version}>
                              v{version}
                            </option>
                          ))}
                      </SelectMenu>
                    </label>
                  </header>
                  {!!selected.structure?.sections.length && (
                    <details className="field-help">
                      <summary>章节目录</summary>
                      <ul>
                        {selected.structure.sections.slice(0, 60).map(([title, line]) => (
                          <li key={line}>
                            <button className="button text-button" type="button" onClick={() => {
                              setStart(line);
                              setEnd(line);
                              setLinePage(Math.floor((line - 1) / 200) * 200);
                            }}>{title}（第 {line} 行）</button>
                          </li>
                        ))}
                      </ul>
                      {selected.structure.sections.length > 60 && <p>显示前 60 个章节。</p>}
                    </details>
                  )}
                  <div className="source-line-inputs">
                    <label>
                      起始行
                      <input
                        type="number"
                        aria-label="起始行"
                        min={1}
                        max={lines.length}
                        value={start}
                        onChange={(event) => {
                          setStart(Number(event.target.value));
                          setLinePage(
                            Math.max(
                              0,
                              Math.floor(
                                (Number(event.target.value) - 1) / 200,
                              ) * 200,
                            ),
                          );
                        }}
                      />
                    </label>
                    <label>
                      结束行
                      <input
                        type="number"
                        aria-label="结束行"
                        min={start}
                        max={lines.length}
                        value={end}
                        onChange={(event) => setEnd(Number(event.target.value))}
                      />
                    </label>
                    {onDraft && (
                      <button
                        className="button primary"
                        disabled={
                          busy ||
                          !selected.document.enabled ||
                          !excerpt.trim() ||
                          [...excerpt].length > 20000
                        }
                        onClick={() => {
                          onDraft({
                            workspaceId: workspace,
                            sourceDocument: {
                              sourceId: selected.document.id,
                              version: selected.version,
                              startLine: start,
                              endLine: end,
                            },
                            sourceName: selected.document.name,
                            excerpt,
                          });
                        }}
                      >
                        将所选文字存为知识草稿
                      </button>
                    )}
                  </div>
                  {!valid && (
                    <p role="alert" className="form-error">
                      请选择有效起止行。
                    </p>
                  )}
                  <div className="source-actions">
                    <button
                      className="button secondary"
                      disabled={linePage === 0}
                      onClick={() => setLinePage(Math.max(0, linePage - 200))}
                    >
                      上一段
                    </button>
                    <span>
                      第 {linePage + 1}–{Math.min(linePage + 200, lines.length)}{" "}
                      行 / 共 {lines.length} 行
                    </span>
                    <button
                      className="button secondary"
                      disabled={linePage + 200 >= lines.length}
                      onClick={() => setLinePage(linePage + 200)}
                    >
                      下一段
                    </button>
                  </div>
                  <pre className="source-text numbered-source">
                    {lines
                      .slice(linePage, linePage + 200)
                      .map((line, index) => {
                        const i = index + linePage;
                        return (
                          <span
                            key={i}
                            className={
                              valid && i + 1 >= start && i + 1 <= end
                                ? "selected-source-line"
                                : ""
                            }
                          >
                            <span
                              aria-hidden="true"
                              className="source-line-number"
                            >
                              {i + 1}
                            </span>
                            {line || " "}
                          </span>
                        );
                      })}
                  </pre>
                  <details>
                    <summary>保存记录与使用权限</summary>
                    <p>
                      保存于{" "}
                      {new Date(selected.createdAt).toLocaleString("zh-CN")}
                      。这是当时保存的副本，不会随原文件变化。停用后，从此文件建立的知识不再用于新的检索或选用；以前的对话和保存记录仍保留。
                    </p>
                    <code className="source-hash">
                      SHA-256 {selected.contentHash}
                    </code>
                    {selected.document.enabled && (
                      <button
                        className="button secondary"
                        disabled={busy}
                        onClick={() => openDialog(revoke.current)}
                      >
                        停用文件
                      </button>
                    )}
                  </details>
                </>
              ) : (
                <p>选择一个文件，查看内容和以前保存的版本。</p>
              )}
            </section>
            {!readerCollapsed && <PaneDivider root={splitRoot} ratio={splitRatio} onChange={setSplitRatio} onCollapse={() => {
              setReaderCollapsed(true);
              requestAnimationFrame(() => readerToggle.current?.focus());
            }} label="调整文件预览宽度" />}
          </div>
        )}
        </div>
        {preview && (
            <footer className="dialog-actions panel-footer">
              <button
                className="button secondary"
                disabled={busy}
                onClick={() => void perform(cancelPreview)}
              >
                取消导入
              </button>
              <button
                className="button primary"
                disabled={busy}
                onClick={() =>
                  void perform(async () => {
                    const saved = await knowledge(
                      "commit_source",
                      { workspaceId: workspace, ticket: preview.ticket },
                    );
                    if (!live.current) return;
                    notify("文件文字副本已保存");
                    onChanged?.();
                    setPreview(null);
                    await refresh();
                    await read(saved.id, saved.latestVersion);
                  })
                }
              >
                确认导入
              </button>
            </footer>
        )}
      </AppDialog>
      <AppDialog
        ref={revoke}
        className="form-dialog"
        aria-labelledby="revoke-source-title"
        onCancel={(event) => {
          if (busy) event.preventDefault();
        }}
      >
        <h2 id="revoke-source-title">停用这个文件？</h2>
        <p>
          从这个文件建立的知识将不再用于新的检索或选用。以前的对话、知识版本和文件副本仍保留，其中可能包含这个文件的内容。
        </p>
        <div className="dialog-actions">
          <button
            className="button secondary"
            disabled={busy}
            onClick={() => revoke.current?.close()}
          >
            取消
          </button>
          <button
            className="button primary"
            disabled={busy}
            onClick={() =>
              void perform(async () => {
                if (!selected) return;
                await knowledge("revoke_source", {
                  workspaceId: workspace,
                  sourceId: selected.document.id,
                  expectedRevision: selected.document.revision,
                });
                revoke.current?.close();
                onChanged?.();
                await refresh();
                await read(selected.document.id, selected.version);
              })
            }
          >
            确认停用
          </button>
        </div>
      </AppDialog>
    </>
  );
}
