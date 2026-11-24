import { useState } from "react";
import { Copy, FileText } from "lucide-react";
import { memoryApi } from "../api/memory";
import { errorText } from "../frontend-error";
import { notify } from "./Toast";

export function WeeklyDigestPanel({ workspaceId }: { workspaceId: string }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [draft, setDraft] = useState<{
    episodeCount: number;
    knowledgeCount: number;
    markdown: string;
  } | null>(null);

  async function generate() {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const result = await memoryApi("generate_weekly_digest_draft", {
        workspaceId,
        days: 7,
      });
      setDraft(result);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section aria-labelledby="weekly-digest-title" style={{ marginTop: "16px" }}>
      <div className="task-line">
        <h3 id="weekly-digest-title" style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <FileText size={16} aria-hidden="true" />
          每周进展草稿
        </h3>
        <button
          type="button"
          className="button secondary"
          disabled={busy}
          onClick={() => void generate()}
        >
          {busy ? "正在汇总…" : "生成近 7 天草稿"}
        </button>
      </div>
      <p className="inline-help">
        基于本工作区近 7 天会话摘要、新增生效记忆与全局「关于我」画像在本地直接汇总，不消耗模型额度。
      </p>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {draft && (
        <div style={{ marginTop: "8px", display: "flex", flexDirection: "column", gap: "8px" }}>
          <div className="task-line">
            <span className="inline-help">
              汇总了 {draft.episodeCount} 段会话与 {draft.knowledgeCount} 条生效记忆
            </span>
            <button
              type="button"
              className="button text-button"
              onClick={() => {
                void navigator.clipboard
                  .writeText(draft.markdown)
                  .then(() => notify("周报草稿已复制"))
                  .catch(() => setError("无法自动复制，请手动选择下方文本。"));
              }}
            >
              <Copy size={14} aria-hidden="true" />
              复制 Markdown
            </button>
          </div>
          <textarea
            aria-label="每周进展草稿内容"
            rows={10}
            value={draft.markdown}
            onChange={(e) => setDraft({ ...draft, markdown: e.target.value })}
          />
        </div>
      )}
    </section>
  );
}
