import { BrandMark } from "./BrandMark";
import { useState } from "react";
import { CornerDownRight, Plus } from "lucide-react";
import type { Studio } from "../studio";
import { preference, remember } from "../preferences";
import { navigationHash } from "../navigation";

const guidePreference = "welcome-skipped";
// Both examples are handled by the built-in reminder and scheduled-brief flows.
const suggestions = ["十分钟后提醒我喝水", "每个工作日早上九点整理 AI 新闻"];

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
  const local = studio.checking ? "正在检测本机模型…"
    : !studio.health ? "本机模型未启动（可离线启动或直接连接云端模型）"
    : !studio.health.models.length ? "本机引擎已连接，尚未下载模型"
    : studio.health.models.some(model => model.name === studio.settings.model) ? "本机模型已就绪"
    : "已找到本机模型，请选择要用的模型";

  const starters = [
    "记住我的工作偏好：回答尽量简洁，先给结论再展开细节",
    "总结当前工作区里已有的项目资料和关键决策",
    "每个工作日早上 9 点给我整理一份 AI 行业简报",
  ];

  return (
    <div className="chat-empty">
      <div className="welcome-heading">
        <BrandMark className="cognitive-mark" size={80} />
        <h2>欢迎使用 CortexOS</h2>
        <p className="welcome-tagline">
          定制你的专属个人 AI：记住你的偏好、本地资料与长期规则，越用越懂你。
        </p>
      </div>
      <div className="welcome-next">
        {!studio.desktop ? (
          <section className="first-run" aria-label="浏览器预览说明">
            <p>当前为浏览器界面预览模式；启动桌面端程序后即可创建工作区、导入本地资料并开始对话。</p>
            <div className="first-run-actions">
              <a className="button secondary" href={navigationHash("knowledge")}>预览知识库</a>
              <a className="button secondary" href={navigationHash("settings", { section: "assistant" })}>预览助手与模型设置</a>
            </div>
          </section>
        ) : fresh && !skipped ? (
          <section className="first-run" aria-label="首次使用">
            <p>在一个工作区里对话、沉淀长期记忆、同步本地资料并随时切换云/地模型。</p>
            <strong role="status">{local}</strong>
            <div className="first-run-actions">
              <button className="button primary" onClick={() => onCreateWorkspace("chat")}>
                <Plus size={16} aria-hidden="true" />用本机模型开始
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
            {cloudAnswer ? "配置云端模型" : "配置本机或云端模型"}
          </a>
        ) : (
          <section className="first-run" aria-label="快速开始示例">
            <div className="welcome-starters">
              {starters.map((prompt) => (
                <button
                  key={prompt}
                  type="button"
                  className="button secondary starter-chip"
                  onClick={() => studio.setDraft(prompt)}
                >
                  {prompt}
                </button>
              ))}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
