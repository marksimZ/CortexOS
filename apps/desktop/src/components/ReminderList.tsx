import { useEffect, useRef, useState } from "react";
import { listReminders, saveReminder, setReminderState, type ReminderInput, type ReminderRun } from "../api/reminders";
import { errorText } from "../frontend-error";
import { reminderOccurrences } from "../reminder-draft";
import { ReminderFields } from "./ReminderFields";
import { StatusLabel } from "./StatusLabel";
import { safeSourceUrl } from "./ChatWebSources";

const states = { active: "启用中", paused: "已暂停", cancelled: "已取消", completed: "已结束" };
const kinds = { once: "一次性", daily: "每天", weekdays: "工作日", weekly: "每周" };
const weekdays = ["一", "二", "三", "四", "五", "六", "日"];
const timeFormat = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });

function ReminderTime({ value }: { value: string }) {
  return <time dateTime={value}>{timeFormat.format(new Date(value))}</time>;
}

function NewsBrief({ brief }: { brief: NonNullable<ReminderRun["brief"]> }) {
  const sources = brief.results.flatMap((source, index) => {
    const url = safeSourceUrl(source.url);
    return url ? [{ source, url, index }] : [];
  });
  const status = {
    pending: "正在获取新闻",
    ready: sources.length ? "简报已保存" : "暂无可用来源",
    empty: "暂无新闻结果",
    failed: "获取新闻失败",
  }[brief.status];
  return <details className="field-help reminder-news-brief">
    <summary>新闻简报 · {status}</summary>
    <p>查询词：{brief.query}</p>
    {brief.status === "failed" && brief.error && <p className="task-error">{errorText(brief.error)}</p>}
    {brief.status === "ready" && sources.length > 0 && <>
      <p>来源为 Bing News 搜索摘要。</p>
      <ol>{sources.map(({ source, url, index }) => <li key={index}>
        <a href={url.href} target="_blank" rel="noopener noreferrer">{source.title || url.hostname}</a>
        {source.snippet && <p>{source.snippet}</p>}
      </li>)}</ol>
    </>}
  </details>;
}

export function ReminderList({ workspaceId, open }: { workspaceId: string; open: boolean }) {
  const [listing, setListing] = useState<Awaited<ReturnType<typeof listReminders>>>();
  const [revision, setRevision] = useState(0);
  const [pending, setPending] = useState(false);
  const [loadError, setLoadError] = useState("");
  const [actionError, setActionError] = useState("");
  const [editing, setEditing] = useState<{ id: string; input: ReminderInput }>();
  const [cancelling, setCancelling] = useState<string>();
  const editor = useRef<HTMLFieldSetElement>(null);
  const editingId = editing?.id;

  useEffect(() => {
    if (open && editingId) editor.current?.querySelector("input")?.focus();
  }, [open, editingId]);

  useEffect(() => {
    if (!open || pending) return;
    let current = true;
    let timer: number;
    async function read() {
      try {
        const next = await listReminders(workspaceId);
        if (current) {
          setListing(next);
          setLoadError("");
        }
      } catch (error) {
        if (current) setLoadError(errorText(error));
      }
      if (current) timer = window.setTimeout(() => void read(), 5000);
    }
    void read();
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [workspaceId, open, revision, pending]);

  async function perform(action: () => Promise<unknown>) {
    setActionError("");
    setPending(true);
    try {
      await action();
      setEditing(undefined);
      setCancelling(undefined);
    } catch (error) {
      setActionError(errorText(error));
    } finally {
      setPending(false);
      setRevision(value => value + 1);
    }
  }

  return <>
    <section className="reminder-list-section" aria-labelledby="reminder-list-title">
      <h3 id="reminder-list-title">提醒</h3>
      {loadError && <div className="task-load-error" role="alert">
        <p>{loadError}</p>
        <button className="button secondary" type="button" onClick={() => setRevision(value => value + 1)}>重新读取</button>
      </div>}
      {actionError && <p className="task-error" role="alert">{actionError}</p>}
      {!listing && !loadError && <p role="status">正在读取提醒…</p>}
      {listing?.reminders.length === 0 && <p>当前工作区还没有提醒。可在对话输入区添加。</p>}
      {!!listing?.reminders.length && <ul className="task-list">{listing.reminders.map(reminder => {
        const { id, title, kind, date, time, weekday, state, nextAt, newsQuery } = reminder;
        const manageable = state === "active" || state === "paused";
        const input = editing?.id === id ? editing.input : null;
        const canSave = input && manageable && !pending && input.title.trim()
          && (input.newsQuery == null || input.newsQuery.trim()) && reminderOccurrences(input).length;
        return <li key={id}>
          <div className="task-line"><div><strong>{title}</strong></div><StatusLabel tone={state === "paused" ? "warning" : state === "active" ? "success" : "neutral"}>{states[state]}</StatusLabel></div>
          <p>{kind === "once" ? date : kind === "weekly" ? `每周${weekdays[weekday - 1]}` : kinds[kind]} {time}</p>
          {newsQuery && <p>新闻查询：{newsQuery}</p>}
          {state === "active" && nextAt && <p>下次提醒 <ReminderTime value={nextAt} /></p>}
          {input && <form onSubmit={event => {
            event.preventDefault();
            if (canSave) void perform(() => saveReminder(workspaceId, id, input));
          }}>
            <fieldset ref={editor} disabled={pending || !manageable}>
              <legend>编辑提醒</legend>
              <ReminderFields value={input} onChange={next => setEditing({ id, input: next })} />
            </fieldset>
            <div className="task-actions">
              <button className="button primary" type="submit" disabled={!canSave}>保存变更</button>
              <button className="button secondary" type="button" disabled={pending} onClick={() => { setEditing(undefined); setActionError(""); }}>放弃编辑</button>
            </div>
          </form>}
          {manageable && !editing && <div className="task-actions">
            {cancelling === id ? <>
              <span>取消这项提醒？</span>
              <button className="button secondary" type="button" disabled={pending} onClick={() => { setCancelling(undefined); setActionError(""); }}>保留提醒</button>
              <button className="button secondary" type="button" disabled={pending} onClick={() => void perform(() => setReminderState(workspaceId, id, "cancelled"))}>确认取消</button>
            </> : <>
              <button className="button secondary" type="button" disabled={pending} onClick={() => { setActionError(""); setCancelling(undefined); setEditing({ id, input: { title, kind, date, time, weekday, newsQuery } }); }}>编辑</button>
              <button className="button secondary" type="button" disabled={pending} onClick={() => void perform(() => setReminderState(workspaceId, id, state === "active" ? "paused" : "active"))}>{state === "active" ? "暂停" : "恢复"}</button>
              <button className="button text-button" type="button" disabled={pending} onClick={() => { setCancelling(id); setActionError(""); }}>取消提醒</button>
            </>}
          </div>}
        </li>;
      })}</ul>}
    </section>
    {listing && <section className="reminder-list-section" aria-labelledby="reminder-history-title">
      <h3 id="reminder-history-title">提醒历史</h3>
      {listing.runs.length === 0 ? <p>暂无提醒记录。</p> : <ul className="task-list">{listing.runs.map(run => <li key={run.id}>
        <div className="task-line"><div><strong>{run.title}</strong></div><StatusLabel tone={run.status === "delivered" ? "success" : "warning"}>{run.status === "delivered" ? "到期" : "已错过"}</StatusLabel></div>
        <p><ReminderTime value={run.scheduledAt} /></p>
        {run.brief && <NewsBrief brief={run.brief} />}
      </li>)}</ul>}
    </section>}
  </>;
}
