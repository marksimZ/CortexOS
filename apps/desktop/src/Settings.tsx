import { capacityError, NumberField, validModelCapacity } from "./components/NumberField";
import { SelectMenu } from "./components/SelectMenu";
import { useEffect, useState } from "react";
import {
  CheckCircle2,
  Database,
  Info,
  SlidersHorizontal,
  Unplug,
} from "lucide-react";
import { errorText, type Studio } from "./studio";
import { notify } from "./components/Toast";
import { RagIndexSettings } from "./components/RagIndexSettings";
import { LocalRuntimeSettings } from "./components/LocalRuntimeSettings";
import { CloudRuntimeSettings } from "./components/CloudRuntimeSettings";
import { RuntimeProfilesSettings } from "./components/RuntimeProfilesSettings";
import { RuntimeResources } from "./components/RuntimeResources";
import { StorageInventory } from "./components/StorageInventory";
import { DataPortability } from "./components/DataPortability";
import { LocalDiagnostics } from "./components/LocalDiagnostics";
import { AppUpdates } from "./components/AppUpdates";
import { AssistantSettingsPanel } from "./components/AssistantSettingsPanel";
import { navigate, useNavigation } from "./navigation";

const sections = [
  ["assistant", "我的助手"],
  ["model", "模型连接"],
  ["cloud", "云端模型"],
  ["profiles", "运行方案"],
  ["usage", "资源与用量"],
  ["data", "本地数据"],
  ["about", "关于"],
] as const;

