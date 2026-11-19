import { invoke } from "@tauri-apps/api/core";
import { useId, useRef, useState } from "react";
import { errorText, localDay, type Studio } from "../studio";
import { SelectMenu } from "./SelectMenu";
import { KnowledgeBranchForm, type SavedKnowledgeScope } from "./KnowledgeBranchForm";

export function KnowledgeScopePicker({ studio }: { studio: Studio }) {
  const id = useId();
  const captureForm = useRef<HTMLDetailsElement>(null);
  const [snapshots, setSnapshots] = useState<SavedKnowledgeScope[]>();
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const workspaceId = studio.workspaceId;
  const scope = studio.knowledgeScope;
  const snapshotId = scope && scope.kind !== "factDate" ? scope.snapshotId : "";
  const selected = snapshots?.find((entry) => entry.id === snapshotId);
  const disabled = busy || studio.sending || !!studio.activeRun;

  async function load() {
    if (busy || !workspaceId) return;
    setBusy(true);
    setError("");
    try {
      setSnapshots(await invoke("list_knowledge_snapshots", { workspaceId }));
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  }

  async function capture() {
    if (!workspaceId || !name.trim() || disabled) return;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const saved = await invoke<SavedKnowledgeScope>("capture_knowledge_snapshot", { workspaceId, name });
      setSnapshots((list) => [saved, ...(list ?? [])]);
      setName("");
      setNotice(`已保存「${saved.name}」，包含 ${saved.itemCount} 项知识。`);
      if (captureForm.current) {
        captureForm.current.open = false;
        captureForm.current.querySelector("summary")?.focus();
      }
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <details className="field-help knowledge-scope" onToggle={(event) => {
      if (event.currentTarget.open && !snapshots && !busy) void load();
    }}>
      <summary>调整知识范围</summary>
      <label htmlFor={`${id}-mode`}>查找范围</label>
      <SelectMenu
        id={`${id}-mode`}
        value={scope?.kind === "branch" ? "snapshot" : scope?.kind ?? "current"}
        disabled={disabled}
        onChange={(event) => {
          const mode = event.target.value;
          setNotice("");
          studio.setKnowledgeScope(mode === "factDate" ? { kind: "factDate", date: localDay() }
            : mode === "snapshot" ? { kind: "snapshot", snapshotId: "" } : null);
        }}
      >
        <option value="current">当前知识</option>
        <option value="snapshot">知识快照或实验分支</option>
        <option value="factDate">按事实日期查找</option>
      </SelectMenu>
      {scope && scope.kind !== "factDate" && <>
        <label htmlFor={`${id}-snapshot`}>选择快照</label>
        <SelectMenu id={`${id}-snapshot`} value={scope.snapshotId} disabled={disabled}
          onChange={(event) => {
            const snapshotId = event.target.value;
            const entry = snapshots?.find((item) => item.id === snapshotId);
            setNotice("");
            studio.setKnowledgeScope({ kind: entry?.baseSnapshotId ? "branch" : "snapshot", snapshotId });
          }}>
          <option value="">请选择快照</option>
          {snapshots?.map((entry) => (
            <option key={entry.id} value={entry.id}>{entry.baseSnapshotId ? "实验分支 " : ""}{entry.name}（{entry.factDate}，{entry.itemCount} 项）</option>
          ))}
        </SelectMenu>
        {snapshots?.length === 0 && <p>还没有快照，可在下方保存当前版本。</p>}
        {selected && workspaceId && <KnowledgeBranchForm key={selected.id} workspaceId={workspaceId} base={selected} disabled={disabled}
          onSaved={(saved) => {
            setSnapshots((list) => [saved, ...(list ?? [])]);
            studio.setKnowledgeScope({ kind: "branch", snapshotId: saved.id });
            setNotice(`已切换到实验分支「${saved.name}」。`);
          }} />}
      </>}
      {scope?.kind === "factDate" && <>
        <label htmlFor={`${id}-day`}>事实日期</label>
        <input id={`${id}-day`} type="date" value={scope.date} disabled={disabled}
          onChange={(event) => studio.setKnowledgeScope({ kind: "factDate", date: event.target.value })} />
        <p>按知识版本填写的生效和失效日期查找。</p>
      </>}
      {scope && !studio.autoRetrieval && <p>开启“自动查找知识”后会在此范围内查找。</p>}
      <details ref={captureForm} className="field-help">
        <summary>保存当前知识快照</summary>
        <label htmlFor={`${id}-name`}>快照名称</label>
        <input id={`${id}-name`} value={name} maxLength={80} readOnly={disabled}
          onChange={(event) => setName(event.target.value)} />
        <div className="knowledge-scope-actions">
          <button type="button" className="button secondary" disabled={disabled || !name.trim()}
            onClick={() => void capture()}>保存快照</button>
        </div>
      </details>
      {busy && <p role="status">正在处理知识快照…</p>}
      {notice && <p role="status">{notice}</p>}
      {error && <div className="form-error" role="alert">{error}
        {!snapshots && <button type="button" className="button text-button" disabled={disabled} onClick={() => void load()}>重试读取</button>}
      </div>}
    </details>
  );
}
