import { useEffect, useState } from "react";
import { Check, X } from "lucide-react";

type Notice = { text: string; workspaceId?: string };

export function notify(text: string, workspaceId?: string) {
  window.dispatchEvent(new CustomEvent<Notice>("cortexos:notice", { detail: { text, workspaceId } }));
}
export function Toast({ workspaceId }: { workspaceId?: string | null }) {
  const [notice, setNotice] = useState<(Notice & { id: number }) | null>(null);
  const [reminder, setReminder] = useState<(Notice & { id: number }) | null>(null);
  useEffect(() => {
    let id = 0;
    const onNotice = (event: Event) => {
      const item = { ...(event as CustomEvent<Notice>).detail, id: ++id };
      if (item.workspaceId) setReminder(item);
      else setNotice(item);
    };
    window.addEventListener("cortexos:notice", onNotice);
    return () => window.removeEventListener("cortexos:notice", onNotice);
  }, []);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 2500);
    return () => window.clearTimeout(timer);
  }, [notice]);
  useEffect(() => {
    if (!reminder) return;
    const timer = window.setTimeout(() => setReminder(null), 8000);
    return () => window.clearTimeout(timer);
  }, [reminder]);
  return <div className="notice-region" role="status" aria-atomic="true">
    {reminder && reminder.workspaceId === workspaceId && <div key={reminder.id} className="toast"><span>{reminder.text}</span><button className="toast-close" aria-label="关闭提醒提示" onClick={() => setReminder(null)}><X size={14} /></button></div>}
    {notice && <div key={notice.id} className="toast"><Check size={16} aria-hidden="true" /><span>{notice.text}</span><button className="toast-close" aria-label="关闭操作提示" onClick={() => setNotice(null)}><X size={14} /></button></div>}
  </div>;
}
