import { relations } from "../graph-data";
import { navigate } from "../navigation";
import { useState } from "react";
import { SourceBrowser, type SourceRef } from "../SourceBrowser";
import type { KnowledgeReference, RetrievalReport } from "../studio";
const reasons: Record<string, string> = {
  included: "本次已选用",
  manual_selected: "你已手动选用",
  auto_limit: "超过自动选用的条数上限",
  auto_budget: "超过自动选用的容量上限",
};
const stageNames: Record<string, string> = {
  prepare_terms: "提取查找词语",
  prepare_vector: "准备语义查询",
  lexical_lookup: "按词语查找",
  version_lookup: "在指定版本范围内查找",
  semantic_lookup: "按语义查找",
  merge_rankings: "合并排名并去重",
  rerank: "重新排序",
  conflict_lookup: "查看已登记的反驳关系",
  graph_lookup: "沿知识关系扩展",
  select_context: "按容量和条数选用资料",
};
const milliseconds = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 });
const duration = (micros: number) => micros > 0 && micros < 10 ? "<0.01 毫秒" : `${milliseconds.format(micros / 1000)} 毫秒`;

export function RetrievalSummary({ report, workspaceId, references = [] }: {
  report?: RetrievalReport; workspaceId?: string | null; references?: KnowledgeReference[];
}) {
  const [source, setSource] = useState<SourceRef | null>(null);
  if (!report) return null;
  const route = report.route;
  const skipped = route?.reason === "standalone_greeting" ? "这条消息是问候或致谢，已跳过自动查找。"
    : route?.reason === "no_searchable_text" ? "消息里没有可查找的文字，已跳过自动查找。"
    : "本次没有自动查找知识";
  return (
    <section className="retrieval-summary" aria-label="知识查找结果">
      <p>
        {!report.enabled
          ? skipped
          : report.status === "no_search_terms"
            ? "问题中没有可用于查找的词语"
            : report.status === "no_match"
              ? (report.algorithm === "scoped-term-coverage-v1" ? "没有找到相关资料" : "没有找到相关的已确认知识")
              : `找到 ${report.candidates.length} 条相关知识，自动选用 ${report.includedCount} 条`}
      </p>
      {route?.reason === "identifier" && <p>本次按词语查找编号、路径或代码名称。</p>}
      {route?.reason === "runtime_no_matching_index" && <p>本次按词语查找。</p>}
      {route?.fallbackReason === "preview" && <p>预览按词语查找，发送时还会尝试语义查找{route.plannedMethod === "hybrid_rerank" ? "并重新排序" : ""}。</p>}
      {route?.fallbackReason === "no_query_vector" && <p>未取得查询向量，本次按词语查找。</p>}
      {route?.graphReason === "relationship_question" && <p>问题涉及关联路径或冲突，已尝试沿知识关系查找。</p>}
      {report.semanticStatus === "completed" && report.embedding && <p>按语义与词语查找。{report.embedding.cloudTransfer ? "云端向量模型" : "向量模型"} {report.embedding.model}。</p>}
      {report.cloudRerank?.attempted && report.cloudRerank.appliedCount > 0 && <p>云端模型 {report.reranker} 重排了 {report.cloudRerank.appliedCount} 条授权候选。</p>}
      {report.reranker && !report.cloudRerank && <p>已按关键词覆盖重新排序。</p>}
      {report.cloudRerank?.status === "no_authorized_candidates" && <p>本次没有可发送给云端重排的候选。</p>}
      {report.cloudRerank && ["unavailable", "privacy_blocked"].includes(report.cloudRerank.status)
        && <p>云端重排未应用，保留本地排序。{report.cloudRerank.reason}</p>}
      {report.semanticStatus === "unavailable" && <p>语义查找不可用，本次使用词语查找。{report.semanticNote && <span> {report.semanticNote}</span>}</p>}
      {report.semanticStatus === "index_changed" && <p>索引已变化，本次使用词语查找。</p>}
      {!!report.stages?.length && <details className="knowledge-disclosure">
        <summary>查找步骤与用时</summary>
        {report.elapsedMicros !== undefined && <p>查找用时 <span className="numeric">{duration(report.elapsedMicros)}</span>（选用资料另计）</p>}
        <ol className="retrieval-stages">
          {report.stages.map((stage) => <li key={stage.kind}>
            <span>{stageNames[stage.kind] ?? stage.kind}{stage.available === false ? "（不可用）" : ""}</span>
            <span className="numeric">{duration(stage.elapsedMicros)}</span>
          </li>)}
        </ol>
      </details>}
      {report.enabled && !report.stages?.length && report.elapsedMicros !== undefined && <p>已记录查找用时 {duration(report.elapsedMicros)}。</p>}
      {report.graph?.status === "no_seeds" && <p>图谱查找没有可用的起点。可先选用一项知识。</p>}
      {report.graph?.status === "no_paths" && <p>查找起点周围暂无可用的知识关系。</p>}
      {!!report.graph?.paths.length && (
        <details className="knowledge-disclosure">
          <summary>图谱查找路径（{report.graph.paths.length}）</summary>
          <ol className="knowledge-graph-paths">
            {report.graph.paths.map((path) => {
              const target = path.nodes[path.nodes.length - 1];
              return <li key={path.steps.map((step) => step.edgeId).join("/")}>
                <div>
                  {workspaceId
                    ? <button type="button" className="button text-button"
                      onClick={() => navigate("knowledge", {
                        workspace:workspaceId, node:target.nodeId, version:target.version, back:window.location.hash,
                      })}>{target.title} v{target.version}</button>
                    : <strong>{target.title} v{target.version}</strong>}
                  <span>{path.steps.length} 跳，{path.citationId ?? "本次未选用"}{path.crossDocument ? "，跨文件" : ""}</span>
                </div>
                <div className="knowledge-graph-steps">
                  {path.steps.map((step) => <p key={step.edgeId}>
                    {step.from.title} v{step.from.version} {relations[step.relation] ?? step.relation} {step.to.title} v{step.to.version}
                    {"，"}关系 v{step.edgeVersion}{step.reversed ? "，沿反方向查到" : ""}{step.recordedIn === "saved_snapshot" ? "，来自快照" : ""}
                  </p>)}
                  {path.nodes.filter((node) => node.source).map((node) => <p key={node.nodeId}>
                    {node.title} 的来源为 {node.source!.name}，文件 v{node.source!.version}，第 {node.source!.startLine}–{node.source!.endLine} 行
                  </p>)}
                </div>
              </li>;
            })}
          </ol>
          {report.graph.truncated && <p>达到查找上限，展示部分路径。</p>}
        </details>
      )}
      {!!report.conflicts?.length && (
        <details className="knowledge-disclosure">
          <summary>已记录的反驳关系（{report.conflicts.length}）</summary>
          <ul>
            {report.conflicts.map((edge) => (
              <li key={edge.edgeId}>
                <strong>{edge.from.title} v{edge.from.version} 反驳 {edge.to.title} v{edge.to.version}</strong>
                <span>{[edge.from, edge.to].map((side) => side.citationId ?? `${side.title}未选用`).join("，")}</span>
                <span>{edge.recordedIn === "saved_snapshot" ? "快照中的关系" : "当前图谱中的关系"} v{edge.edgeVersion}</span>
              </li>
            ))}
          </ul>
          {report.conflictsTruncated && <p>只显示前 10 条反驳关系。</p>}
        </details>
      )}
      {report.candidates.length > 0 && (
        <details>
          <summary>查看哪些知识被选用及原因</summary>
          <ul className="retrieval-candidates">
            {report.candidates.map((item) => {
              const reference = references.find((ref) => ref.nodeId === item.nodeId && ref.version === item.version);
              const origin = item.sourceLocation ?? reference?.sourceEvidence?.[0];
              const match = item.sourceLocation?.match ?? (reference?.sourceChunk ? {
                startLine: reference.sourceChunk.matchStartLine, endLine: reference.sourceChunk.matchEndLine,
              } : null);
              const ranks = [item.lexicalRank && `词语第 ${item.lexicalRank}`, item.semanticRank && `语义第 ${item.semanticRank}`,
                item.fusionRank && `合并后第 ${item.fusionRank}`, item.rerankScore != null && `重排得分 ${item.rerankScore}`].filter(Boolean);
              return <li key={`${item.nodeId}:${item.version}`}>
                <details>
                  <summary>{item.title}，v{item.version}</summary>
                  <p>{reasons[item.reason] ?? item.reason}{reference?.citationId && `，${reference.citationId}`}</p>
                  {item.recallSource === "explicit_conflict" && <p>来自已记录的反驳关系</p>}
                  {item.recallSource === "graph_relation" && <p>沿知识关系查到</p>}
                  {!!ranks.length && <p>{ranks.join("，")}</p>}
                  {origin && <>
                    <p>{origin.name}，文件 v{origin.version}，{match ? "命中" : "来源摘录"}第 {(match ?? origin).startLine}–{(match ?? origin).endLine} 行</p>
                    {workspaceId && <button type="button" className="button text-button"
                      onClick={() => setSource({ ...origin, ...match })}>查看此处原文</button>}
                  </>}
                  {item.costUnits > 0 && <p>{item.included ? "计入预算" : "拟占预算"} {item.costUnits.toLocaleString()} 字节</p>}
                </details>
              </li>;
            })}
          </ul>
        </details>
      )}
      {(report.candidatesTruncated || report.queryTruncated) && (
        <p>只显示数量上限内的结果；问题关键词过多时，只使用前面的一部分。</p>
      )}
      {source && workspaceId && <SourceBrowser workspace={workspaceId} initial={source} onClose={() => setSource(null)} />}
    </section>
  );
}
