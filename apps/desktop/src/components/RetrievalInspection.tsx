import { navigate } from "../navigation";
import type { CitationReport, RunDetail } from "../studio";
import { RetrievalSummary } from "./RetrievalSummary";

const encoder = new TextEncoder();

const referenceReasons: Record<string, string> = {
  recorded_conflict_counterpart: "通过已登记的反驳关系选用",
  recorded_graph_path: "沿知识关系选用",
  automatic_effective_knowledge: "自动查找后选用",
  automatic_scoped_knowledge: "在指定版本范围内查找后选用",
  user_selected_effective_version: "你手动选用",
  user_selected_scoped_version: "你在指定版本范围内手动选用",
};

export function RetrievalInspection({ detail, beforeLocate, showContext = false, archived = false }: {
  detail: RunDetail;
  beforeLocate?: () => void;
  showContext?: boolean;
  archived?: boolean;
}) {
  const { run, contextManifest, events } = detail;
  const references = contextManifest.knowledgeRefs ?? [];
  const citations = events.find(event => event.kind === "citations_checked")?.payload as CitationReport | undefined;
  const completion = events.find(event => event.kind === "response_completed")?.payload as
    { metrics?: { promptEvalCount?: number | null; evalCount?: number | null } } | undefined;
  const input = completion?.metrics?.promptEvalCount;
  const output = completion?.metrics?.evalCount;
  const scope = contextManifest.knowledgeScope;
  const accepted = events.some(event => event.kind === "model_response_received" || event.kind === "response_completed");
  const attempted = events.some(event => event.kind === "model_request_started");
  const contextTitle = accepted ? "发给模型的资料" : "为模型准备的资料";

  function locate(start: number) {
    const back = window.location.hash.startsWith("#/graph?") ? window.location.hash : null;
    beforeLocate?.();
    navigate("chat", { workspace: run.workspaceId, conversation: run.conversationId,
      message: run.assistantMessageId, citation: start, back });
  }

  return <>
    {scope?.kind === "factDate" && <p>事实日期 {scope.factDate}</p>}
    {scope?.kind === "snapshot" && <p>知识快照「{scope.name}」</p>}
    {scope?.kind === "branch" && <p>实验分支「{scope.name}」</p>}
    {contextManifest.retrieval
      ? <RetrievalSummary report={contextManifest.retrieval} references={references} workspaceId={archived ? null : run.workspaceId} />
      : <p>这条记录没有保存查找详情。</p>}
    <details className="knowledge-disclosure" open={showContext || undefined}>
      <summary>{contextTitle}（{references.length}）</summary>
      {!accepted && <p>{attempted ? "已尝试发送，未记录模型服务收到请求。" : "尚未记录模型服务收到请求。"}</p>}
      {input != null && <p>整次输入 {input.toLocaleString()} Token（模型服务报告）</p>}
      {output != null && <p>整次输出 {output.toLocaleString()} Token（模型服务报告）</p>}
      {!!references.length && <p>单条显示文字的 UTF-8 字节数，模型服务只报告整次用量。</p>}
      <ul className="retrieval-candidates">
        {references.map((item) => {
          const mentions = citations?.occurrences?.filter(occurrence =>
            item.citationId && occurrence.referenceId === item.citationId && !occurrence.error) ?? [];
          return <li key={`${item.nodeId}:${item.version}`}>
            <details>
              <summary>{item.title}，{accepted ? "本次使用 " : "准备使用 "}v{item.version}</summary>
              {referenceReasons[item.reason] && <p>{referenceReasons[item.reason]}</p>}
              <p>{item.citationId && `${item.citationId}，`}文字 {encoder.encode(item.content).length.toLocaleString()} 字节</p>
              {item.sourceEvidence?.map(source => <p key={source.sourceId}>{source.name}，文件 v{source.version}，第 {source.startLine}–{source.endLine} 行</p>)}
              {item.sourceChunk && <p>命中第 {item.sourceChunk.matchStartLine}–{item.sourceChunk.matchEndLine} 行{item.sourceChunk.expanded ? "，另附同节上下文" : ""}。</p>}
              {!!item.versionLineage?.replacesVersion && <p>此版本替代 v{item.versionLineage.replacesVersion}。</p>}
              {!!item.versionLineage?.replacedByVersion && <p>此版本后由 v{item.versionLineage.replacedByVersion} 接替。</p>}
              <p className="context-content">{item.content}</p>
              {!!mentions.length && <div className="retrieval-links">
                {mentions.map(occurrence => archived
                  ? <span key={occurrence.start}>{occurrence.label}</span>
                  : <button type="button" className="button text-button" key={occurrence.start}
                    onClick={() => locate(occurrence.start)}>定位回答中的 {occurrence.label}</button>)}
              </div>}
              {citations && !mentions.length && <p>回答中没有可定位到这条资料的引用。</p>}
              <code>{item.nodeId}</code>
            </details>
          </li>;
        })}
      </ul>
    </details>
  </>;
}
