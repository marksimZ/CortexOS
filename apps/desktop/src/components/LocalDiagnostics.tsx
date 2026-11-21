import { useState } from "react";
import { diagnostics, type DiagnosticsReport } from "../api/diagnostics";
import { errorText } from "../studio";

const states: Record<string, string> = {
  available: "可用",
  unavailable: "当前不可用",
  nonlocal_context: "未选用本机 Docker",
  linux_required: "需要 Linux 容器模式",
  cli_unavailable: "未能启动 Docker 命令",
  timeout: "检查超时",
  verified: "曾通过检查",
  unverified: "待检查",
  invalid_credential: "密钥无效",
  model_denied: "没有模型访问权限",
  quota_exceeded: "额度不足",
  rate_limited: "请求过于频繁",
  network_unavailable: "网络中断或请求超时",
  service_error: "服务返回异常",
  revalidation_required: "需要重新检查",
  outcome_unknown: "上次请求结果未确认",
  interrupted: "意外中断",
  failed: "执行失败",
};
const nextActions: Record<string, string> = {
  check_credentials: "检查密钥后再试。",
  check_configuration: "核对模型权限和连接设置。",
  check_budget: "核对服务额度和工作区预算。",
  retry_manually_later: "稍后手动重试。",
  check_connection: "恢复网络后手动重试。",
  check_history_before_retry: "先核对处理记录，再决定是否重试。",
  check_history: "查看任务记录里的错误。",
};
const areas: Record<string, string> = {
  chat: "对话",
  benchmark: "评测",
  harness: "项目适配",
  rag: "查找对照",
  embedding: "本地向量",
  cloud: "云端请求",
};
const providers: Record<string, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  gemini: "Gemini",
  compatible: "OpenAI 兼容服务",
  other: "其他服务",
};
const size = (bytes: number | null) =>
  bytes === null
    ? "未测得"
    : Math.round(bytes / 1024 ** 2).toLocaleString("zh-CN") + " MiB";
const rate = (value: number | null) =>
  value === null ? "未测得" : value.toFixed(1) + "%";
const queueText = (queue: DiagnosticsReport["queue"]) =>
  queue
    ? "等待 " +
      queue.queued +
      "，执行中 " +
      queue.running +
      "，停止中 " +
      queue.cancelling
    : "未读取";

export function LocalDiagnostics() {
  const [report, setReport] = useState<DiagnosticsReport>();
  const [pending, setPending] = useState<"read" | "save">();
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  async function run(action: "read" | "save") {
    if (pending) return;
    setPending(action);
    setError("");
    setMessage("");
    try {
      if (action === "read") setReport(await diagnostics("read_diagnostics"));
      else if (await diagnostics("export_diagnostics"))
        setMessage("诊断报告已保存。");
    } catch (failure) {
      setError(errorText(failure));
    } finally {
      setPending(undefined);
    }
  }

  return (
    <section aria-labelledby="local-diagnostics" className="storage-preview">
      <h3 id="local-diagnostics">本机诊断</h3>
      <div className="cloud-actions">
        <button
          type="button"
          className="button secondary"
          disabled={!!pending}
          onClick={() => void run("read")}
        >
          {report ? "重新检查" : "检查运行状态"}
        </button>
        <button
          type="button"
          className="button secondary"
          disabled={!!pending || !report}
          onClick={() => void run("save")}
        >
          导出诊断报告…
        </button>
      </div>
      {(pending || message) && (
        <p role="status">
          {pending === "read"
            ? "正在检查运行状态…"
            : pending === "save"
              ? "正在准备报告…"
              : message}
        </p>
      )}
      {error && (
        <p role="alert" className="form-error">
          {error}
        </p>
      )}
      {report && (
        <>
          <dl className="data-summary">
            <div>
              <dt>检查时间</dt>
              <dd>{new Date(report.capturedAt).toLocaleString("zh-CN")}</dd>
            </div>
            <div>
              <dt>应用版本</dt>
              <dd>{report.appVersion}</dd>
            </div>
            <div>
              <dt>本机后端</dt>
              <dd>{report.closing ? "正在关闭" : states[report.backend]}</dd>
            </div>
            <div>
              <dt>数据库</dt>
              <dd>
                {report.database
                  ? "结构版本 " + report.database.schemaVersion
                  : "无法读取"}
              </dd>
            </div>
            <div>
              <dt>Ollama</dt>
              <dd>{states[report.ollama]}</dd>
            </div>
            <div>
              <dt>Docker</dt>
              <dd>{states[report.docker]}</dd>
            </div>
            <div>
              <dt>任务队列</dt>
              <dd>{queueText(report.queue)}</dd>
            </div>
            <div>
              <dt>存储操作</dt>
              <dd>{queueText(report.storage)}</dd>
            </div>
          </dl>
          <details className="field-help">
            <summary>最近错误（{report.historyHours} 小时）</summary>
            {!report.database ? (
              <p>数据库无法读取，暂无诊断记录。</p>
            ) : report.database.problems.length === 0 ? (
              <p>这段时间没有登记到的失败或中断。</p>
            ) : (
              <ul className="runtime-usage-list">
                {report.database.problems.map((item, index) => (
                  <li key={index}>
                    <strong>
                      {areas[item.area]} {states[item.category]}（{item.count}）
                    </strong>
                    <p>{nextActions[item.nextAction]}</p>
                    <time>
                      {new Date(item.lastSeenUnix * 1000).toLocaleString(
                        "zh-CN",
                      )}
                    </time>
                  </li>
                ))}
              </ul>
            )}
          </details>
          <details className="field-help">
            <summary>最近保存的云端连接检查</summary>
            {report.database?.cloudConnections.length ? (
              <ul className="runtime-usage-list">
                {report.database.cloudConnections.map((item, index) => (
                  <li key={index}>
                    {providers[item.provider]} {states[item.status]}（
                    {item.count}）
                  </li>
                ))}
              </ul>
            ) : (
              <p>
                {report.database ? "当前没有云端连接。" : "无法读取云端连接。"}
              </p>
            )}
          </details>
          <details className="field-help">
            <summary>资源占用</summary>
            <dl className="data-summary">
              <div>
                <dt>整机 CPU</dt>
                <dd>{rate(report.resources.systemCpuPercent)}</dd>
              </div>
              <div>
                <dt>整机内存</dt>
                <dd>
                  {report.resources.systemMemory
                    ? size(report.resources.systemMemory.usedBytes) +
                      " / " +
                      size(report.resources.systemMemory.totalBytes)
                    : "未测得"}
                </dd>
              </div>
              <div>
                <dt>本机后端内存</dt>
                <dd>{size(report.resources.backendWorkingSetBytes)}</dd>
              </div>
              {report.resources.gpus?.map((gpu, index) => (
                <div key={index}>
                  <dt>显卡 {index + 1} 显存与使用率</dt>
                  <dd>
                    {size(gpu.usedBytes)} / {size(gpu.totalBytes)}，
                    {rate(gpu.utilizationPercent)}
                  </dd>
                </div>
              )) || (
                <div>
                  <dt>显卡</dt>
                  <dd>未测得</dd>
                </div>
              )}
            </dl>
          </details>
        </>
      )}
    </section>
  );
}
