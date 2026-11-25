import {
  Eye,
  Globe,
  HardDrive,
  ImagePlus,
  Pencil,
  Send,
  Square,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { MarkdownContent } from "./MarkdownContent";
import { ContextPicker } from "../ContextPicker";
import { navigationHash } from "../navigation";
import { webSearchProviderLabel, type WebSearchProvider } from "../api/web-search";
import { assistant, type AssistantSettings } from "../api/assistant";
import { AppDialog, openDialog } from "./AppDialog";
import { notify } from "./Toast";
import { cloudPresetForAddress } from "../cloud-presets";
import { SelectMenu } from "./SelectMenu";
import type { MessageImage, Studio } from "../studio";
import { cloudConnectionLabel, type GenerationTarget, type RuntimeConnection } from "../runtime-profiles";

const ALLOWED_IMAGE_TYPES = new Set<MessageImage["mimeType"]>([
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/gif",
]);
const MAX_IMAGES = 4;
const MAX_DIMENSION = 1568;
const MAX_BASE64_LENGTH = 2_700_000;

function readDataUrl(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === "string") resolve(reader.result);
      else reject(new Error("无法读取图片数据。"));
    };
    reader.onerror = () => reject(new Error("读取图片失败。"));
    reader.readAsDataURL(file);
  });
}

function parseDataUrl(dataUrl: string): MessageImage {
  const match = /^data:(image\/(?:png|jpeg|webp|gif));base64,([A-Za-z0-9+/=]+)$/.exec(dataUrl.trim());
  if (!match) {
    throw new Error("不支持的图片编码格式，请使用 PNG、JPEG、WebP 或 GIF 图片。");
  }
  const mimeType = match[1] as MessageImage["mimeType"];
  const dataBase64 = match[2];
  if (!dataBase64 || dataBase64.length > MAX_BASE64_LENGTH) {
    throw new Error("单张图片过大（压缩后仍超过约 2 MB），请裁剪后重试。");
  }
  return { mimeType, dataBase64 };
}

async function encodeImageFile(file: File): Promise<MessageImage> {
  const rawType = file.type as MessageImage["mimeType"];
  if (!ALLOWED_IMAGE_TYPES.has(rawType)) {
    throw new Error("仅支持粘贴或上传 PNG、JPG、WebP、GIF 格式图片。");
  }
  if (rawType === "image/gif") {
    return parseDataUrl(await readDataUrl(file));
  }
  if (file.size <= 350 * 1024 && (rawType === "image/png" || rawType === "image/webp" || rawType === "image/jpeg")) {
    return parseDataUrl(await readDataUrl(file));
  }
  const objectUrl = URL.createObjectURL(file);
  try {
    const img = await new Promise<HTMLImageElement>((resolve, reject) => {
      const element = new Image();
      element.onload = () => resolve(element);
      element.onerror = () => reject(new Error("图片解码失败，请确认图片未损坏。"));
      element.src = objectUrl;
    });
    const maxSide = Math.max(img.naturalWidth || 1, img.naturalHeight || 1);
    const scale = maxSide > MAX_DIMENSION ? MAX_DIMENSION / maxSide : 1;
    const width = Math.max(1, Math.round((img.naturalWidth || 1) * scale));
    const height = Math.max(1, Math.round((img.naturalHeight || 1) * scale));
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      return parseDataUrl(await readDataUrl(file));
    }
    ctx.drawImage(img, 0, 0, width, height);
    let dataUrl = canvas.toDataURL("image/webp", 0.86);
    if (!dataUrl.startsWith("data:image/webp;base64,") || dataUrl.length > MAX_BASE64_LENGTH) {
      dataUrl = canvas.toDataURL("image/jpeg", 0.84);
    }
    return parseDataUrl(dataUrl);
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}

