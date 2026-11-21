import { Channel } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import {
  updates,
  type UpdateChannels,
  type UpdateChannel,
  type UpdateInfo,
  type UpdateProgress,
} from "../api/updates";
import { errorText } from "../studio";
import { SelectMenu } from "./SelectMenu";

const labels = { stable: "稳定版", preview: "预览版" };

export function AppUpdates() {
  const [settings, setSettings] = useState<UpdateChannels>();
  const [channel, setChannel] = useState<UpdateChannel>("stable");
  const [update, setUpdate] = useState<UpdateInfo>();
  const [progress, setProgress] = useState<UpdateProgress>();
  const [ready, setReady] = useState(false);
  const [pending, setPending] = useState<"check" | "download" | "install">();
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    const controller = new AbortController();
    void updates("update_channels", undefined, controller.signal).then(
      (value) => {
        setSettings(value);
        setChannel(value.channels[0] ?? "stable");
      },
      () => {
        if (!controller.signal.aborted) setError("无法读取当前安装版本。");
      },
    );
    return () => controller.abort();
  }, []);

  function resetSelection() {
    setUpdate(undefined);
    setReady(false);
    setProgress(undefined);
    setMessage("");
    setError("");
  }

  async function run(action: NonNullable<typeof pending>) {
    if (pending) return;
    setPending(action);
    setError("");
    setMessage("");
    let feedback: Channel<UpdateProgress> | undefined;
    try {
      if (action === "check") {
        resetSelection();
        const result = await updates("check_update", { channel });
        setUpdate(result ?? undefined);
        if (!result) setMessage("当前渠道没有更新。");
      } else if (update && action === "download") {
        setProgress(undefined);
        feedback = new Channel<UpdateProgress>(setProgress);
        await updates("download_update", { id: update.id, progress: feedback });
        setReady(true);
        setMessage("已校验更新包。");
      } else if (update && ready) {
        await updates("install_update", { id: update.id });
      }
    } catch (failure) {
      setError(errorText(failure));
    } finally {
      if (feedback) feedback.onmessage = () => {};
      setPending(undefined);
    }
  }

  const available = settings?.channels.length;
  return (
    <section aria-labelledby="app-updates" className="storage-preview">
      <h3 id="app-updates">应用更新</h3>
      {settings && <p>当前版本 {settings.version}</p>}
      {available === 0 && <p>此安装包使用手动更新。</p>}
      {settings && !!available && (
        <div className="cloud-actions">
          <label className="field">
            <span>更新渠道</span>
            <SelectMenu
              value={channel}
              disabled={!!pending}
              onChange={(event) => {
                const value = event.currentTarget.value;
                if (value === "stable" || value === "preview") {
                  resetSelection();
                  setChannel(value);
                }
              }}
            >
              {settings.channels.map((value) => (
                <option key={value} value={value}>
                  {labels[value]}
                </option>
              ))}
            </SelectMenu>
          </label>
          <button
            type="button"
            className="button secondary"
            disabled={!!pending}
            onClick={() => void run("check")}
          >
            检查更新
          </button>
        </div>
      )}
      {update && (
        <>
          <p>
            <strong>可用版本 {update.version}</strong>
          </p>
          {update.notes && (
            <details>
              <summary>更新说明</summary>
              <p className="knowledge-prose">{update.notes}</p>
            </details>
          )}
          {!ready && (
            <button
              type="button"
              className="button secondary"
              disabled={!!pending}
              onClick={() => void run("download")}
            >
              下载更新
            </button>
          )}
          {ready && settings?.canInstall && (
            <button
              type="button"
              className="button primary"
              disabled={!!pending}
              onClick={() => void run("install")}
            >
              安装更新…
            </button>
          )}
        </>
      )}
      {pending === "download" && (
        <progress
          aria-label="更新下载进度"
          value={progress?.total ? progress.bytes : undefined}
          max={progress?.total ?? undefined}
        />
      )}
      {(pending || message) && (
        <p role="status">
          {pending === "check"
            ? "正在检查更新…"
            : pending === "download"
              ? "正在下载并校验更新…"
              : pending === "install"
                ? "等待安装确认…"
                : message}
        </p>
      )}
      {error && (
        <p role="alert" className="form-error">
          {error}
        </p>
      )}
    </section>
  );
}
