import { useEffect, useState } from "react";
import { Bot, Sparkles } from "lucide-react";
import { assistant, type AssistantDraft, type AssistantSettings } from "../api/assistant";
import { draftStore } from "../draft-store";
import { errorText } from "../frontend-error";
import { StatusLabel } from "./StatusLabel";
import { WebSearchSettings } from "./WebSearchSettings";
import { GlobalPersonaPanel } from "./GlobalPersonaPanel";
import { QuickBarSettingsPanel } from "./QuickBarSettingsPanel";

const assistantPresets: { name: string; tag: string; instructions: string }[] = [
  {
    name: "深度思考伙伴",
    tag: "拆解问题 · 追问盲点",
    instructions: "先一句话给出核心判断，再分点拆解关键假设与潜在盲点；遇到信息不足时先提出最关键的 1-2 个澄清问题，不敷衍迎合。",
  },
  {
    name: "精简技术搭档",
    tag: "代码优先 · 零废话",
    instructions: "直接给可运行的方案或代码，省略寒暄和基础概念科普；涉及架构取舍时用简短对比说明利弊与边界条件。",
  },
  {
    name: "犀利逻辑审稿人",
    tag: "挑刺 · 查漏洞",
    instructions: "以严格审稿人视角审视我的方案与文字：优先指出逻辑漏洞、隐含前提、边界失败场景和冗余表达，并给出可直接替换的改写建议。",
  },
  {
    name: "温和写作编辑",
    tag: "润色 · 保留原味",
    instructions: "帮我润色与结构化表达，保持我原本的语气与观点，消除冗余和公文腔，给出修改后的成稿并简述 2-3 处关键改动原因。",
  },
];

const styleChips = [
  "结论放最前面，先给答案再解释原因。",
  "回答保持精简，避免客套话和重复总结。",
  "遇到不确定的地方直说不知道，不要编造。",
  "给出方案时同时附上风险提示与下一步行动。",
];

const edits = draftStore<AssistantDraft>("assistant", value => {
  if (!value || typeof value !== "object") return false;
  const draft = value as Partial<AssistantDraft>;
  return typeof draft.name === "string" && typeof draft.instructions === "string";
});

function sameDraft(first: AssistantDraft, second: AssistantDraft) {
  return first.name === second.name && first.instructions === second.instructions;
}

