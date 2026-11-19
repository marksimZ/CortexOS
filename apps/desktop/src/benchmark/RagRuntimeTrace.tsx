import { useCallback, useLayoutEffect, useMemo, useRef, useState } from "react";
import { X } from "lucide-react";
import { AppDialog, openDialog } from "../components/AppDialog";
import { RetrievalInspection } from "../components/RetrievalInspection";
import { projectExecution, type ExecutionData } from "../execution-data";
import GraphViewport, { type ViewportApi } from "../GraphViewport";
import type { RunDetail } from "../studio";

export default function RagRuntimeTrace({
  execution, answer, onClose,
}: { execution: RunDetail; answer: string; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const api = useRef<ViewportApi>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useLayoutEffect(() => openDialog(dialog.current), []);
  const data = useMemo<ExecutionData>(() => ({
    detail: execution,
    view: { revision: 0, camera: [0, 9, 22], target: [0, 0, 0] },
    output: {
      id: execution.run.assistantMessageId,
      workspaceId: execution.run.workspaceId,
      conversationId: execution.run.conversationId,
      role: "assistant", content: answer, status: execution.run.status,
      modelRunId: execution.run.id, finishReason: null,
      createdAt: execution.run.finishedAt ?? execution.run.startedAt,
    },
  }), [execution, answer]);
  const graph = useMemo(() => projectExecution(data), [data]);
  const node = graph.nodes.find(item => item.id === selected);
  const retrieval = node != null && execution.events.find(event => event.seq === node.seq)?.kind === "retrieval_completed";
  const failure = useCallback(() => setFailed(true), []);
  function select(id: string) {
    setSelected(id);
    api.current?.focus(graph.positions[id]);
  }
  return <AppDialog ref={dialog} className="panel-dialog rag-runtime-trace" aria-labelledby="rag-trace-title" onClose={onClose}>
    <header className="panel-header">
      <h2 id="rag-trace-title">3D 执行记录</h2>
      <button className="icon-button" type="button" aria-label="关闭执行记录" onClick={() => dialog.current?.close()}><X size={19} aria-hidden="true" /></button>
    </header>
    <div className="panel-body">
      <div className="trace-toolbar"><button className="button secondary" type="button" disabled={failed} onClick={() => api.current?.fit()}>显示完整流程</button></div>
      <div className="trace-workbench rag-runtime-canvas">
        <section className="trace-index" aria-label="执行节点列表">
          {graph.nodes.map(item => <button type="button" key={item.id} className="trace-node" aria-current={selected === item.id ? "true" : undefined} onClick={() => select(item.id)}>{item.title}</button>)}
        </section>
        <div className="trace-canvas">
          {failed ? <p className="graph-empty">3D 不可用，仍可从列表选取并查看节点。</p> : <GraphViewport
            label="3D 执行画布" nodes={graph.nodes} edges={graph.edges} positions={graph.positions} selected={selected}
            camera={data.view.camera} target={data.view.target} apiRef={api} onSelect={select} onFailure={failure}
            onReady={() => api.current?.fit()} onExitFocus={() => setSelected(null)} onChange={() => {}}
          />}
        </div>
      </div>
      {node && <section aria-label="执行节点详情">
        <h3>{node.title}</h3>
        {retrieval ? <RetrievalInspection detail={execution} archived /> : node.summary && <p>{node.summary}</p>}
        {node.seq == null ? <p className="context-content">{node.content}</p> : <details>
          <summary>详细记录</summary><pre className="context-content">{node.content}</pre>
        </details>}
      </section>}
    </div>
  </AppDialog>;
}
