import { useEffect, useRef, useState } from "react";
import { webSearch } from "../api/web-search";
import { errorText } from "../frontend-error";
import { StatusLabel } from "./StatusLabel";

export function WebSearchSettings({ workspaceId, focusOnOpen }: { workspaceId: string; focusOnOpen?: boolean }) {
  const [configured, setConfigured] = useState<boolean>();
  const [revision, setRevision] = useState(0);
  const [loadError, setLoadError] = useState("");
  const [actionError, setActionError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const keyField = useRef<HTMLInputElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const focusedOnOpen = useRef(false);

  useEffect(() => {
    if (focusOnOpen && !focusedOnOpen.current) {
      heading.current?.focus();
      focusedOnOpen.current = true;
    }
  }, [focusOnOpen]);

  useEffect(() => {
    const controller = new AbortController();
    void webSearch("read_web_search_status", { workspaceId }, controller.signal)
      .then(status => setConfigured(status.configured))
      .catch(error => { if (!controller.signal.aborted) setLoadError(errorText(error)); });
    return () => controller.abort();
  }, [workspaceId, revision]);

  async function save(key: string) {
    if (busy) return;
    setBusy(true);
    setActionError("");
    setNotice("");
    try {
      await webSearch("save_web_search_key", { workspaceId, key });
      if (keyField.current) keyField.current.value = "";
      setConfigured(!!key);
      setNotice(key ? "已保存搜索密钥。" : "已移除搜索密钥。");
    } catch (error) {
      const message = errorText(error);
      setActionError(key ? message.split(key).join("［密钥］") : message);
    } finally {
      setBusy(false);
    }
  }

  return <section className="assistant-web-search" aria-labelledby="assistant-web-search-title">
    <h3 id="assistant-web-search-title" ref={heading} tabIndex={-1}>网页搜索</h3>
    <p>对话中的公开新闻查询使用 Bing News，无需密钥。配置 Brave Search 密钥后，可在对话中切换查询服务。</p>
    <p>主动查询时，查询词会发给所选服务。结果包含标题、链接和摘要，全文可从来源链接打开。</p>
    {loadError && <div className="form-error" role="alert">
      <p>{loadError}</p>
      <button className="button secondary" type="button" onClick={() => { setLoadError(""); setRevision(value => value + 1); }}>重新读取</button>
    </div>}
    {actionError && <p className="form-error" role="alert">{actionError}</p>}
    <div className="inline-help" role="status">{notice}</div>
    {configured === undefined && !loadError && <p role="status">正在读取搜索配置…</p>}
    {configured !== undefined && <>
      <p><StatusLabel tone={configured ? "success" : "neutral"}>{configured ? "已配置 Brave Search" : "可选配置 Brave Search"}</StatusLabel></p>
      <form aria-label="Brave Search 配置" onSubmit={event => {
        event.preventDefault();
        const key = keyField.current?.value.trim();
        if (!key || !/^[\x21-\x7e]{1,256}$/.test(key)) {
          setNotice("");
          setActionError("Brave Search API 密钥格式无效。");
          return;
        }
        void save(key);
      }}>
        <div className="field">
          <label htmlFor="brave-search-key">Brave Search API 密钥</label>
          <input id="brave-search-key" ref={keyField} type="password" autoComplete="off" spellCheck={false} required disabled={busy} />
        </div>
        <div className="settings-save">
          <button className="button secondary" type="submit" disabled={busy}>{configured ? "更新密钥" : "保存密钥"}</button>
          {configured && <button className="button text-button" type="button" disabled={busy} onClick={() => void save("")}>移除密钥</button>}
        </div>
      </form>
    </>}
  </section>;
}