export function SettingsPage({ studio }: { studio: Studio }) {
  const draft = studio.settingsDraft;
  const setDraft = studio.setSettingsDraft;
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [capacityInvalid, setCapacityInvalid] = useState(false);
  const route = useNavigation();
  const section = sections.find(([id]) => id === route.params.get("section"))?.[0] ?? "model";
  useEffect(() => {
    const active = document.activeElement;
    if (active instanceof HTMLButtonElement && active.closest(".settings-navigation") && active.getAttribute("aria-pressed") !== "true") {
      active.blur();
    }
  }, [section]);
  const installed = studio.health?.models.some(
    (model) => model.name === draft.model,
  );
  const changed = JSON.stringify(draft) !== JSON.stringify(studio.settings);
  const field = (
    name: "numCtx" | "numPredict" | "temperature" | "topP" | "timeoutSeconds",
    value: number | null,
  ) => {
    setCapacityInvalid(false);
    setDraft((current) => ({ ...current, [name]: value ?? NaN }));
  };
  return (
    <section className="standard-page" aria-labelledby="page-title">
      <h1 id="page-title" className="sr-only">设置</h1>
      <div className="page-content settings-content">
        <nav className="settings-navigation" aria-label="设置分类">
          {sections.map(([id, label]) => (
            <button
              key={id}
              className="button text-button"
              aria-pressed={section === id}
              aria-controls={`${id}-section`}
              onClick={() => navigate("settings", { section: id, workspace: route.workspace })}
            >
              {label}
            </button>
          ))}
        </nav>
        <section id="assistant-section" hidden={section !== "assistant"} className="settings-section" aria-labelledby="assistant-settings-title">
          {section === "assistant" && (studio.desktop && studio.workspaceId
            ? <AssistantSettingsPanel key={studio.workspaceId} studio={studio} workspaceId={studio.workspaceId} focusSearch={route.params.get("focus") === "web-search"} />
            : <div className="section-description"><h2 id="assistant-settings-title">我的助手</h2>
              <p>{studio.desktop ? "请先选择工作区。" : "请在桌面版中管理个人助手。"}</p></div>)}
        </section>
        <section
          id="model-section"
          hidden={section !== "model"}
          className="settings-section"
          aria-labelledby="model-settings"
        >
          <div className="section-description">
            <h2 id="model-settings">
              <SlidersHorizontal size={18} aria-hidden="true" />
              模型连接
            </h2>
          </div>
          <div className="settings-fields">
          <form
            className="settings-fields"
            onSubmit={(event) => {
              event.preventDefault();
              if (!validModelCapacity(draft.numCtx, draft.numPredict)) {
                setCapacityInvalid(true);
                const parameters = event.currentTarget.querySelector<HTMLDetailsElement>(".advanced-settings");
                if (parameters) { parameters.open = true; parameters.querySelector("input")?.focus(); }
                return;
              }
              setSaving(true);
              setError(null);
              void studio
                .saveSettings(draft)
                .then(() => notify("设置已保存"))
                .catch((value) => setError(errorText(value)))
                .finally(() => setSaving(false));
            }}
          >
            <div
              className={`connection-status ${studio.health ? "connected" : ""}`}
            >
              {studio.health ? (
                <CheckCircle2 size={18} aria-hidden="true" />
              ) : (
                <Unplug size={18} aria-hidden="true" />
              )}
              <div>
                <strong>
                  {!studio.desktop
                    ? "浏览器仅供界面预览"
                    : studio.checking
                      ? "正在检测本地服务…"
                      : studio.health
                        ? `Ollama ${studio.health.version} 已连接`
                        : "未连接本地 Ollama"}
                </strong>
                {studio.health && !studio.health.models.length && (
                  <p>服务可用，尚未安装模型。</p>
                )}
              </div>
            </div>
            <div className="field">
              <label htmlFor="endpoint">服务地址（固定本机回环）</label>
              <div className="field-inline-action">
                <input id="endpoint" value="http://127.0.0.1:11434" disabled />
                <button
                  type="button"
                  className="button secondary"
                  disabled={!studio.desktop || studio.checking}
                  aria-busy={studio.checking}
                  onClick={() => {
                    void studio.checkConnection();
                  }}
                >
                  {studio.checking ? "正在检测…" : "检测连接"}
                </button>
              </div>
            </div>
            {studio.connectionError && (
              <p className="form-error" role="alert">
                {studio.connectionError}
              </p>
            )}
            {studio.desktop &&
              (!studio.health || !studio.health.models.length) && (
                <div className="setup-instructions">
                  <h3>准备本地模型</h3>
                  <p>安装并启动 Ollama 后，在终端下载模型：</p>
                  <code>ollama pull qwen3:4b-instruct</code>
                  <p>
                    下载完成后，点击“检测连接”。模型下载需要联网，对话在本机运行。
                  </p>
                </div>
              )}
            <div className="field model-field">
              <label htmlFor="model">模型</label>
              <SelectMenu
                id="model"
                value={draft.model}
                disabled={!studio.desktop || !studio.health?.models.length}
                onChange={(event) => {
                  setDraft((current) => ({
                    ...current,
                    model: event.target.value,
                  }));
                }}
              >
                {!installed && (
                  <option value={draft.model}>{draft.model}（未安装）</option>
                )}
                {studio.health?.models.map((model) => (
                  <option key={model.name} value={model.name}>
                    {model.name}（{model.quantization ?? "压缩方式未知"}，
                    {(model.size / 1e9).toFixed(1)} GB）
                  </option>
                ))}
              </SelectMenu>
              <details className="field-help">
                <summary>如何选择模型</summary>
                <p>
                  建议
                  qwen3:4b-instruct。带思考功能的模型可能需要更高的回答长度上限。更改从下一次回答开始使用。
                </p>
              </details>
            </div>
            <details className="advanced-settings">
              <summary>生成参数</summary>
              <div className="parameter-fields">
                <NumberField label="输入与回答总容量（Token）" min={1024} max={16384} value={draft.numCtx} readOnly={!studio.desktop} disabled={!studio.desktop} error={capacityInvalid ? capacityError : undefined} onChange={value => field("numCtx", value)} />
                <NumberField label="回答长度上限（Token）" min={32} max={2048} value={draft.numPredict} readOnly={!studio.desktop} disabled={!studio.desktop} onChange={value => field("numPredict", value)} />
                <NumberField label="回答变化程度（温度）" min={0} max={2} step="any" value={draft.temperature} readOnly={!studio.desktop} disabled={!studio.desktop} onChange={value => field("temperature", value)} />
                <NumberField label="核采样（top_p）" min={0.1} max={1} step={0.05} value={draft.topP} readOnly={!studio.desktop} disabled={!studio.desktop} onChange={value => field("topP", value)} />
                <NumberField label="生成超时（秒）" min={30} max={600} value={draft.timeoutSeconds} readOnly={!studio.desktop} disabled={!studio.desktop} onChange={value => field("timeoutSeconds", value)} />
              </div>
              <label className="generation-thinking-toggle">
                <input
                  type="checkbox"
                  checked={draft.deepThinking}
                  disabled={!studio.desktop}
                  onChange={event => {
                    const deepThinking = event.target.checked;
                    setDraft(current => ({ ...current, deepThinking }));
                  }}
                />
                <span>
                  <strong>开启深度思考</strong>
                  <small>向支持的推理模型发送思考开关指令（云端与本地 Ollama 均生效）。</small>
                </span>
              </label>
              <p>
                一次只生成一个回答。Token
                是模型计算文字长度的单位，不等于字数。总容量包含说明、资料、对话与回答，需为回答留出空间；容量越大，通常越占显卡内存。“温度”越高，回答的措辞和选取通常越多变，不代表更正确。
              </p>
            </details>
            {error && (
              <p className="form-error" role="alert">
                {error}
              </p>
            )}
            <div className="settings-save">
              <button
                className="button primary"
                disabled={!studio.desktop || saving || !changed}
              >
                {saving ? "正在保存…" : "保存设置"}
              </button>
              <span className="inline-help" role="status">
                {changed ? "有未保存的更改" : ""}
              </span>
            </div>
          </form>
            {studio.desktop && studio.workspaceId && (
              <div className="settings-subsections">
                <LocalRuntimeSettings key={`runtime:${studio.workspaceId}`} workspaceId={studio.workspaceId} model={draft.model} running={!!studio.activeRun || studio.sending} />
                <RagIndexSettings key={`index:${studio.workspaceId}`} workspaceId={studio.workspaceId} />
              </div>
            )}
          </div>
        </section>
        <section
          id="cloud-section"
          hidden={section !== "cloud"}
          className="settings-section"
          aria-labelledby="runtime-cloud-settings"
        >
          {section === "cloud" && (studio.desktop && studio.workspaceId
            ? <CloudRuntimeSettings key={studio.workspaceId} workspaceId={studio.workspaceId} />
            : <div className="section-description"><h2 id="runtime-cloud-settings">云端连接</h2>
              <p>{studio.desktop ? "请先选择工作区。" : "请在桌面版中管理云端连接。"}</p></div>)}
        </section>
        <section
          id="profiles-section"
          hidden={section !== "profiles"}
          className="settings-section"
          aria-labelledby="runtime-profiles-settings"
        >
          {section === "profiles" && (studio.desktop && studio.workspaceId
            ? <RuntimeProfilesSettings key={studio.workspaceId} workspaceId={studio.workspaceId} localModels={studio.health?.models.map(model => model.name) ?? []} />
            : <div className="section-description"><h2 id="runtime-profiles-settings">运行方案</h2>
              <p>{studio.desktop ? "请先选择工作区。" : "请在桌面版中管理运行方案。"}</p></div>)}
        </section>
        <section id="usage-section" hidden={section !== "usage"} className="settings-section" aria-labelledby="runtime-usage-settings">
          {section === "usage" && (studio.desktop && studio.workspaceId
            ? <RuntimeResources key={studio.workspaceId} workspaceId={studio.workspaceId} />
            : <div className="section-description"><h2 id="runtime-usage-settings">资源与用量</h2>
              <p>{studio.desktop ? "请先选择工作区。" : "请在桌面版中查看资源与用量。"}</p></div>)}
        </section>
        <section
          id="data-section"
          hidden={section !== "data"}
          className="settings-section"
          aria-labelledby="data-settings"
        >
          <div className="section-description">
            <h2 id="data-settings">
              <Database size={18} aria-hidden="true" />
              本地数据
            </h2>
          </div>
          <div className="settings-fields">
            <dl className="data-summary">
              <div>
                <dt>工作区与会话保存</dt>
                <dd>
                  {studio.desktop && studio.data
                    ? "自动保存在本机数据库"
                    : "桌面程序中可用"}
                </dd>
              </div>
              <div>
                <dt>文件访问</dt>
                <dd>导入或恢复时，只读取你主动选择的文件</dd>
              </div>
              <div>
                <dt>升级备份</dt>
                <dd>数据库版本升级前自动创建</dd>
              </div>
            </dl>
            {studio.data?.dataPath && (
              <p className="data-path">
                <strong>数据位置</strong>
                <br />
                {studio.data.dataPath}
              </p>
            )}
            {studio.desktop && section === "data" && <DataPortability workspaceId={studio.workspaceId} />}
            {studio.desktop && <StorageInventory />}
            <details className="field-help">
              <summary>隐私与存储说明</summary>
              <p>
                添加文件时，由你选择原始文件，应用在本机保存副本。使用云端模型时，发送前可核对传给云端的内容。
              </p>
            </details>
          </div>
        </section>
        <section
          id="about-section"
          hidden={section !== "about"}
          className="settings-section"
          aria-labelledby="about-settings"
        >
          <div className="section-description">
            <h2 id="about-settings">
              <Info size={18} aria-hidden="true" />
              关于
            </h2>
          </div>
          <div className="settings-fields">
            <div className="about-line">
              <strong>CortexOS</strong>
              <span className="version-label">本地认知工作台</span>
            </div>
            {studio.desktop && section === "about" && <AppUpdates />}
            {studio.desktop && section === "about" && <LocalDiagnostics />}
          </div>
        </section>
      </div>
    </section>
  );
}
