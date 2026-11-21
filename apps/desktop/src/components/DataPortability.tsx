import { useEffect, useRef, useState } from "react";
import {
  portability,
  type BackupHistory,
  type BackupPreview,
  type OpenBackup,
  type RestoreStatus,
} from "../api/portability";
import { capturePortableUi, portabilityStartupMessage } from "../portable-ui";
import { errorText } from "../studio";
import { AppDialog, openDialog } from "./AppDialog";
import { SelectMenu } from "./SelectMenu";

type View =
  | { kind: "save"; scope: string | null; preview: BackupPreview }
  | { kind: "open"; preview: OpenBackup };

const historyLabels = {
  backup: "已保存备份",
  restore: "已恢复数据",
  restore_failed: "恢复未完成",
};
const size = (bytes: number) =>
  new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 1 }).format(
    bytes / 1024 ** 2,
  ) + " MiB";
const time = (date: string) => new Date(date).toLocaleString("zh-CN");
const names = (workspaces: { name: string }[]) =>
  workspaces.map((item) => item.name).join("、") || "无工作区";

export function DataPortability({
  workspaceId,
}: {
  workspaceId: string | null;
}) {
  const [scope, setScope] = useState("all");
  const [format, setFormat] = useState<"json" | "markdown">("json");
  const [view, setView] = useState<View | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState(portabilityStartupMessage);
  const [confirmed, setConfirmed] = useState(false);
  const [status, setStatus] = useState<RestoreStatus | null>(null);
  const [history, setHistory] = useState<BackupHistory[]>([]);
  const dialog = useRef<HTMLDialogElement>(null);
  const focusReturn = useRef<Element | null>(null);
  const selected = scope === "workspace" ? workspaceId : null;

  async function refresh() {
    const [next, records] = await Promise.all([
      portability("restore_on_start"),
      portability("backup_history"),
    ]);
    setStatus(next);
    setHistory(records);
  }

  useEffect(() => {
    let active = true;
    void Promise.all([
      portability("restore_on_start"),
      portability("backup_history"),
    ])
      .then(([next, records]) => {
        if (!active) return;
        setStatus(next);
        setHistory(records);
      })
      .catch((cause) => {
        if (active) setError(errorText(cause));
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (view) openDialog(dialog.current, () => focusReturn.current);
  }, [view]);

  async function run(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await action();
    } catch (cause) {
      setError(errorText(cause));
    } finally {
      setBusy(false);
    }
  }

  function show(next: View) {
    focusReturn.current = document.activeElement;
    setConfirmed(false);
    setView(next);
  }

  const restore = view?.kind === "open" ? view.preview : null;
  const preview = view?.kind === "save" ? view.preview : null;
  const blocked =
    restore?.pending ||
    (!!restore?.manifest.workspaceId && !!restore?.conflicts.length);

  return (
    <section
      className="storage-preview"
      aria-labelledby="data-portability-title"
      aria-busy={busy}
    >
      <h3 id="data-portability-title">备份与恢复</h3>
      <div className="cloud-actions">
        <SelectMenu
          aria-label="备份范围"
          value={scope}
          disabled={busy}
          onChange={(event) => setScope(event.target.value)}
        >
          <option value="all">整个应用</option>
          <option value="workspace" disabled={!workspaceId}>
            当前工作区
          </option>
        </SelectMenu>
        <button
          type="button"
          className="button secondary"
          disabled={busy || (scope === "workspace" && !selected)}
          onClick={() =>
            void run(async () =>
              show({
                kind: "save",
                scope: selected,
                preview: await portability("backup_preview", {
                  workspaceId: selected,
                }),
              }),
            )
          }
        >
          预览备份
        </button>
        <button
          type="button"
          className="button secondary"
          disabled={
            busy || status?.phase === "pending" || status?.phase === "apply_ui"
          }
          onClick={() =>
            void run(async () => {
              const opened = await portability("backup_open");
              if (opened) show({ kind: "open", preview: opened });
            })
          }
        >
          打开备份…
        </button>
      </div>
      {status?.phase === "pending" && (
        <div className="storage-preview">
          <p role="status">
            恢复已预约。请保存其他编辑，关闭并重新打开 CortexOS。
          </p>
          <button
            type="button"
            className="button secondary"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                await portability("restore_cancel");
                await refresh();
                setMessage("已撤销恢复预约。");
              })
            }
          >
            撤销恢复预约
          </button>
        </div>
      )}
      {status?.phase === "apply_ui" && (
        <p role="alert">
          数据库已恢复。请重新打开 CortexOS，完成草稿与界面偏好恢复。
        </p>
      )}
      {status?.message && <p role="alert">{status.message}</p>}
      {busy && <p role="status">正在处理，请稍候。</p>}
      {message && <p role="status">{message}</p>}
      {error && !view && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {!!history.length && (
        <details className="field-help">
          <summary>最近备份与恢复</summary>
          <ul className="runtime-usage-list">
            {history.map((entry) => (
              <li key={entry.id}>
                <details className="field-help">
                  <summary>
                    {historyLabels[entry.kind]}{" "}
                    <time dateTime={entry.at}>{time(entry.at)}</time>
                  </summary>
                  <p>工作区 {names(entry.workspaces)}</p>
                  <p className="data-path">{entry.path}</p>
                  {entry.safetyPath && (
                    <p className="data-path">恢复前备份 {entry.safetyPath}</p>
                  )}
                  {entry.detail && <p>{entry.detail}</p>}
                </details>
              </li>
            ))}
          </ul>
        </details>
      )}
      <details className="field-help">
        <summary>导出当前工作区知识</summary>
        <div className="cloud-actions">
          <SelectMenu
            aria-label="知识导出格式"
            value={format}
            disabled={busy}
            onChange={(event) =>
              setFormat(event.target.value === "json" ? "json" : "markdown")
            }
          >
            <option value="json">JSON</option>
            <option value="markdown">Markdown</option>
          </SelectMenu>
          <button
            type="button"
            className="button secondary"
            disabled={busy || !workspaceId}
            onClick={() =>
              void run(async () => {
                if (
                  workspaceId &&
                  (await portability("export_knowledge", {
                    workspaceId,
                    format,
                  }))
                )
                  setMessage("知识已导出。");
              })
            }
          >
            导出知识…
          </button>
        </div>
      </details>

      <AppDialog
        ref={dialog}
        className="form-dialog"
        aria-labelledby="portability-dialog-title"
        onCancel={(event) => {
          if (busy) event.preventDefault();
        }}
        onClose={() => {
          setView(null);
          setError("");
        }}
      >
        <h2 id="portability-dialog-title">
          {restore ? "检查备份" : "备份预览"}
        </h2>
        {preview && (
          <div>
            <p>工作区 {names(preview.workspaces)}</p>
            <p>
              包括{preview.includes.join("、")}。请妥善保管备份中的私有内容。
            </p>
            <details className="field-help">
              <summary>查看空间与恢复所需环境</summary>
              <p>
                原数据库大小 {size(preview.estimatedDatabaseBytes)}
                。备份的大小以保存结果为准。
              </p>
              {preview.availableBytes !== null && (
                <p>数据盘可用 {size(preview.availableBytes)}</p>
              )}
              <ul>
                {preview.dependencies.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            </details>
          </div>
        )}
        {restore && (
          <div>
            <p>
              {restore.manifest.workspaceId
                ? "导入此工作区，当前其他工作区会保留。"
                : "用备份替换整个应用的数据。重启时会先保存一份当前数据的安全备份。"}
            </p>
            <dl className="data-summary">
              <div>
                <dt>工作区</dt>
                <dd>{names(restore.manifest.workspaces)}</dd>
              </div>
              <div>
                <dt>备份时间</dt>
                <dd>{time(restore.manifest.createdAt)}</dd>
              </div>
              <div>
                <dt>完整性</dt>
                <dd>文件与 SHA-256 校验通过</dd>
              </div>
            </dl>
            <details className="field-help">
              <summary>查看备份清单</summary>
              <p>
                应用版本 {restore.manifest.appVersion}，数据库版本{" "}
                {restore.manifest.schemaVersion}，备份格式{" "}
                {restore.manifest.formatVersion}
              </p>
              {restore.availableBytes !== null && (
                <p>数据盘可用 {size(restore.availableBytes)}</p>
              )}
              <ul>
                {restore.manifest.files.map((file) => (
                  <li key={file.name} className="data-path">
                    {file.name}，{size(file.bytes)}
                    <br />
                    SHA-256 {file.sha256}
                  </li>
                ))}
              </ul>
              <p className="data-path">{restore.path}</p>
              <ul>
                {restore.manifest.dependencies.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            </details>
            {blocked ? (
              <p role="alert">
                {restore.pending
                  ? "已有等待重启的恢复。"
                  : "该工作区已存在。请选择另一个实例恢复。"}
              </p>
            ) : (
              <p>
                <label>
                  <input
                    type="checkbox"
                    checked={confirmed}
                    disabled={busy}
                    onChange={(event) => setConfirmed(event.target.checked)}
                  />{" "}
                  {restore.manifest.workspaceId
                    ? "确认导入此工作区"
                    : "确认替换整个应用数据"}
                </label>
              </p>
            )}
          </div>
        )}
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        {busy && <p role="status">正在处理，请稍候。</p>}
        <div className="cloud-actions dialog-actions">
          <button
            type="button"
            className="button secondary"
            disabled={busy}
            onClick={() => dialog.current?.close()}
          >
            关闭
          </button>
          {view?.kind === "save" && (
            <button
              type="button"
              className="button primary"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  const saved = await portability("backup_save", {
                    workspaceId: view.scope,
                    ui: capturePortableUi(view.scope),
                  });
                  if (!saved) return;
                  dialog.current?.close();
                  setMessage("备份已保存。");
                  await refresh();
                })
              }
            >
              选择位置并备份…
            </button>
          )}
          {restore && (
            <button
              type="button"
              className="button primary"
              disabled={busy || blocked || !confirmed}
              onClick={() =>
                void run(async () => {
                  await portability("restore_stage", {
                    backupId: restore.manifest.backupId,
                    confirmed,
                    previousUi: capturePortableUi(null),
                  });
                  dialog.current?.close();
                  await refresh();
                })
              }
            >
              预约恢复
            </button>
          )}
        </div>
      </AppDialog>
    </section>
  );
}
