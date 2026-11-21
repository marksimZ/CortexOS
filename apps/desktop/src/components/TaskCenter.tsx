import type { RefObject } from "react";
import { X } from "lucide-react";
import { AppDialog } from "./AppDialog";
import { lazyView } from "./lazyView";

const TaskList = lazyView(() => import("./TaskList"), module => module.TaskList);

export function TaskCenter({ dialog, open, onClose, desktop, workspaceId, workspaceName }: {
  dialog: RefObject<HTMLDialogElement | null>;
  open: boolean;
  onClose: () => void;
  desktop: boolean;
  workspaceId: string | null;
  workspaceName?: string;
}) {
  return <AppDialog ref={dialog} onClose={onClose} className="side-drawer panel-dialog task-center" aria-labelledby="task-center-title">
    <header className="panel-header">
      <h2 id="task-center-title">任务中心</h2>
      <button className="icon-button" type="button" aria-label="关闭任务中心" onClick={() => dialog.current?.close()}><X size={18} aria-hidden="true" /></button>
    </header>
    <div className="panel-body">
      {open && (desktop
        ? <TaskList key={workspaceId ?? "local"} workspaceId={workspaceId} workspaceName={workspaceName} close={() => dialog.current?.close()} />
        : <p>请在桌面版查看任务与历史。</p>)}
    </div>
  </AppDialog>;
}
