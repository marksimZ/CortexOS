import { useEffect, useState } from "react";
import { chat } from "../api/chat";
import { taskCenter, type LiveTask, type TaskRecord } from "../api/task-center";
import { navigationHash } from "../navigation";
import { errorText } from "../frontend-error";
import { StatusLabel } from "./StatusLabel";

const labels: Record<string, [name: string, module: string]> = {
  chat: ["生成回答", "对话"],
  benchmark: ["评测或项目任务", "评测"],
  standard: ["标准评测", "评测"],
  personalized_batch: ["个性化评测", "评测"],
  personalized_generation: ["生成评测题库", "个性化评测"],
  rag: ["模型组合对照", "检索评测"],
  rag_index: ["构建知识索引", "知识"],
  harness_validation: ["验证编程题", "项目适配"],
  harness_programming_validation: ["验证编程题", "项目适配"],
  harness_workspace: ["准备或清理任务副本", "项目适配"],
  harness_session: ["运行项目评测单题", "项目适配"],
  harness_codex_session: ["运行 Codex 单题", "项目适配"],
  harness_claude_session: ["运行 Claude Code 单题", "项目适配"],
  harness_aider_session: ["运行 Aider 单题", "项目适配"],
  harness_experiment: ["运行组合实验", "项目适配"],
  harness_readonly_run: ["运行只读测试", "项目适配"],
  harness_execution_preflight: ["检查任务环境", "项目适配"],
  harness_codex_runtime: ["检查 Codex", "项目适配"],
  harness_claude_runtime: ["检查 Claude Code", "项目适配"],
  harness_aider_runtime: ["检查 Aider", "项目适配"],
  local_maintenance: ["本机数据操作", "设置"],
};
const statusNames: Record<string, string> = {
  queued: "等待资源", running: "进行中", cancelling: "正在取消", stopping: "正在停止",
  sending: "正在生成", grading: "正在评分", opening: "正在准备", planned: "待开始",
  completed: "已完成", cancelled: "已取消", failed: "失败", error: "运行出错",
  interrupted: "意外中断", review: "待审核", invalid: "输出需检查", closed: "已关闭",
  ready: "已准备", cleanup_required: "待清理", blocked: "已阻断",
};
const tone = (status: string) => status === "completed" ? "success"
  : ["failed", "interrupted", "error", "blocked", "invalid"].includes(status) ? "attention"
  : ["queued", "cancelling", "stopping", "cleanup_required"].includes(status) ? "warning" : "neutral";
const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "short", timeStyle: "medium" });

function taskLink(workspace: string | null, kind: string, record?: TaskRecord) {
  if (kind === "chat") {
    return record?.conversation
      ? navigationHash("chat", { workspace, conversation: record.conversation, message: record.message })
      : navigationHash("chat", { workspace });
  }
  if (kind === "standard" || kind === "personalized_batch") {
    return navigationHash("benchmark", { workspace, view: kind === "personalized_batch" ? "personalized" : undefined, batch: record?.id });
  }
  if (kind === "personalized_generation") {
    return navigationHash("benchmark", { workspace, view: "personalized", job: record?.id });
  }
  if (kind === "rag") return navigationHash("benchmark", { workspace, view: "rag", tab: "models", experiment: record?.id });
  if (kind.startsWith("harness_")) {
    const experiment = kind === "harness_experiment";
    return navigationHash("benchmark", { workspace, view: "harness", assessment: record?.assessment,
      step: experiment ? "experiments" : "tasks", experiment: experiment ? record?.id : undefined });
  }
  if (kind === "rag_index" || kind === "local_maintenance") {
    return navigationHash("settings", { section: kind === "rag_index" ? "model" : "data" });
  }
  return navigationHash("benchmark", { workspace });
}

function stopAction(task: LiveTask, kind: string) {
  const { id, workspaceId } = task;
  if (task.status === "queued") return () =>
    taskCenter("cancel_waiting_task", { workspaceId, taskId: id, kind: task.kind });
  if (task.status !== "running") return;
  if (kind === "chat") return () => chat("cancel_run", { workspaceId, runId: id });
  if (kind === "standard") return () => taskCenter("stop_standard_benchmark", { workspaceId, batchId: id });
  if (kind === "personalized_batch" || kind === "personalized_generation") return () =>
    taskCenter("stop_personalized_task", { workspaceId, taskId: id });
  if (kind === "harness_experiment") return () => taskCenter("stop_harness_experiment", { workspaceId, experimentId: id });
  if (kind === "harness_programming_validation") return () =>
    taskCenter("stop_harness_programming_validation", { workspaceId, validationId: id });
  if (kind === "rag_index") return () => taskCenter("rag_cancel_index", { workspaceId, requestId: id });
  if (kind === "rag") return () => taskCenter("cancel_rag_runtime_experiment", { workspaceId, id });
}

