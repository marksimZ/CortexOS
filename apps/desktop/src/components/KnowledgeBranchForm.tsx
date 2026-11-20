import { knowledge } from "../api/knowledge";
import { useId, useState } from "react";
import type { KnowledgeDetail } from "../knowledge-types";
import { errorText, type ContextSelection } from "../studio";
import { SelectMenu } from "./SelectMenu";

export interface SavedKnowledgeScope {
  id: string;
  name: string;
  factDate: string;
  itemCount: number;
  baseSnapshotId?: string | null;
}

export interface ScopeMember extends ContextSelection {
  title: string;
}

export function KnowledgeBranchForm({ workspaceId, base, disabled, onSaved }: {
  workspaceId: string;
  base: SavedKnowledgeScope;
  disabled: boolean;
  onSaved: (scope: SavedKnowledgeScope) => void;
}) {
  const id = useId();
  const [members, setMembers] = useState<ScopeMember[]>();
  const [details, setDetails] = useState<KnowledgeDetail>();
  const [selected, setSelected] = useState("");
  const [version, setVersion] = useState("");
  const [name, setName] = useState("");
  const [overrides, setOverrides] = useState<ScopeMember[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const locked = disabled || busy;

  async function run(action: () => Promise<void>) {
    if (locked) return;
    setBusy(true);
    setError("");
    try { await action(); }
    catch (error) { setError(errorText(error)); }
    finally { setBusy(false); }
  }

  const load = () => run(async () => {
    setMembers(await knowledge("list_knowledge_snapshot_members", { workspaceId, snapshotId: base.id }));
  });
  const member = members?.find((value) => value.nodeId === selected);
  const choices = details?.versions.filter((value) => value.version !== member?.version
    && ["knowledge", "context", "decision"].includes(value.kind)
    && (details.sourceEvidence ?? []).every((source) => source.knowledgeVersion !== value.version || source.enabled));

  return <details className="field-help" onToggle={(event) => {
    if (event.currentTarget.open && !members) void load();
  }}>
    <summary>从此快照创建实验分支</summary>
    <p>基准「{base.name}」。替换只在实验中使用，草稿也可选。</p>
    <label htmlFor={`${id}-name`}>分支名称</label>
    <input id={`${id}-name`} value={name} maxLength={80} readOnly={locked} onChange={(event) => setName(event.target.value)} />
    <label htmlFor={`${id}-node`}>要替换的知识</label>
    <SelectMenu id={`${id}-node`} value={selected} disabled={locked || !members?.length}
      onChange={(event) => {
        const node = event.target.value;
        setSelected(node);
        setVersion("");
        setDetails(undefined);
        if (node) void run(async () => setDetails(await knowledge("get_knowledge", { workspaceId, nodeId: node })));
      }}>
      <option value="">选择基准中的知识</option>
      {members?.map((item) => <option key={item.nodeId} value={item.nodeId}>{item.title}（基准 v{item.version}）</option>)}
    </SelectMenu>
    {member && <>
      <label htmlFor={`${id}-version`}>实验版本</label>
      <SelectMenu id={`${id}-version`} value={version} disabled={locked || !choices?.length}
        onChange={(event) => setVersion(event.target.value)}>
        <option value="">选择版本</option>
        {choices?.map((item) => <option key={item.version} value={item.version}>
          v{item.version}，{item.approvedAt ? "已确认" : "待确认草稿"}，{item.title}
        </option>)}
      </SelectMenu>
      {!busy && choices?.length === 0 && <p>这项知识没有其他可用版本。</p>}
      <div className="knowledge-scope-actions">
        <button type="button" className="button secondary" disabled={locked || !version || (overrides.length >= 20 && !overrides.some((item) => item.nodeId === member.nodeId))}
          onClick={() => {
            setOverrides((list) => [...list.filter((item) => item.nodeId !== member.nodeId), { ...member, version: Number(version) }]);
            setNotice("实验版本已加入。");
          }}>加入替换</button>
      </div>
    </>}
    {overrides.length > 0 && <ul className="knowledge-branch-replacements">
      {overrides.map((item) => <li key={item.nodeId}>
        <span>{item.title} 使用 v{item.version}</span>
        <button className="button text-button" type="button" disabled={locked} aria-label={`移除 ${item.title} 的替换`}
          onClick={() => setOverrides((list) => list.filter((entry) => entry.nodeId !== item.nodeId))}>移除</button>
      </li>)}
    </ul>}
    <div className="knowledge-scope-actions">
      <button type="button" className="button secondary" disabled={locked || !overrides.length || !name.trim()}
        onClick={() => void run(async () => {
          const scope = await knowledge("create_knowledge_branch", {
            workspaceId, baseSnapshotId: base.id, name,
            overrides: overrides.map(({ nodeId, version }) => ({ nodeId, version })),
          });
          onSaved(scope);
        })}>保存并使用分支</button>
    </div>
    {busy && <p role="status">正在处理分支资料…</p>}
    {notice && <p role="status">{notice}</p>}
    {error && <div role="alert" className="form-error">{error}
      {!members && <button type="button" disabled={locked} className="button text-button" onClick={() => void load()}>重试读取</button>}
    </div>}
  </details>;
}
