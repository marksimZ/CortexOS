import { getCurrentWebview } from "@tauri-apps/api/webview";
import { FileUp, Loader2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { knowledge, type DroppedKnowledgeItem } from "../api/knowledge";
import { navigate, type PageName } from "../navigation";
import { errorText, type Studio } from "../studio";
import { notify } from "./Toast";

export function DropImportOverlay({
  studio,
  page,
  onDocumentsImported,
}: {
  studio: Studio;
  page: PageName;
  onDocumentsImported: (workspaceId: string, docs: DroppedKnowledgeItem[]) => void;
}) {
  const [dragging, setDragging] = useState(false);
  const [importing, setImporting] = useState(false);
  const latestRef = useRef({ studio, page, onDocumentsImported });
  latestRef.current = { studio, page, onDocumentsImported };
  const busyRef = useRef(false);

  useEffect(() => {
    if (!studio.desktop) return;
    let active = true;
    const unlistenPromise = getCurrentWebview()
      .onDragDropEvent((event) => {
        if (!active) return;
        const { studio: curStudio, page: curPage, onDocumentsImported: onDocs } = latestRef.current;
        if (!curStudio.workspaceId) {
          setDragging(false);
          return;
        }
        const type = event.payload.type;
        if (type === "enter") {
          setDragging(true);
        } else if (type === "leave") {
          setDragging(false);
        } else if (type === "drop") {
          setDragging(false);
          const paths = event.payload.paths ?? [];
          if (!paths.length || busyRef.current) return;
          const ws = curStudio.workspaceId;
          busyRef.current = true;
          setImporting(true);
          void knowledge("import_dropped_files", { workspaceId: ws, paths })
            .then((report) => {
              if (!active) return;
              if (report.documents.length > 0) {
                onDocs(ws, report.documents);
                if (curPage === "chat") {
                  const existing = curStudio.contextSelection;
                  const merged = [...existing];
                  for (const doc of report.documents) {
                    const idx = merged.findIndex((item) => item.nodeId === doc.nodeId);
                    if (idx >= 0) {
                      merged[idx] = { nodeId: doc.nodeId, version: doc.version };
                    } else if (merged.length < 20) {
                      merged.push({ nodeId: doc.nodeId, version: doc.version });
                    }
                  }
                  curStudio.setContextSelection(merged);
                  requestAnimationFrame(() => document.getElementById("message")?.focus());
                }
                const names = report.documents
                  .slice(0, 2)
                  .map((d) => `《${d.title}》`)
                  .join("、");
                const extra = report.documents.length > 2 ? ` 等 ${report.documents.length} 份资料` : "";
                notify(
                  curPage === "chat"
                    ? `已导入 ${names}${extra}，并挂载为本轮对话上下文`
                    : `已导入 ${names}${extra} 到当前工作区知识库`,
                  ws,
                );
              }
              if (report.images.length > 0) {
                const nextImages = [...curStudio.draftImages, ...report.images].slice(0, 4);
                curStudio.setDraftImages(nextImages);
                if (curPage !== "chat") {
                  navigate("chat", { workspace: ws, conversation: curStudio.conversationId });
                }
                requestAnimationFrame(() => document.getElementById("message")?.focus());
                notify(`已添加 ${report.images.length} 张图片到对话输入框`, ws);
              }
              if (report.errors.length > 0) {
                const summary = report.errors
                  .slice(0, 3)
                  .map((e) => `${e.fileName}：${e.reason}`)
                  .join("；");
                curStudio.reportError(summary);
              }
            })
            .catch((err) => {
              if (active) curStudio.reportError(errorText(err));
            })
            .finally(() => {
              busyRef.current = false;
              if (active) setImporting(false);
            });
        }
      })
      .catch(() => undefined);

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten?.()).catch(() => {});
    };
  }, [studio.desktop]);

  if (!dragging && !importing) return null;

  return (
    <div
      className="drop-import-overlay"
      role="status"
      aria-live="polite"
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1500,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "rgba(15, 23, 42, 0.36)",
        backdropFilter: "blur(4px)",
        pointerEvents: importing ? "auto" : "none",
      }}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: "10px",
          padding: "28px 36px",
          borderRadius: "16px",
          border: "2px dashed var(--accent, #2563eb)",
          background: "var(--surface-elevated, var(--surface))",
          color: "var(--ink, var(--text))",
          boxShadow: "0 18px 48px rgba(0, 0, 0, 0.22)",
          maxWidth: "460px",
          textAlign: "center",
        }}
      >
        {importing ? (
          <>
            <Loader2 size={32} className="spin" aria-hidden="true" />
            <strong style={{ fontSize: "1rem" }}>正在解析并导入文件…</strong>
            <p style={{ margin: 0, fontSize: "0.85rem", color: "var(--muted)" }}>
              提取文本并建立本地检索索引，完成后立即可用。
            </p>
          </>
        ) : (
          <>
            <FileUp size={34} aria-hidden="true" style={{ color: "var(--accent, #2563eb)" }} />
            <strong style={{ fontSize: "1.02rem" }}>
              {page === "chat"
                ? "松手导入资料并挂载到当前对话"
                : "松手导入文件到当前工作区知识库"}
            </strong>
            <p style={{ margin: 0, fontSize: "0.85rem", color: "var(--muted)", lineHeight: 1.5 }}>
              支持 PDF、PPTX、DOCX、XLSX、EPUB、Markdown、TXT、代码文件；
              {page === "chat" ? "拖入图片将直接作为附图贴入输入框。" : "拖入图片将自动跳转对话页贴入输入框。"}
            </p>
          </>
        )}
      </div>
    </div>
  );
}
