import { AppDialog, openDialog } from "./AppDialog";
import { RetrievalInspection } from "./RetrievalInspection";
import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { errorText, type ContextSnapshot, type RunDetail, type Studio } from "../studio";
const statusNames: Record<string, string> = {
  streaming: "生成中",
  completed: "已完成",
  cancelled: "已停止",
  failed: "生成失败",
  interrupted: "生成中断",
};
const eventNames: Record<string, string> = {
  request_received: "收到问题",
  context_prepared: "准备回答资料",
  retrieval_started: "开始查找知识",
  retrieval_completed: "知识查找完成",
  model_called: "开始本地模型调用尝试（旧记录）",
  model_inspection_started: "开始检查本地模型",
  model_inspection_completed: "本地模型检查完成",
  model_request_started: "开始发送模型请求",
  model_response_received: "收到模型服务响应",
  model_fallback: "切换到本机备用模型",
  local_model_usage: "记录本机模型用量",
  response_completed: "回答完成",
  citations_checked: "检查引用位置",
  run_cancelled: "用户停止生成",
  run_failed: "生成失败",
  run_interrupted: "恢复中断记录",
};

export function CognitiveInspector({
  runId,
  studio,
  onClose,
}: {
  runId: string | null;
  studio: Studio;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const completion = detail?.events.find(
    (event) => event.kind === "response_completed",
  )?.payload as { metrics?: { finishReason?: string } } | undefined;
  const outputLimited = completion?.metrics?.finishReason === "length";
  const responseRecorded = !!completion || detail?.events.some(event => event.kind === "model_response_received");
  useEffect(() => {
    if (!runId) return;
    let current = true;
    let timer: number;
    setDetail(null);
    setError(null);
    openDialog(dialog.current);
    const load = async () => {
      try {
        const result = await studio.getRun(runId);
        if (!current) return;
        setDetail(result);
        setError(null);
        if (result.run.status !== "running") return;
      } catch (value) {
        if (current) setError(errorText(value));
      }
      if (current) timer = window.setTimeout(load, 1000);
    };
    void load();
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [runId, studio.workspaceId, studio.getRun]);
  return (
    <AppDialog
      ref={dialog}
      className="side-drawer run-drawer panel-dialog"
      aria-labelledby="run-title"
      onClose={onClose}
    >
      <div className="drawer-heading panel-header">
        <h2 id="run-title">本次回答的处理记录</h2>
        <button
          className="icon-button"
          aria-label="关闭执行详情"
          onClick={() => dialog.current?.close()}
        >
          <X size={20} aria-hidden="true" />
        </button>
      </div>
      <div className="panel-body">
      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : !detail ? (
        <p role="status">正在读取执行记录…</p>
      ) : (
        <>
          <dl className="run-summary">
            <div>
              <dt>模型</dt>
              <dd>{detail.run.modelId}</dd>
            </div>
            <div>
              <dt>状态</dt>
              <dd>
                {outputLimited
                  ? "达到输出上限"
                  : (statusNames[detail.run.status] ??
                    (detail.run.status === "running"
                      ? "生成中"
                      : detail.run.status))}
              </dd>
            </div>
            <div>
              <dt>开始时间</dt>
              <dd>{new Date(detail.run.startedAt).toLocaleString("zh-CN")}</dd>
            </div>
            <div>
              <dt>总容量 / 回答长度上限（Token）</dt>
              <dd>
                {detail.settings.numCtx} / {detail.settings.numPredict}
              </dd>
            </div>
            <div>
              <dt>回答变化程度 / 超时</dt>
              <dd>
                {detail.settings.temperature} / {detail.settings.timeoutSeconds}{" "}
                秒
              </dd>
            </div>
          </dl>
          {detail.run.errorMessage && (
            <p className="form-error">{detail.run.errorMessage}</p>
          )}
          <h3>实际执行步骤</h3>
          <ol className="execution-timeline">
            {detail.events.map((event) => (
              <li key={event.seq}>
                <span className="timeline-dot" />
                <div>
                  <strong>{eventNames[event.kind] ?? event.kind}</strong>
                  <span>
                    {new Date(event.occurredAt).toLocaleTimeString("zh-CN")}，
                    距开始{" "}
                    {Math.max(
                      0,
                      new Date(event.occurredAt).getTime() -
                        new Date(detail.run.startedAt).getTime(),
                    )}{" "}
                    毫秒
                  </span>
                </div>
              </li>
            ))}
          </ol>
          <h3>查找过程与资料</h3>
          {detail.contextManifest.assistant && <p>
            助手版本：{detail.contextManifest.assistant.kind === "workspace"
              ? `${detail.contextManifest.assistant.name} · 版本 ${detail.contextManifest.assistant.version}`
              : "内置默认"}
          </p>}
          <RetrievalInspection detail={detail} beforeLocate={() => dialog.current?.close()} showContext />
          {detail.contextManifest.modelRoute && <p>
            {detail.contextManifest.modelRoute.task === "knowledge" ? "知识问答" : "普通问答"}，
            {detail.contextManifest.modelRoute.reason === "manual_generation" ? "使用对话输入框选定的模型"
              : detail.contextManifest.modelRoute.reason === "lower_estimated_cost" ? "采用估算费用较低的方案"
              : detail.contextManifest.modelRoute.reason === "prices_unavailable" ? "费率未知，使用首选方案"
              : detail.contextManifest.modelRoute.reason === "workspace_default" ? "使用工作区当前方案" : "使用该任务的首选方案"}
          </p>}
          {detail.contextManifest.modelFallback && <p>
            已从 {detail.contextManifest.modelFallback.primaryModel} 切换到本机模型 {detail.contextManifest.modelFallback.backupModel}。
            {detail.contextManifest.modelFallback.reason}
          </p>}
          {detail.primarySnapshot && <RequestSnapshot label="主模型请求快照" snapshot={detail.primarySnapshot} />}
          {detail.snapshot ? (
            <RequestSnapshot label={detail.primarySnapshot ? "本机备用请求快照" : responseRecorded ? "发给模型的完整内容" : "为模型准备的完整内容"}
              snapshot={detail.snapshot} />
          ) : (
            <p>这条旧记录只保存了资料清单，没有保存发给模型的完整内容。</p>
          )}
          <details className="advanced-settings">
            <summary>资料预算与记录说明</summary>
            <p>
              使用 {detail.contextManifest.messageRefs.length} 条对话消息。
              {!!detail.contextManifest.retrieval?.sourceChunksUsed && <> 已选用 {detail.contextManifest.retrieval.sourceChunksUsed} 处经确认的文件片段。</>}
            </p>
            <p>
              预计资料用量 {detail.contextManifest.estimatedUnits} /{" "}
              {detail.contextManifest.budgetUnits}。
              {detail.contextManifest.estimateNote}。
            </p>
            <p>这里记录应用实际执行的操作，不展示模型内部的思考过程。</p>
          </details>
          <details className="advanced-settings">
            <summary>记录编号与对应消息</summary>
            <p className="identifier">{detail.run.id}</p>
            {detail.contextManifest.messageRefs.map((reference) => (
              <p className="identifier" key={reference.messageId}>
                {reference.messageId}
                <br />
                内容校验码（SHA-256）：{reference.contentHash}
              </p>
            ))}
          </details>
        </>
      )}
      </div>
    </AppDialog>
  );
}

function RequestSnapshot({ label, snapshot }: { label: string; snapshot: ContextSnapshot }) {
  const roles: Record<string, string> = { system: "系统说明", user: "你的消息", assistant: "模型回答" };
  return <details className="advanced-settings">
    <summary>{label}</summary>
    {snapshot.request.messages.map((message, index) => <details key={index}>
      <summary>{index + 1}. {roles[message.role] ?? message.role}</summary>
      <p className="context-content">{message.content}</p>
    </details>)}
    <p className="identifier">SHA-256 {snapshot.requestHash}</p>
  </details>;
}