export function TaskList({ workspaceId, workspaceName, close }: {
  workspaceId: string | null;
  workspaceName?: string;
  close: () => void;
}) {
  const [live, setLive] = useState<LiveTask[]>();
  const [history, setHistory] = useState<TaskRecord[]>([]);
  const [loadError, setLoadError] = useState("");
  const [actionError, setActionError] = useState("");
  const [pending, setPending] = useState<string | null>(null);
  const [revision, refresh] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    const signal = controller.signal;
    let timer: number;
    let previousSignature: string | undefined;
    let lastHistory = 0;
    async function poll() {
      try {
        const next = await taskCenter("read_live_tasks", { workspaceId }, signal);
        setLive(next);
        const signature = next.map(task => `${task.id}:${task.status}`).join(",");
        if (workspaceId && (signature !== previousSignature || next.length && Date.now() - lastHistory >= 3000)) {
          const activeId = next.find(task => task.workspaceId === workspaceId && task.status !== "queued")?.id ?? null;
          setHistory(await taskCenter("read_task_history", { workspaceId, activeId }, signal));
          previousSignature = signature;
          lastHistory = Date.now();
        }
        setLoadError("");
      } catch (error) {
        if (!signal.aborted) setLoadError(errorText(error));
      }
      if (!signal.aborted) timer = window.setTimeout(() => void poll(), 1200);
    }
    void poll();
    return () => { controller.abort(); window.clearTimeout(timer); };
  }, [workspaceId, revision]);
  const liveIds = new Set(live?.map(task => task.id));
  const recent = history.filter(item => !liveIds.has(item.id));
  const open = (kind: string, record?: TaskRecord) =>
    <a className="button text-button" href={taskLink(workspaceId, kind, record)} onClick={close}>
      {record?.kind === "chat" && record.message ? "打开原对话" : record ? "查看记录" : "打开任务页面"}
    </a>;
  const description = (kind: string) => {
    const [name, module] = labels[kind] ?? ["任务记录", "评测"];
    return <div><strong>{name}</strong><span className="task-module">{module}</span></div>;
  };
  const details = (record?: TaskRecord) => record && <>
    {record.contextLabel && <p className="task-context">{record.contextLabel}</p>}
    {!!record.total && <p>已处理 {record.processed} / {record.total} 道题</p>}
    {record.error && <p className="task-error">{errorText(record.error)}</p>}
  </>;
  const perform = (id: string, stop: () => Promise<void>) => {
    setActionError("");
    setPending(id);
    void stop().catch(error => setActionError(errorText(error))).finally(() => {
      setPending(null);
      refresh(value => value + 1);
    });
  };
  return <>
    {(loadError || actionError) && <div className="task-load-error" role="alert">
      <p>{actionError || loadError}</p>
      <button className="button secondary" onClick={() => { setActionError(""); refresh(value => value + 1); }}>重新读取</button>
    </div>}
    <section aria-labelledby="task-current-title">
      <h3 id="task-current-title">当前任务</h3>
      {!live && !loadError && <p role="status">正在读取任务…</p>}
      {live?.length === 0 && <p>当前没有等待或运行中的任务。</p>}
      {!!live?.length && <ul className="task-list">{live.map(task => {
        const record = history.find(item => item.id === task.id);
        const kind = record?.kind ?? task.kind;
        const stop = stopAction(task, kind);
        return <li key={task.id}>
          <div className="task-line">{description(kind)}<StatusLabel tone={tone(task.status)}>{statusNames[task.status]}</StatusLabel></div>
          {task.status === "queued" && <p>前序任务释放运行资源后开始。</p>}
          {details(record)}
          <div className="task-actions">
            {open(kind, record)}
            {stop && <button className="button secondary" disabled={pending !== null || !!loadError}
              onClick={() => perform(task.id, stop)}>{pending === task.id ? "正在提交…" : task.status === "queued" ? "取消等待" : "停止任务"}</button>}
          </div>
        </li>;
      })}</ul>}
    </section>
    <section aria-labelledby="task-history-title">
      <h3 id="task-history-title">{workspaceName ? `${workspaceName}的最近记录` : "工作区最近记录"}</h3>
      {!workspaceId ? <p>创建或选择工作区后查看记录。</p> : recent.length === 0 && live && !loadError
        ? <p>{liveIds.size ? "当前没有其他记录。" : "当前工作区还没有记录。"}</p> : <ul className="task-list">{recent.map(item => (
          <li key={item.id}>
            <div className="task-line">{description(item.kind)}<StatusLabel tone={tone(item.status)}>{statusNames[item.status] ?? item.status}</StatusLabel></div>
            <p><time dateTime={item.at}>{Number.isNaN(Date.parse(item.at)) ? item.at : time.format(new Date(item.at))}</time></p>
            {details(item)}
            {open(item.kind, item)}
          </li>
        ))}</ul>}
      {workspaceId && <div className="task-actions task-history-links">
        <a className="button text-button" href={navigationHash("graph", { workspace: workspaceId, mode: "execution" })} onClick={close}>对话记录</a>
        <a className="button text-button" href={navigationHash("benchmark", { workspace: workspaceId, tab: "history" })} onClick={close}>标准评测历史</a>
      </div>}
      <a className="button text-button" href={navigationHash("settings", { section: "data" })} onClick={close}>备份与恢复记录</a>
    </section>
  </>;
}
