import { listen } from "@tauri-apps/api/event";
import {
  Bell,
  BookOpen,
  FileText,
  GitFork,
  MessageSquare,
  Newspaper,
  Plus,
  Sparkles,
} from "lucide-react";
import { useEffect, useState } from "react";
import { memoryApi, type DailyFocus } from "../api/memory";
import { navigate, navigationHash } from "../navigation";
import { preference, remember } from "../preferences";
import type { Studio } from "../studio";
import { BrandMark } from "./BrandMark";

const guidePreference = "welcome-skipped";

function greetingByTime(): string {
  const hour = new Date().getHours();
  if (hour >= 5 && hour < 12) return "早上好";
  if (hour >= 12 && hour < 18) return "下午好";
  return "晚上好";
}

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
  const [focus, setFocus] = useState<DailyFocus | null>(null);
  const fresh = studio.desktop && !studio.loading && studio.data?.workspaces.length === 0;
  const local = studio.checking
    ? "正在检测本机模型…"
    : !studio.health
      ? "本机模型未启动（可离线启动或直接连接云端模型）"
      : !studio.health.models.length
        ? "本机引擎已连接，尚未下载模型"
        : studio.health.models.some((model) => model.name === studio.settings.model)
          ? "本机模型已就绪"
          : "已找到本机模型，请选择要用的模型";

  useEffect(() => {
    const ws = studio.workspaceId;
    if (!studio.desktop || !ws) {
      setFocus(null);
      return;
    }
    let active = true;
    const load = () => {
      void memoryApi("read_daily_focus", { workspaceId: ws })
        .then((res) => {
          if (active && res.workspaceId === ws) setFocus(res);
        })
        .catch(() => {
          if (active) setFocus(null);
        });
    };
    load();
    const unlistenPromise = listen<{ workspaceId: string }>(
      "memory-candidates-updated",
      ({ payload }) => {
        if (active && payload.workspaceId === ws) load();
      },
    ).catch(() => undefined);
    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten?.()).catch(() => {});
    };
  }, [studio.desktop, studio.workspaceId, studio.conversations.length]);

  const starters = focus?.suggestedPrompts?.length
    ? focus.suggestedPrompts
    : [
        "记住我的工作偏好：回答尽量简洁，先给结论再展开细节",
        "总结当前工作区里已有的项目资料和关键决策",
        "每个工作日早上 9 点给我整理一份 AI 行业简报",
      ];

  const showFocusBoard = Boolean(
    studio.desktop &&
      studio.workspaceId &&
      focus &&
      focus.workspaceId === studio.workspaceId &&
      focus.hasActivity,
  );

  if (showFocusBoard && focus) {
    const greeting = greetingByTime();
    const hasEpisodes = focus.recentEpisodes.length > 0;
    const hasSourcesOrMemory = focus.recentSources.length > 0 || focus.pendingMemoryCount > 0;
    const hasBriefOrReminder = Boolean(focus.latestBrief) || focus.upcomingReminders.length > 0;
    const activeCardCount =
      Number(hasEpisodes) + Number(hasSourcesOrMemory) + Number(hasBriefOrReminder);

    const cardStyle: React.CSSProperties = {
      display: "flex",
      flexDirection: "column",
      gap: "8px",
      padding: "12px 14px",
      borderRadius: "var(--radius-control)",
      border: "1px solid var(--line)",
      background: "var(--surface)",
      textAlign: "left",
    };

    return (
      <div className="chat-empty daily-focus-empty" style={{ gap: "16px", padding: "18px 16px" }}>
        <div className="welcome-heading" style={{ gap: "6px" }}>
          <BrandMark className="cognitive-mark" size={52} />
          <h2 style={{ fontSize: "1.25rem", margin: 0 }}>
            {greeting}
            {focus.displayName ? `，${focus.displayName}` : ""}
          </h2>
          <p className="welcome-tagline" style={{ margin: 0, fontSize: "0.86rem" }}>
            {focus.workspaceName} · 已沉淀 {focus.knowledgeCount} 条有效知识
            {focus.pendingMemoryCount > 0 ? ` · ${focus.pendingMemoryCount} 条记忆待确认` : ""}
          </p>
        </div>

        <section
          aria-label="今日聚焦工作台"
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))",
            gap: "10px",
            width: "100%",
            maxWidth: "740px",
          }}
        >
          {hasEpisodes && (
            <div style={cardStyle}>
              <strong style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "0.85rem" }}>
                <MessageSquare size={15} aria-hidden="true" />
                接着上次聊
              </strong>
              <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
                {focus.recentEpisodes.map((ep) => (
                  <button
                    key={ep.conversationId}
                    type="button"
                    className="button secondary"
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "flex-start",
                      gap: "2px",
                      padding: "7px 10px",
                      height: "auto",
                      textAlign: "left",
                    }}
                    onClick={() => {
                      void studio.openConversation(focus.workspaceId, ep.conversationId);
                    }}
                  >
                    <span
                      style={{
                        fontWeight: 600,
                        fontSize: "0.83rem",
                        width: "100%",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {ep.title}
                    </span>
                    <small
                      style={{
                        color: "var(--muted)",
                        fontSize: "0.76rem",
                        lineHeight: 1.35,
                        display: "-webkit-box",
                        WebkitLineClamp: 2,
                        WebkitBoxOrient: "vertical",
                        overflow: "hidden",
                      }}
                    >
                      {ep.summary}
                    </small>
                  </button>
                ))}
              </div>
            </div>
          )}

          {hasSourcesOrMemory && (
            <div style={cardStyle}>
              <strong style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "0.85rem" }}>
                <FileText size={15} aria-hidden="true" />
                近期资料与待办记忆
              </strong>
              <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
                {focus.pendingMemoryCount > 0 && (
                  <button
                    type="button"
                    className="button secondary"
                    style={{
                      justifyContent: "flex-start",
                      gap: "6px",
                      padding: "6px 10px",
                      fontSize: "0.82rem",
                    }}
                    onClick={() => navigate("knowledge", { workspace: focus.workspaceId })}
                  >
                    <Sparkles size={14} aria-hidden="true" />
                    待确认新记忆（{focus.pendingMemoryCount} 条）
                  </button>
                )}
                {focus.recentSources.slice(0, 3).map((src) => (
                  <button
                    key={src.sourceId}
                    type="button"
                    className="button text-button"
                    style={{
                      justifyContent: "space-between",
                      padding: "4px 6px",
                      fontSize: "0.81rem",
                      width: "100%",
                    }}
                    onClick={() =>
                      navigate("knowledge", {
                        workspace: focus.workspaceId,
                        node: src.nodeId ?? undefined,
                      })
                    }
                  >
                    <span
                      style={{
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {src.name}
                    </span>
                    <small style={{ color: "var(--muted)", flexShrink: 0 }}>v{src.version}</small>
                  </button>
                ))}
              </div>
            </div>
          )}

          {hasBriefOrReminder && (
            <div style={cardStyle}>
              <strong style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "0.85rem" }}>
                {focus.latestBrief ? (
                  <Newspaper size={15} aria-hidden="true" />
                ) : (
                  <Bell size={15} aria-hidden="true" />
                )}
                今日简报与提醒
              </strong>
              {focus.latestBrief && (
                <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                  <span style={{ fontSize: "0.82rem", fontWeight: 600 }}>
                    {focus.latestBrief.title}
                  </span>
                  {focus.latestBrief.headlines.slice(0, 2).map((item, idx) => (
                    <small
                      key={idx}
                      style={{
                        color: "var(--muted)",
                        fontSize: "0.76rem",
                        lineHeight: 1.35,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      · {item.title}
                    </small>
                  ))}
                  <button
                    type="button"
                    className="button text-button"
                    style={{ alignSelf: "flex-start", padding: "2px 4px", fontSize: "0.78rem" }}
                    onClick={() => {
                      const brief = focus.latestBrief!;
                      const lines = brief.headlines.map((h) => `- ${h.title}`).join("\n");
                      studio.setDraft(
                        `请结合最新一期「${brief.title}」简报重点，分析与我当前项目的关系：\n${lines}`,
                      );
                      document.getElementById("message")?.focus();
                    }}
                  >
                    围绕本期简报深聊 →
                  </button>
                </div>
              )}
              {focus.upcomingReminders.slice(0, 2).map((rem) => (
                <small
                  key={rem.id}
                  style={{
                    color: "var(--muted)",
                    display: "inline-flex",
                    alignItems: "center",
                    gap: "5px",
                    fontSize: "0.78rem",
                  }}
                >
                  <Bell size={12} aria-hidden="true" />
                  {rem.nextDate} {rem.time} · {rem.title}
                </small>
              ))}
            </div>
          )}

          {activeCardCount < 2 && (
            <div style={cardStyle}>
              <strong style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "0.85rem" }}>
                <GitFork size={15} aria-hidden="true" />
                知识导图与探索
              </strong>
              <small style={{ color: "var(--muted)", fontSize: "0.79rem", lineHeight: 1.45 }}>
                打开 2D 思维导图框选多张卡片开启专题自测，或在知识页同步本地文档。
              </small>
              <div style={{ display: "flex", gap: "6px", flexWrap: "wrap", marginTop: "auto" }}>
                <button
                  type="button"
                  className="button secondary"
                  style={{ fontSize: "0.8rem", padding: "5px 10px" }}
                  onClick={() => navigate("graph", { workspace: focus.workspaceId })}
                >
                  <GitFork size={14} aria-hidden="true" />
                  2D 思维导图
                </button>
                <button
                  type="button"
                  className="button secondary"
                  style={{ fontSize: "0.8rem", padding: "5px 10px" }}
                  onClick={() => navigate("knowledge", { workspace: focus.workspaceId })}
                >
                  <BookOpen size={14} aria-hidden="true" />
                  知识库
                </button>
              </div>
            </div>
          )}
        </section>

        <div className="welcome-next" style={{ width: "100%", maxWidth: "740px" }}>
          {!modelReady ? (
            <a
              className="button secondary"
              href={navigationHash("settings", { section: cloudAnswer ? "cloud" : "model" })}
            >
              {cloudAnswer ? "配置云端模型" : "配置本机或云端模型"}
            </a>
          ) : (
            <section className="first-run" aria-label="推荐追问">
              <div className="welcome-starters">
                {starters.map((prompt) => (
                  <button
                    key={prompt}
                    type="button"
                    className="button secondary starter-chip"
                    onClick={() => {
                      studio.setDraft(prompt);
                      document.getElementById("message")?.focus();
                    }}
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
              <a className="button secondary" href={navigationHash("knowledge")}>
                预览知识库
              </a>
              <a
                className="button secondary"
                href={navigationHash("settings", { section: "assistant" })}
              >
                预览助手与模型设置
              </a>
            </div>
          </section>
        ) : fresh && !skipped ? (
          <section className="first-run" aria-label="首次使用">
            <p>在一个工作区里对话、沉淀长期记忆、同步本地资料并随时切换云/地模型。</p>
            <strong role="status">{local}</strong>
            <div className="first-run-actions">
              <button className="button primary" onClick={() => onCreateWorkspace("chat")}>
                <Plus size={16} aria-hidden="true" />
                用本机模型开始
              </button>
              <button className="button secondary" onClick={() => onCreateWorkspace("cloud")}>
                连接云端模型
              </button>
            </div>
            <div className="first-run-actions">
              <a className="button text-button" href={navigationHash("settings", { section: "model" })}>
                查看本机连接
              </a>
              <a className="button text-button" href={navigationHash("settings", { section: "data" })}>
                从备份恢复
              </a>
              <button
                className="button text-button"
                onClick={() => {
                  remember(guidePreference, "true");
                  setSkipped(true);
                }}
              >
                跳过引导
              </button>
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
          <a
            className="button secondary"
            href={navigationHash("settings", { section: cloudAnswer ? "cloud" : "model" })}
          >
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
                  onClick={() => {
                    studio.setDraft(prompt);
                    document.getElementById("message")?.focus();
                  }}
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
