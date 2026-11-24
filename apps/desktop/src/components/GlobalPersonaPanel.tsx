import { useEffect, useState } from "react";
import { Plus, Trash2, User } from "lucide-react";
import { memoryApi, type GlobalPersona } from "../api/memory";
import { errorText } from "../frontend-error";
import { notify } from "./Toast";

const emptyPersona: GlobalPersona = {
  displayName: "",
  roleBackground: "",
  communicationStyle: "",
  preferences: [],
  rules: [],
  updatedAt: "",
};

export function GlobalPersonaPanel() {
  const [persona, setPersona] = useState<GlobalPersona>(emptyPersona);
  const [savedSnapshot, setSavedSnapshot] = useState<string>("");
  const [newPref, setNewPref] = useState("");
  const [newRule, setNewRule] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let current = true;
    void memoryApi("get_global_persona", undefined)
      .then((data) => {
        if (!current) return;
        setPersona(data);
        setSavedSnapshot(JSON.stringify(data));
      })
      .catch((e) => {
        if (current) setError(errorText(e));
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, []);

  const dirty = !loading && JSON.stringify(persona) !== savedSnapshot;

  async function save(next: GlobalPersona = persona) {
    if (saving) return;
    setSaving(true);
    setError("");
    try {
      const updated = await memoryApi("save_global_persona", { input: next });
      setPersona(updated);
      setSavedSnapshot(JSON.stringify(updated));
      notify("个人画像已保存，跨工作区生效");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="settings-fields" aria-labelledby="global-persona-title">
      <div className="section-description">
        <h3 id="global-persona-title">
          <User size={17} aria-hidden="true" />
          关于我（全局个人画像）
        </h3>
        <p>跨所有工作区生效。日常对话中确认的个人偏好与长期规则也会自动沉淀到这里。</p>
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {loading ? (
        <p role="status">正在读取个人画像…</p>
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <div className="field">
            <label htmlFor="persona-name">希望 AI 如何称呼你</label>
            <input
              id="persona-name"
              maxLength={60}
              value={persona.displayName}
              disabled={saving}
              placeholder="例如：阿明 / Alex"
              onChange={(e) => setPersona({ ...persona, displayName: e.target.value })}
            />
          </div>
          <div className="field">
            <label htmlFor="persona-background">身份与常用技术/领域背景</label>
            <textarea
              id="persona-background"
              rows={3}
              maxLength={800}
              value={persona.roleBackground}
              disabled={saving}
              placeholder="例如：独立软件开发者，熟悉 TypeScript / Rust，正在做本地 AI 桌面应用。"
              onChange={(e) => setPersona({ ...persona, roleBackground: e.target.value })}
            />
          </div>
          <div className="field">
            <label htmlFor="persona-style">偏好表达风格</label>
            <textarea
              id="persona-style"
              rows={2}
              maxLength={800}
              value={persona.communicationStyle}
              disabled={saving}
              placeholder="例如：结论先行、简洁直白、避免客套，代码示例默认给完整可运行片段。"
              onChange={(e) => setPersona({ ...persona, communicationStyle: e.target.value })}
            />
          </div>

          <div className="field">
            <label>已记住的个人偏好（{persona.preferences.length}）</label>
            <ul className="task-list">
              {persona.preferences.map((item, idx) => (
                <li key={`${item}-${idx}`} className="task-line">
                  <span>{item}</span>
                  <button
                    type="button"
                    className="icon-button"
                    aria-label="删除偏好"
                    disabled={saving}
                    onClick={() =>
                      setPersona({
                        ...persona,
                        preferences: persona.preferences.filter((_, i) => i !== idx),
                      })
                    }
                  >
                    <Trash2 size={14} aria-hidden="true" />
                  </button>
                </li>
              ))}
            </ul>
            <div className="settings-save">
              <input
                aria-label="新增个人偏好"
                maxLength={240}
                value={newPref}
                disabled={saving}
                placeholder="手动补充一条个人习惯或偏好…"
                onChange={(e) => setNewPref(e.target.value)}
              />
              <button
                type="button"
                className="button secondary"
                disabled={saving || !newPref.trim()}
                onClick={() => {
                  const text = newPref.trim();
                  if (!text) return;
                  setPersona({
                    ...persona,
                    preferences: persona.preferences.includes(text)
                      ? persona.preferences
                      : [...persona.preferences, text],
                  });
                  setNewPref("");
                }}
              >
                <Plus size={14} aria-hidden="true" />
                添加
              </button>
            </div>
          </div>

          <div className="field">
            <label>必须遵守的长期规则（{persona.rules.length}）</label>
            <ul className="task-list">
              {persona.rules.map((item, idx) => (
                <li key={`${item}-${idx}`} className="task-line">
                  <span>{item}</span>
                  <button
                    type="button"
                    className="icon-button"
                    aria-label="删除长期规则"
                    disabled={saving}
                    onClick={() =>
                      setPersona({
                        ...persona,
                        rules: persona.rules.filter((_, i) => i !== idx),
                      })
                    }
                  >
                    <Trash2 size={14} aria-hidden="true" />
                  </button>
                </li>
              ))}
            </ul>
            <div className="settings-save">
              <input
                aria-label="新增长期规则"
                maxLength={240}
                value={newRule}
                disabled={saving}
                placeholder="例如：不要使用公文腔 / 每次先列风险再给方案…"
                onChange={(e) => setNewRule(e.target.value)}
              />
              <button
                type="button"
                className="button secondary"
                disabled={saving || !newRule.trim()}
                onClick={() => {
                  const text = newRule.trim();
                  if (!text) return;
                  setPersona({
                    ...persona,
                    rules: persona.rules.includes(text)
                      ? persona.rules
                      : [...persona.rules, text],
                  });
                  setNewRule("");
                }}
              >
                <Plus size={14} aria-hidden="true" />
                添加
              </button>
            </div>
          </div>

          <div className="settings-save">
            <button className="button primary" type="submit" disabled={saving || !dirty}>
              {saving ? "正在保存…" : "保存个人画像"}
            </button>
            {dirty && <span className="inline-help">有未保存的更改</span>}
          </div>
        </form>
      )}
    </section>
  );
}