export function ChatComposer({
  studio,
  modelName,
  generationTarget,
  chooseGeneration,
  connections,
  modelError,
  retryModels,
  cloudAnswer,
  cloudLookup,
  cloudSyncDefault,
  currentRunning,
  preparing,
  cancelling,
  setCancelling,
  canSend,
  taskDraft,
  draftTooLong,
  searchConfigured,
  retrySearchStatus,
  searching,
  searchWeb,
  send,
}: {
  studio: Studio;
  modelName: string;
  generationTarget?: GenerationTarget;
  chooseGeneration: (target?: GenerationTarget) => void;
  connections: RuntimeConnection[];
  modelError?: string;
  retryModels: () => void;
  cloudAnswer: boolean;
  cloudLookup: boolean;
  cloudSyncDefault: boolean;
  currentRunning: boolean;
  preparing: boolean;
  cancelling: boolean;
  setCancelling: (value: boolean) => void;
  canSend: boolean;
  taskDraft: "briefing" | "reminder" | null;
  draftTooLong: boolean;
  searchConfigured: boolean | null | undefined;
  retrySearchStatus: () => void;
  searching: boolean;
  searchWeb: (provider: WebSearchProvider) => void;
  send: (content?: string) => Promise<void>;
}) {
  const [previewKey, setPreviewKey] = useState<string | null>(null);
  const [assistantState, setAssistantState] = useState<AssistantSettings | null>(null);
  const [switchingAssistant, setSwitchingAssistant] = useState(false);
  const [previewAttachment, setPreviewAttachment] = useState<MessageImage | null>(null);
  const attachmentDialog = useRef<HTMLDialogElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const taskPreview = taskDraft === "briefing" ? "预览新闻简报" : taskDraft === "reminder" ? "预览提醒" : undefined;
  const [searchChoice, setSearchChoice] = useState<{ workspace: string; provider: WebSearchProvider }>();
  const provider = searchConfigured
    ? (searchChoice?.workspace === studio.workspaceId ? searchChoice?.provider : undefined) ?? "brave"
    : "news";
  const scope = `${studio.workspaceId}:${studio.conversationId}`;
  const selectedModel = generationTarget ? JSON.stringify(generationTarget) : "";
  const localModels = studio.health?.models.map(({ name }) => {
    const target: GenerationTarget = { kind: "local", model: name };
    return { target, label: name, key: JSON.stringify(target) };
  }) ?? [];
  const cloudModels = connections.flatMap(connection => {
    if (connection.credentialAvailable === false || connection.catalogState === "invalid_credential") return [];
    const seen = new Set<string>();
    const items: { target: GenerationTarget; label: string; key: string }[] = [];
    const connLabel = cloudConnectionLabel(connection);
    const pushModel = (model: string) => {
      if (!model || seen.has(model)) return;
      seen.add(model);
      const target: GenerationTarget = { kind: "cloud", connectionId: connection.id, model };
      items.push({ target, label: model + " · " + connLabel, key: JSON.stringify(target) });
    };
    for (const probe of connection.probes) {
      if (probe.capability === "generation" && probe.state === "verified") pushModel(probe.model);
    }
    for (const item of connection.models ?? []) {
      if (item.generation !== "unsupported") pushModel(item.id);
    }
    if (items.length === 0) {
      const preset = connection.provider === "compatible" ? cloudPresetForAddress(connection.address) : undefined;
      for (const fallback of preset?.defaultModels ?? []) pushModel(fallback);
    }
    return items;
  });
  const options = [...localModels, ...cloudModels];
  const preview = previewKey === scope;
  const textarea = useRef<HTMLTextAreaElement>(null);
  const previewArea = useRef<HTMLDivElement>(null);
  const previousPreview = useRef(false);
  const composing = useRef(false);
  const compositionEndedAt = useRef(-Infinity);

  useEffect(() => {
    if (previousPreview.current !== preview) {
      (preview ? previewArea.current : textarea.current)?.focus();
      previousPreview.current = preview;
    }
  }, [preview]);

  useEffect(() => {
    const ws = studio.workspaceId;
    if (!studio.desktop || !ws) {
      setAssistantState(null);
      return;
    }
    let active = true;
    void assistant("read_assistant", { workspaceId: ws })
      .then((res) => { if (active) setAssistantState(res); })
      .catch(() => { if (active) setAssistantState(null); });
    return () => { active = false; };
  }, [studio.desktop, studio.workspaceId]);

  const attachFiles = async (files: File[]) => {
    if (!files.length) return;
    const remaining = MAX_IMAGES - studio.draftImages.length;
    if (remaining <= 0) {
      notify(`单条消息最多添加 ${MAX_IMAGES} 张图片`);
      return;
    }
    const chosen = files.slice(0, remaining);
    if (files.length > remaining) {
      notify(`已选取前 ${remaining} 张图片（单条最多 ${MAX_IMAGES} 张）`);
    }
    try {
      const encoded = await Promise.all(chosen.map(encodeImageFile));
      studio.setDraftImages([...studio.draftImages, ...encoded]);
    } catch (err) {
      studio.reportError(err);
    }
  };

  return (
    <div className="composer-area">
      <div className="composer">
        {studio.draftImages.length > 0 && (
          <div
            className="composer-image-strip"
            aria-label="待发送图片附件"
            style={{
              display: "flex",
              flexWrap: "wrap",
              gap: "8px",
              padding: "4px 4px 8px",
              borderBottom: "1px solid var(--line)",
              marginBottom: "6px",
            }}
          >
            {studio.draftImages.map((img, idx) => (
              <div
                key={idx}
                style={{
                  position: "relative",
                  width: "56px",
                  height: "56px",
                  borderRadius: "8px",
                  overflow: "hidden",
                  border: "1px solid var(--line)",
                  background: "var(--surface-inset, var(--surface))",
                }}
              >
                <button
                  type="button"
                  onClick={() => {
                    setPreviewAttachment(img);
                    requestAnimationFrame(() => openDialog(attachmentDialog.current));
                  }}
                  aria-label={`预览待发送图片 ${idx + 1}`}
                  title="点击放大预览"
                  style={{
                    padding: 0,
                    border: "none",
                    width: "100%",
                    height: "100%",
                    cursor: "zoom-in",
                    background: "transparent",
                  }}
                >
                  <img
                    src={`data:${img.mimeType};base64,${img.dataBase64}`}
                    alt={`待发送图片 ${idx + 1}`}
                    style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
                  />
                </button>
                <button
                  type="button"
                  aria-label={`移除第 ${idx + 1} 张图片`}
                  title="移除图片"
                  disabled={studio.sending}
                  onClick={() => {
                    studio.setDraftImages(studio.draftImages.filter((_, i) => i !== idx));
                  }}
                  style={{
                    position: "absolute",
                    top: "2px",
                    right: "2px",
                    width: "18px",
                    height: "18px",
                    borderRadius: "999px",
                    border: "none",
                    background: "rgba(0, 0, 0, 0.68)",
                    color: "#fff",
                    display: "inline-flex",
                    alignItems: "center",
                    justifyContent: "center",
                    cursor: "pointer",
                    padding: 0,
                  }}
                >
                  <X size={12} aria-hidden="true" />
                </button>
              </div>
            ))}
          </div>
        )}
        <label className="sr-only" htmlFor="message">
          你的问题
        </label>
        <textarea
          ref={textarea}
          hidden={preview}
          id="message"
          rows={1}
          aria-invalid={draftTooLong || undefined}
          aria-describedby={draftTooLong ? "message-length-error" : undefined}
          value={studio.draft}
          onChange={(event) => studio.setDraft(event.target.value)}
          onPaste={(event) => {
            if (!studio.desktop || !studio.workspaceId || studio.sending) return;
            const imageItems = Array.from(event.clipboardData?.items ?? []).filter(
              (item) => item.kind === "file" && item.type.startsWith("image/"),
            );
            if (!imageItems.length) return;
            const files = imageItems
              .map((item) => item.getAsFile())
              .filter((file): file is File => file !== null);
            if (files.length > 0) {
              event.preventDefault();
              void attachFiles(files);
            }
          }}
          onCompositionStart={() => { composing.current = true; }}
          onCompositionEnd={() => {
            composing.current = false;
            compositionEndedAt.current = performance.now();
          }}
          placeholder={
            studio.workspaceId
              ? studio.draftImages.length > 0
                ? "补充关于图片的问题，或直接按 Enter 发送…"
                : `向 ${studio.assistantDisplayName} 提问（支持 Ctrl+V 直接粘贴截图）…`
              : "创建工作区后，在这里输入问题…"
          }
          disabled={!studio.desktop || !studio.workspaceId}
          readOnly={studio.sending || studio.creatingConversation || studio.loadingMessages}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              // Let the IME consume its own Enter; suppress only the subsequent send.
              if (composing.current || event.nativeEvent.isComposing || event.keyCode === 229) return;
              event.preventDefault();
              if (!event.repeat && performance.now() - compositionEndedAt.current >= 500 && canSend) void send();
            }
          }}
        />
        {preview && (
          <div
            ref={previewArea}
            className="composer-preview markdown-content"
            tabIndex={0}
            role="region"
            aria-label="Markdown 预览"
          >
            {studio.draft ? (
              <MarkdownContent content={studio.draft} />
            ) : (
              <p>输入内容后可预览排版。</p>
            )}
          </div>
        )}
        <div className="composer-toolbar">
          <div className="composer-model">
            <SelectMenu
              aria-label="回答模型"
              title={modelName}
              value={selectedModel}
              disabled={!studio.desktop || !studio.workspaceId || !!studio.activeRun || currentRunning || studio.sending}
              onChange={event => chooseGeneration(options.find(item => item.key === event.target.value)?.target)}
            >
              <option value="">{generationTarget ? "自动" : `自动 · ${modelName}`}</option>
              {selectedModel && !options.some(item => item.key === selectedModel) &&
                <option value={selectedModel} disabled>{generationTarget?.model}（不可用）</option>}
              {cloudModels.length > 0 && <optgroup label="云端模型">
                {cloudModels.map(item => <option key={item.key} value={item.key}>{item.label}</option>)}
              </optgroup>}
              {localModels.length > 0 && <optgroup label="本机模型">
                {localModels.map(item => <option key={item.key} value={item.key}>{item.label}</option>)}
              </optgroup>}
            </SelectMenu>
            {studio.desktop && studio.workspaceId && assistantState && assistantState.versions.length > 0 && (
              <SelectMenu
                aria-label="助手分身"
                title="快速切换当前工作区的助手分身"
                value={assistantState.activeVersion === null ? "" : String(assistantState.activeVersion)}
                disabled={switchingAssistant || !!studio.activeRun || currentRunning || studio.sending}
                onChange={event => {
                  const ws = studio.workspaceId;
                  if (!ws) return;
                  const nextVersion = event.target.value === "" ? null : Number(event.target.value);
                  setSwitchingAssistant(true);
                  void assistant("activate_assistant", { workspaceId: ws, version: nextVersion })
                    .then(() => {
                      setAssistantState(prev => prev ? { ...prev, activeVersion: nextVersion } : prev);
                      const label = nextVersion === null
                        ? studio.assistantDisplayName
                        : assistantState.versions.find(v => v.version === nextVersion)?.name || `分身 v${nextVersion}`;
                      notify(`已切换助手分身：${label}`);
                    })
                    .catch(studio.reportError)
                    .finally(() => setSwitchingAssistant(false));
                }}
              >
                <option value="">{studio.assistantDisplayName}（默认）</option>
                {assistantState.versions.map(v => (
                  <option key={v.version} value={String(v.version)}>
                    {v.name || `分身 v${v.version}`}
                  </option>
                ))}
              </SelectMenu>
            )}
          </div>
          <div className="composer-actions">
            {studio.desktop && studio.workspaceId && <button
              type="button"
              className="button text-button"
              disabled={searchConfigured === undefined || !studio.draft.trim() || currentRunning || studio.sending || studio.loadingMessages || searching}
              aria-describedby={studio.draft.trim() ? "web-query-disclosure" : undefined}
              title={`将本题发送给${webSearchProviderLabel[provider]}`}
              onClick={() => { setPreviewKey(null); searchWeb(provider); }}
            ><Globe size={16} aria-hidden="true" />{searchConfigured === null ? "查询公开新闻" : "联网查询"}</button>}
            <input
              ref={fileInputRef}
              type="file"
              accept="image/png,image/jpeg,image/webp,image/gif"
              multiple
              hidden
              onChange={(event) => {
                const files = Array.from(event.target.files ?? []);
                event.target.value = "";
                void attachFiles(files);
              }}
            />
            <button
              type="button"
              className="icon-button"
              aria-label="添加图片附件"
              title="添加图片（也可在输入框按 Ctrl+V 直接粘贴截图）"
              disabled={
                !studio.desktop ||
                !studio.workspaceId ||
                studio.sending ||
                studio.loadingMessages ||
                studio.draftImages.length >= MAX_IMAGES
              }
              onClick={() => fileInputRef.current?.click()}
            >
              <ImagePlus size={17} aria-hidden="true" />
            </button>
            <button
              type="button"
              className="icon-button"
              aria-label={preview ? "返回 Markdown 编辑" : "预览 Markdown"}
              title={preview ? "返回编辑" : "预览 Markdown"}
              aria-pressed={preview}
              disabled={
                !studio.desktop ||
                !studio.workspaceId ||
                studio.sending ||
                studio.loadingMessages
              }
              onClick={() => setPreviewKey(preview ? null : scope)}
            >
              {preview ? (
                <Pencil size={17} aria-hidden="true" />
              ) : (
                <Eye size={17} aria-hidden="true" />
              )}
            </button>
            {currentRunning ? (
              <button
                className="button secondary stop-button"
                disabled={cancelling}
                onClick={() => {
                  setCancelling(true);
                  void studio.cancel().catch((value) => {
                    setCancelling(false);
                    studio.reportError(value);
                  });
                }}
              >
                <Square size={13} aria-hidden="true" />
                {cancelling ? "正在停止…" : preparing ? "停止查找" : "停止生成"}
              </button>
            ) : (
              <button
                className="send-button"
                disabled={!canSend}
                onClick={() => {
                  if (!canSend) return;
                  setPreviewKey(null);
                  if (!preview) textarea.current?.focus({ preventScroll: true });
                  void send();
                }}
                aria-label={studio.sending ? "正在发送" : taskPreview ?? "发送消息"}
                title={canSend ? (taskPreview ?? "发送消息（Enter）") : "请先选择可用模型并输入问题或粘贴图片"}
              >
                <Send size={19} aria-hidden="true" />
              </button>
            )}
          </div>
        </div>
      </div>
      <div className="composer-context">
        {searchConfigured && studio.workspaceId && <SelectMenu aria-label="查询服务" value={provider} disabled={searching}
          onChange={event => {
            if (studio.workspaceId) setSearchChoice({
              workspace: studio.workspaceId, provider: event.target.value === "brave" ? "brave" : "news",
            });
          }}>
          <option value="news">{webSearchProviderLabel.news}</option>
          <option value="brave">{webSearchProviderLabel.brave}</option>
        </SelectMenu>}
        {studio.desktop && studio.workspaceId && (!cloudAnswer || cloudSyncDefault) && (
          <label className="retrieval-switch">
            <input
              type="checkbox"
              checked={studio.autoRetrieval}
              disabled={
                studio.savingRetrieval ||
                studio.sending ||
                !!studio.activeRun ||
                studio.loadingMessages ||
                studio.creatingConversation
              }
              onChange={(event) => {
                void studio.setAutoRetrieval(event.target.checked);
              }}
            />
            {studio.savingRetrieval ? "正在保存…" : "自动查找知识"}
          </label>
        )}
        {!cloudAnswer && <span className="local-label"><HardDrive size={15} aria-hidden="true" />本地</span>}
        {(cloudAnswer || cloudLookup) && <span>{cloudAnswer ? "云端回答" : "云端查找"}</span>}
        <ContextPicker
          key={`${scope}:${selectedModel}`}
          studio={studio}
          cloudAnswer={cloudAnswer}
          generationTarget={generationTarget}
        />
      </div>
      {draftTooLong && <small className="form-error" id="message-length-error">问题超过输入上限。请缩短后再发送。</small>}
      {modelError && <div className="composer-context">
        <small className="form-error" role="status">{modelError}</small>
        <button className="button text-button" type="button" onClick={retryModels}>重新读取模型</button>
      </div>}
      {searchConfigured !== undefined && !!studio.draft.trim() && <small className="inline-help" id="web-query-disclosure">
        联网查询会将本题发送给{webSearchProviderLabel[provider]}。
      </small>}
      {searchConfigured === null && <div className="composer-context">
        <small className="form-error" role="status">无法读取查询服务配置，可显式选择查询公开新闻。</small>
        <button className="button text-button" type="button" onClick={retrySearchStatus}>重新读取配置</button>
      </div>}
      {!!studio.draftPersistence && <small className="inline-help">{studio.draftPersistence === "local" ? "草稿保存在本机" : "草稿只保留在当前窗口"}</small>}
      <details className="composer-help">
        <summary title="展开查看对话说明与常用快捷键">对话说明与快捷键</summary>
        <div className="composer-help-popover">
          <p>
            Enter 发送，Shift+Enter 换行，Ctrl+V 可直接粘贴截图（需多模态模型支持）。在回答或资料文件中划选文字，可一键「追问这段」或「存为知识卡」。
            支持输入“十分钟后提醒我喝水”创建提醒，或“每个工作日早上九点整理 AI 新闻”创建定时简报。
            {cloudAnswer
              ? cloudSyncDefault ? "云端回答会按当前同步设置使用对话与记忆。自动查找知识可在此关闭。" : "发送前可确认要传给云端的内容。"
              : "系统会在容量允许的范围内选取最近的完整对话，并查找当前工作区已确认的知识；不会直接搜索文件全文。请结合依据判断回答是否可靠。"}
          </p>
          {studio.workspaceId && <a href={navigationHash("settings", { workspace: studio.workspaceId, section: cloudAnswer ? "profiles" : "model" })}>模型设置</a>}
          {!studio.desktop && <p>浏览器预览不会读取或保存本地数据。</p>}
        </div>
      </details>
      <AppDialog
        ref={attachmentDialog}
        className="form-dialog"
        aria-label="预览待发送图片"
        onClose={() => setPreviewAttachment(null)}
        style={{ maxWidth: "min(90vw, 900px)", padding: "12px" }}
      >
        {previewAttachment && (
          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <strong style={{ fontSize: "0.9rem" }}>待发送图片预览</strong>
              <button
                type="button"
                className="icon-button"
                aria-label="关闭图片预览"
                onClick={() => attachmentDialog.current?.close()}
              >
                <X size={18} aria-hidden="true" />
              </button>
            </div>
            <img
              src={`data:${previewAttachment.mimeType};base64,${previewAttachment.dataBase64}`}
              alt="待发送图片预览"
              style={{
                maxWidth: "100%",
                maxHeight: "76vh",
                objectFit: "contain",
                borderRadius: "8px",
                margin: "0 auto",
              }}
            />
          </div>
        )}
      </AppDialog>
    </div>
  );
}
