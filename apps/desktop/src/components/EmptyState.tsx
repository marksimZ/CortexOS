import { BrandMark } from "./BrandMark";
import { useState } from "react";
import { Plus } from "lucide-react";
import type { Studio } from "../studio";
import { preference, remember } from "../preferences";
import { navigationHash } from "../navigation";

const guidePreference = "welcome-skipped";

export function EmptyState({
  studio,
  modelReady,
  cloudAnswer,
  onCreateWorkspace,
}: {
  studio: Studio;
  modelReady: boolean | undefined;
  cloudAnswer: boolean;
  onCreateWorkspace: (next?: "chat" | "cloud") => void;
}) {
  const [skipped, setSkipped] = useState(() => preference(guidePreference) === "true");
  const fresh = studio.desktop && !studio.loading && studio.data?.workspaces.length === 0;
  const local = studio.checking ? "正在检查 Ollama…"
    : !studio.health ? "Ollama 未连接"
    : !studio.health.models.length ? "Ollama 已连接，尚无模型"
    : studio.health.models.some(model => model.name === studio.settings.model) ? "本机模型已就绪"
    : "已找到本机模型，请选择要用的模型";

  return (
    <div className="chat-empty">
      <div className="welcome-heading">
        <BrandMark className="cognitive-mark" size={80} />
        <h2>欢迎使用 CortexOS</h2>
      </div>
      <div className="welcome-next">
        {!studio.desktop ? (
          <p>在桌面程序中开始，浏览器仅供界面预览。</p>
        ) : fresh && !skipped ? (
          <section className="first-run" aria-label="首次使用">
            <p>在一个工作区里对话、整理知识并查看回答依据。</p>
            <strong role="status">{local}</strong>
            <div className="first-run-actions">
              <button className="button primary" onClick={() => onCreateWorkspace("chat")}>
                <Plus size={16} aria-hidden="true" />用本机模型
              </button>
              <button className="button secondary" onClick={() => onCreateWorkspace("cloud")}>
                连接云端模型
              </button>
            </div>
            <div className="first-run-actions">
              <a className="button text-button" href={navigationHash("settings", { section: "model" })}>查看本机连接</a>
              <a className="button text-button" href={navigationHash("settings", { section: "data" })}>从备份恢复</a>
              <button className="button text-button" onClick={() => {
                remember(guidePreference, "true");
                setSkipped(true);
              }}>跳过引导</button>
            </div>
          </section>
        ) : !studio.workspaceId ? (
          <button
            className="button primary"
            disabled={!studio.data || studio.loading}
            onClick={() => onCreateWorkspace("chat")}
          >
            <Plus size={16} aria-hidden="true" />
            创建工作区
          </button>
        ) : !modelReady ? (
          <a className="button secondary" href={navigationHash("settings", { section: cloudAnswer ? "cloud" : "model" })}>
            {cloudAnswer ? "配置云端连接" : "配置模型连接"}
          </a>
        ) : null}
      </div>
    </div>
  );
}
