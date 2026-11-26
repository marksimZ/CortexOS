import { SlidersHorizontal, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { errorText } from "../frontend-error";
import type { Studio } from "../studio";
import { defaultSettings } from "../studio-settings";
import { capacityError, validModelCapacity } from "./NumberField";
import { StatusLabel } from "./StatusLabel";
import { notify } from "./Toast";

interface GenerationPreset {
  id: string;
  label: string;
  description: string;
  temperature: number;
  topP: number;
}

export const GENERATION_PRESETS: GenerationPreset[] = [
  {
    id: "precise",
    label: "严谨准确",
    description: "适合查资料、总结与精准问答",
    temperature: 0.2,
    topP: 0.85,
  },
  {
    id: "balanced",
    label: "均衡日常",
    description: "适合日常交流与通用写作",
    temperature: 0.7,
    topP: 0.9,
  },
  {
    id: "creative",
    label: "灵感发散",
    description: "适合头脑风暴、创意改写与发散",
    temperature: 1.1,
    topP: 0.95,
  },
];

function matchesPreset(temperature: number, topP: number, preset: GenerationPreset): boolean {
  return Math.abs(temperature - preset.temperature) < 0.001 && Math.abs(topP - preset.topP) < 0.001;
}

export function activePresetLabel(temperature: number, topP: number): string {
  const found = GENERATION_PRESETS.find((preset) => matchesPreset(temperature, topP, preset));
  return found ? found.label : "自定义";
}

export function GenerationStylePanel({
  studio,
  compact = false,
}: {
  studio: Studio;
  compact?: boolean;
}) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [tempDraft, setTempDraft] = useState(studio.settings.temperature);
  const [topPDraft, setTopPDraft] = useState(studio.settings.topP);
  const [numPredictDraft, setNumPredictDraft] = useState(studio.settings.numPredict);

  useEffect(() => {
    setTempDraft(studio.settings.temperature);
    setTopPDraft(studio.settings.topP);
    setNumPredictDraft(studio.settings.numPredict);
  }, [studio.settings.temperature, studio.settings.topP, studio.settings.numPredict]);

  const currentPreset = GENERATION_PRESETS.find((preset) =>
    matchesPreset(studio.settings.temperature, studio.settings.topP, preset),
  );
  const numPredictValid =
    Number.isInteger(numPredictDraft) &&
    numPredictDraft >= 32 &&
    numPredictDraft <= 2048 &&
    validModelCapacity(studio.settings.numCtx, numPredictDraft);
  const advancedDirty =
    Math.abs(tempDraft - studio.settings.temperature) >= 0.001 ||
    Math.abs(topPDraft - studio.settings.topP) >= 0.001 ||
    numPredictDraft !== studio.settings.numPredict;

  const commit = async (next: Partial<Studio["settings"]>, successMessage: string) => {
    if (!studio.desktop || saving) return;
    setSaving(true);
    setError("");
    try {
      await studio.saveSettings({
        ...studio.settings,
        ...next,
      });
      notify(successMessage);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <section
      className={compact ? "generation-style-panel compact" : "settings-card generation-style-panel"}
      aria-label="回答风格与生成控制"
    >
      <div className="generation-style-header">
        <strong>
          <Sparkles size={15} aria-hidden="true" />
          回答风格与生成控制
        </strong>
        <StatusLabel tone={currentPreset ? "neutral" : "warning"}>
          {currentPreset ? currentPreset.label : "自定义参数"}
        </StatusLabel>
      </div>
      {!compact && (
        <p className="generation-style-subtitle">
          选择预设即可一键调节回答严谨度或创造力；云端模型与本地 Ollama 会同步使用当前设置。
        </p>
      )}
      <div className="generation-preset-grid" role="group" aria-label="回答风格预设">
        {GENERATION_PRESETS.map((preset) => {
          const active = matchesPreset(studio.settings.temperature, studio.settings.topP, preset);
          return (
            <button
              key={preset.id}
              type="button"
              className={`generation-preset-button${active ? " active" : ""}`}
              aria-pressed={active}
              disabled={!studio.desktop || saving}
              title={`温度 ${preset.temperature} / 核采样 ${preset.topP}`}
              onClick={() =>
                void commit(
                  { temperature: preset.temperature, topP: preset.topP },
                  `已切换为「${preset.label}」风格`,
                )
              }
            >
              <span className="generation-preset-title">{preset.label}</span>
              <span className="generation-preset-desc">{preset.description}</span>
            </button>
          );
        })}
      </div>

      <label className="generation-thinking-toggle">
        <input
          type="checkbox"
          checked={studio.settings.deepThinking}
          disabled={!studio.desktop || saving}
          onChange={(event) =>
            void commit(
              { deepThinking: event.target.checked },
              event.target.checked ? "已开启深度思考" : "已关闭深度思考",
            )
          }
        />
        <span>
          <strong>开启深度思考</strong>
          <small>为支持推理的模型发送思考指令（不兼容模型可随时关闭）。</small>
        </span>
      </label>

      <details className="advanced-settings generation-advanced">
        <summary>
          <SlidersHorizontal size={14} aria-hidden="true" />
          高级生成参数微调
        </summary>
        <div className="generation-slider-list">
          <div className="generation-slider-row">
            <div className="generation-slider-label">
              <label htmlFor={compact ? "compact-temp-slider" : "settings-temp-slider"}>
                温度（Temperature）
              </label>
              <span>{tempDraft.toFixed(2)}</span>
            </div>
            <input
              id={compact ? "compact-temp-slider" : "settings-temp-slider"}
              type="range"
              min={0}
              max={2}
              step={0.05}
              value={tempDraft}
              disabled={!studio.desktop || saving}
              onChange={(event) => setTempDraft(Number(event.target.value))}
            />
          </div>

          <div className="generation-slider-row">
            <div className="generation-slider-label">
              <label htmlFor={compact ? "compact-topp-slider" : "settings-topp-slider"}>
                核采样（Top P）
              </label>
              <span>{topPDraft.toFixed(2)}</span>
            </div>
            <input
              id={compact ? "compact-topp-slider" : "settings-topp-slider"}
              type="range"
              min={0.1}
              max={1}
              step={0.05}
              value={topPDraft}
              disabled={!studio.desktop || saving}
              onChange={(event) => setTopPDraft(Number(event.target.value))}
            />
          </div>

          <div className="generation-slider-row">
            <div className="generation-slider-label">
              <label htmlFor={compact ? "compact-predict-input" : "settings-predict-input"}>
                回答长度上限（Token）
              </label>
              <span>32–2048</span>
            </div>
            <input
              id={compact ? "compact-predict-input" : "settings-predict-input"}
              type="number"
              min={32}
              max={2048}
              step={1}
              value={Number.isFinite(numPredictDraft) ? numPredictDraft : ""}
              disabled={!studio.desktop || saving}
              onChange={(event) => setNumPredictDraft(Number(event.target.value))}
            />
            {!numPredictValid && (
              <p className="form-error" role="alert">
                {numPredictDraft >= studio.settings.numCtx
                  ? capacityError
                  : "回答长度上限需为 32 到 2048 之间的整数。"}
              </p>
            )}
          </div>
        </div>
        <p className="inline-help">
          云端与本机统一生效；Anthropic 会自动将温度限制在 0–1 且省略 Top P。
        </p>
        <div className="generation-advanced-actions">
          <button
            type="button"
            className="button primary"
            disabled={!studio.desktop || saving || !advancedDirty || !numPredictValid}
            onClick={() =>
              void commit(
                {
                  temperature: Number(tempDraft.toFixed(2)),
                  topP: Number(topPDraft.toFixed(2)),
                  numPredict: numPredictDraft,
                },
                "已保存高级生成参数",
              )
            }
          >
            {saving ? "保存中…" : "保存高级参数"}
          </button>
          <button
            type="button"
            className="button secondary"
            disabled={!studio.desktop || saving}
            onClick={() => {
              setTempDraft(defaultSettings.temperature);
              setTopPDraft(defaultSettings.topP);
              setNumPredictDraft(defaultSettings.numPredict);
              void commit(
                {
                  temperature: defaultSettings.temperature,
                  topP: defaultSettings.topP,
                  numPredict: defaultSettings.numPredict,
                  deepThinking: defaultSettings.deepThinking,
                },
                "已恢复默认生成参数",
              );
            }}
          >
            恢复默认
          </button>
        </div>
      </details>

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
