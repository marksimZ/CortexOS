import { useCallback, useEffect, useState } from "react";
import { Check, X } from "lucide-react";

type Notice = { text: string; workspaceId?: string };
type ToastItem = Notice & { id: number; leaving?: boolean };

const EXIT_MS = 160;

export function notify(text: string, workspaceId?: string) {
  window.dispatchEvent(new CustomEvent<Notice>("cortexos:notice", { detail: { text, workspaceId } }));
}
export function Toast({ workspaceId }: { workspaceId?: string | null }) {
  const [notice, setNotice] = useState<ToastItem | null>(null);
  const [reminder, setReminder] = useState<ToastItem | null>(null);

  const dismissNotice = useCallback((id: number) => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setNotice(cur => (cur?.id === id ? null : cur));
      return;
    }
    setNotice(cur => (cur?.id === id ? { ...cur, leaving: true } : cur));
    window.setTimeout(() => setNotice(cur => (cur?.id === id ? null : cur)), EXIT_MS);
  }, []);

  const dismissReminder = useCallback((id: number) => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setReminder(cur => (cur?.id === id ? null : cur));
      return;
    }
    setReminder(cur => (cur?.id === id ? { ...cur, leaving: true } : cur));
    window.setTimeout(() => setReminder(cur => (cur?.id === id ? null : cur)), EXIT_MS);
  }, []);

  useEffect(() => {
    let id = 0;
    const onNotice = (event: Event) => {
      const item: ToastItem = { ...(event as CustomEvent<Notice>).detail, id: ++id };
      if (item.workspaceId) setReminder(item);
      else setNotice(item);
    };
    window.addEventListener("cortexos:notice", onNotice);
    return () => window.removeEventListener("cortexos:notice", onNotice);
  }, []);

  useEffect(() => {
    if (!notice || notice.leaving) return;
    const currentId = notice.id;
    const timer = window.setTimeout(() => dismissNotice(currentId), 2500 - EXIT_MS);
    return () => window.clearTimeout(timer);
  }, [notice, dismissNotice]);

  useEffect(() => {
    if (!reminder || reminder.leaving) return;
    const currentId = reminder.id;
    const timer = window.setTimeout(() => dismissReminder(currentId), 8000 - EXIT_MS);
    return () => window.clearTimeout(timer);
  }, [reminder, dismissReminder]);

  return <div className="notice-region" role="status" aria-atomic="true">
    {reminder && reminder.workspaceId === workspaceId && <div key={reminder.id} className={`toast${reminder.leaving ? " is-leaving" : ""}`}><span>{reminder.text}</span><button className="toast-close" aria-label="关闭提醒提示" onClick={() => dismissReminder(reminder.id)}><X size={14} /></button></div>}
    {notice && <div key={notice.id} className={`toast${notice.leaving ? " is-leaving" : ""}`}><Check size={16} aria-hidden="true" /><span>{notice.text}</span><button className="toast-close" aria-label="关闭操作提示" onClick={() => dismissNotice(notice.id)}><X size={14} /></button></div>}
  </div>;
}