export function AssistantSettingsPanel({ workspaceId, focusSearch }: { workspaceId: string; focusSearch?: boolean }) {
  const [settings, setSettings] = useState<AssistantSettings>();
  const [draft, setDraft] = useState(() => edits.get(workspaceId));
  const [refresh, setRefresh] = useState(0);
  const [loadError, setLoadError] = useState("");
  const [actionError, setActionError] = useState("");
  const [pending, setPending] = useState(false);
  const [notice, setNotice] = useState("");

  useEffect(() => {
    const controller = new AbortController();
    void assistant("read_assistant", { workspaceId }, controller.signal).then(next => {
      setSettings(next);
      setDraft(edits.get(workspaceId) ?? next.draft);
      setLoadError("");
    }).catch(error => { if (!controller.signal.aborted) setLoadError(errorText(error)); });
    return () => controller.abort();
  }, [workspaceId, refresh]);

  async function perform(action: () => Promise<void>, message: string, submitted?: AssistantDraft) {
    if (pending) return;
    setPending(true);
    setActionError("");
    setNotice("");
    try {
      await action();
      const local = edits.get(workspaceId);
      if (local && submitted && sameDraft(local, submitted)) edits.delete(workspaceId);
      setNotice(message);
      setSettings(undefined);
      setRefresh(value => value + 1);
    } catch (error) {
      setActionError(errorText(error));
    } finally {
      setPending(false);
    }
  }

  function change(next: AssistantDraft) {
    if (settings && sameDraft(next, settings.draft)) edits.delete(workspaceId);
    else edits.set(workspaceId, next);
    setDraft(next);
    setNotice("");
  }

  const active = settings?.versions.find(version => version.version === settings.activeVersion);
  const dirty = !!settings && !!draft && !sameDraft(draft, settings.draft);
  const valid = !!draft && [...draft.name.trim()].length <= 80 && [...draft.instructions].length <= 2000;
  const publishable = valid && !!draft?.name.trim() && !!draft.instructions.trim();

  return <>
    <div className="section-description">
      <h2 id="assistant-settings-title"><Bot size={18} aria-hidden="true" />我的助手</h2>
      <p>为当前工作区设置专属回答方式。本地对话始终生效；云端对话在开启工作区同步策略时同步生效。</p>
    </div>
    <div className="settings-fields assistant-settings-fields">
      <QuickBarSettingsPanel />
      <GlobalPersonaPanel />
      {loadError && <div className="form-error" role="alert">
        <p>{loadError}</p><button className="button secondary" type="button" onClick={() => {
          setLoadError("");
          setRefresh(value => value + 1);
        }}>重新读取</button>
      </div>}
      {actionError && <p className="form-error" role="alert">{actionError}</p>}
      <div className="inline-help" role="status">{notice}</div>
      {!settings && !loadError && !notice && <p role="status">正在读取助手设置…</p>}
      {settings && draft && <>
        <div className="settings-save assistant-current">
          <span>当前启用 <StatusLabel tone="success">{active ? `${active.name} · 版本 ${active.version}` : "内置默认"}</StatusLabel></span>
          {active && <button className="button secondary" type="button" disabled={pending}
            onClick={() => void perform(() => assistant("activate_assistant", { workspaceId, version: null }), "已恢复内置默认。")}>使用内置默认</button>}
        </div>
        <form aria-label="助手草稿" onSubmit={event => {
          event.preventDefault();
          if (dirty && valid) void perform(() => assistant("save_assistant_draft", { workspaceId, ...draft }), "草稿已保存。", draft);
        }}>
          <div className="field">
            <label style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
              <Sparkles size={14} aria-hidden="true" />
              快速套用角色分身（点击填入草稿，可继续微调）
            </label>
            <div style={{ display: "flex", flexWrap: "wrap", gap: "8px", marginTop: "4px" }}>
              {assistantPresets.map(preset => (
                <button
                  key={preset.name}
                  type="button"
                  className="button secondary"
                  disabled={pending}
                  title={preset.tag}
                  onClick={() => change({ name: preset.name, instructions: preset.instructions })}
                >
                  {preset.name}
                </button>
              ))}
            </div>
          </div>
          <div className="field">
            <label htmlFor="assistant-name">助手名称</label>
            <input id="assistant-name" value={draft.name} disabled={pending} onChange={event => change({ ...draft, name: event.target.value })} />
          </div>
          <div className="field">
            <label htmlFor="assistant-instructions">回答指令</label>
            <textarea id="assistant-instructions" rows={5} value={draft.instructions} disabled={pending}
              placeholder="例如：先列出风险，再说明进展，最后给出下一步。"
              onChange={event => change({ ...draft, instructions: event.target.value })} />
            <div style={{ display: "flex", flexWrap: "wrap", gap: "6px", marginTop: "6px" }}>
              {styleChips.map(chip => (
                <button
                  key={chip}
                  type="button"
                  className="button text-button"
                  disabled={pending || draft.instructions.includes(chip)}
                  onClick={() => {
                    const nextText = draft.instructions.trim()
                      ? `${draft.instructions.trim()}\n${chip}`
                      : chip;
                    change({ ...draft, instructions: nextText.slice(0, 2000) });
                  }}
                >
                  + {chip.replace(/。$/, "")}
                </button>
              ))}
            </div>
          </div>
          {!valid && <p className="form-error" role="alert">名称最多 80 个字符，指令最多 2000 个字符。</p>}
          <div className="settings-save">
            <button className="button primary" type="button" disabled={pending || !publishable}
              onClick={() => void perform(() => assistant("publish_assistant", { workspaceId, ...draft }), "新版本已发布并启用。", draft)}>发布并启用</button>
            <button className="button secondary" type="submit" disabled={pending || !dirty || !valid}>保存草稿</button>
            {dirty && <span className="inline-help">有未保存的更改</span>}
          </div>
        </form>
        <section aria-labelledby="assistant-versions-title">
          <h3 id="assistant-versions-title">发布版本</h3>
          {settings.versions.length === 0 ? <p>暂无发布版本。填写名称和指令后可发布。</p>
            : <ul className="task-list">{settings.versions.map(version => <li key={version.version}>
              <div className="task-line">
                <div><strong>{version.name}</strong><span className="task-module">版本 {version.version}</span></div>
                {active?.version === version.version ? <StatusLabel tone="success">使用中</StatusLabel>
                  : <button className="button secondary" type="button" aria-label={`启用版本 ${version.version}`} disabled={pending}
                    onClick={() => void perform(() => assistant("activate_assistant", { workspaceId, version: version.version }), `已启用版本 ${version.version}。`)}>启用</button>}
              </div>
              <details className="field-help">
                <summary>查看指令</summary>
                <p className="assistant-version-instructions">{version.instructions}</p>
              </details>
            </li>)}</ul>}
        </section>
      </>}
      <WebSearchSettings workspaceId={workspaceId} focusOnOpen={focusSearch && (!!settings || !!loadError)} />
    </div>
  </>;
}
