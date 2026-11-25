import { Command, ExternalLink } from "lucide-react";
import { useEffect, useState } from "react";
import { quickbarApi, type QuickbarStatus } from "../api/quickbar";
import { errorText } from "../frontend-error";
import { StatusLabel } from "./StatusLabel";
import { notify } from "./Toast";

export function QuickBarSettingsPanel() {
  const [status, setStatus] = useState<QuickbarStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    void quickbarApi("quickbar_status")
      .then((res) => {
        if (active) setStatus(res);
      })
      .catch((err) => {
        if (active) setError(errorText(err));
      });
    return () => {
      active = false;
    };
  }, []);

  const toggleEnabled = async (enabled: boolean) => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const next = await quickbarApi("set_quickbar_enabled", { enabled });
      setStatus(next);
      notify(enabled ? "已开启 Alt+Space 全局呼出" : "已关闭全局快捷键");
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section
      className="settings-card"
      aria-labelledby="quickbar-settings-heading"
      style={{
        padding: "14px 16px",
        borderRadius: "var(--radius-control)",
        border: "1px solid var(--line)",
        background: "var(--surface)",
        display: "flex",
        flexDirection: "column",
        gap: "10px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap" }}>
        <strong id="quickbar-settings-heading" style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <Command size={16} aria-hidden="true" />
          全局随手记与秒问浮窗（Alt + Space）
        </strong>
        {status && (
          <StatusLabel
            tone={!status.enabled ? "neutral" : status.registered ? "success" : "warning"}
          >
            {!status.enabled
              ? "已停用"
              : status.registered
                ? `${status.shortcut} 已就绪`
                : "快捷键冲突"}
          </StatusLabel>
        )}
      </div>
      <p style={{ margin: 0, fontSize: "0.85rem", color: "var(--muted)", lineHeight: 1.5 }}>
        在任何软件中按 <code>Alt + Space</code> 秒级呼出置顶悬浮条：直接向当前工作区提问，或按 <code>Tab</code>（或输入 <code>/</code>）切换「随手记」，回车 0.1 秒直接沉淀进本地知识库。
      </p>
      {status?.error && (
        <p className="form-error" role="status" style={{ margin: 0 }}>
          {status.error}
        </p>
      )}
      {error && (
        <p className="form-error" role="alert" style={{ margin: 0 }}>
          {error}
        </p>
      )}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "10px", flexWrap: "wrap" }}>
        <label className="retrieval-switch">
          <input
            type="checkbox"
            checked={status?.enabled ?? true}
            disabled={busy || !status}
            onChange={(event) => void toggleEnabled(event.target.checked)}
          />
          启用 <code>Alt + Space</code> 全局呼出热键
        </label>
        <button
          type="button"
          className="button secondary"
          disabled={busy}
          onClick={() => {
            void quickbarApi("toggle_quickbar").catch((err) => setError(errorText(err)));
          }}
        >
          <ExternalLink size={14} aria-hidden="true" />
          立即呼出浮窗
        </button>
      </div>
    </section>
  );
}
