import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Layers2 } from "lucide-react";
import { SelectMenu } from "./SelectMenu";
import { RuntimeBudgetSettings } from "./RuntimeBudgetSettings";
import { RuntimeRoutingSettings } from "./RuntimeRoutingSettings";
import { errorText } from "../studio";
import {
  cloudConnectionLabel, runtimeModes, runtimeRoles, type Profile, type ProfileInput, type Profiles,
  type RuntimeConnection, type RuntimeMode, type RuntimeRole, type Target,
} from "../runtime-profiles";

const local: Target = { kind: "local", model: null };
const off: Target = { kind: "off" };
const cloud: Target = { kind: "cloud", connectionId: "", model: "" };
const initial: ProfileInput = { name: "", mode: "local", generation: local, embedding: local, reranking: off };
const localLabels = { generation: "使用本地默认模型", embedding: "使用当前本地向量索引", reranking: "按词语覆盖度重排" };
const targetKey = (target: Target) => target.kind === "cloud" ? JSON.stringify([target.connectionId, target.model])
  : target.kind === "local" ? "local:" + (target.model ?? "") : "off";
const roleNames = Object.keys(runtimeRoles) as RuntimeRole[];

export function RuntimeProfilesSettings({ workspaceId, localModels }: { workspaceId: string; localModels: string[] }) {
  const [data, setData] = useState<Profiles>({ activeId: null, profiles: [] });
  const [selected, setSelected] = useState("");
  const [draft, setDraft] = useState<ProfileInput>();
  const [connections, setConnections] = useState<RuntimeConnection[]>([]);
  const [indexedModel, setIndexedModel] = useState("");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    let current = true;
    void Promise.all([
      invoke<Profiles>("list_model_runtime_profiles", { workspaceId }),
      invoke<RuntimeConnection[]>("list_runtime_cloud_connections", { workspaceId }),
      invoke<{ index: { model: string } | null }>("rag_index_status", { workspaceId }).catch(() => null),
    ]).then(([snapshot, cloudConnections, index]) => {
      if (!current) return;
      setData(snapshot);
      setConnections(cloudConnections);
      setIndexedModel(index?.index?.model ?? "");
      const profile = snapshot.profiles.find(item => item.id === snapshot.activeId);
      setSelected(profile?.id ?? "");
      setDraft(profile && toInput(profile));
    }).catch(value => { if (current) setError(errorText(value)); })
      .finally(() => { if (current) setBusy(false); });
    return () => { current = false; };
  }, [workspaceId]);

  async function run(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError("");
    setNotice("");
    setDeleting(false);
    try { await action(); }
    catch (value) { setError(errorText(value)); }
    finally { setBusy(false); }
  }

  function select(id: string) {
    const found = data.profiles.find(item => item.id === id);
    setDraft(found ? toInput(found) : id === "new" ? initial : undefined);
    setSelected(id);
    setDeleting(false);
    setError("");
    setNotice("");
  }

  function switchMode(mode: RuntimeMode) {
    if (!draft || draft.mode === mode) return;
    const available = (role: RuntimeRole) => cloudOptions(role, connections)[0]?.[1] ?? cloud;
    if (mode === "cloud") setDraft({ ...draft, mode, generation: available("generation"), embedding: off, reranking: off });
    else if (mode === "hybrid") setDraft({ ...draft, mode, generation: available("generation"), embedding: local, reranking: off });
    else setDraft({ ...draft, mode, generation: local, embedding: local, reranking: off });
  }

  function choices(role: RuntimeRole, input: ProfileInput): [string, Target][] {
    const result: [string, Target][] = [];
    if (role !== "generation") result.push(["关闭", off]);
    if (input.mode !== "cloud") {
      result.push([localLabels[role], local]);
      const models = role === "generation" ? localModels : role === "embedding" && indexedModel ? [indexedModel] : [];
      result.push(...models.map((model): [string, Target] => [model, { kind: "local", model }]));
    }
    if (input.mode !== "local") result.push(...cloudOptions(role, connections));
    const target = input[role];
    if (!result.some(([, choice]) => targetKey(choice) === targetKey(target))) {
      result.unshift([target.kind !== "off" && target.model ? target.model + "（当前不可选）" : "请选择已验证的模型", target]);
    }
    return result;
  }

  const current = data.profiles.find(item => item.id === data.activeId);
  const saved = data.profiles.find(item => item.id === selected);
  const edited = !!draft && (!saved || JSON.stringify(draft) !== JSON.stringify(toInput(saved)));
  return <>
    <div className="section-description"><h2 id="runtime-profiles-settings"><Layers2 size={18} aria-hidden="true" />运行方案</h2></div>
    <div className="settings-fields">
      <RuntimeBudgetSettings workspaceId={workspaceId} />
      <RuntimeRoutingSettings workspaceId={workspaceId} profiles={data.profiles} localModels={localModels} />
      <p role="status">当前方案　{current?.name ?? "本地默认设置"}</p>
      <div className="field">
        <label htmlFor="runtime-preset">选择方案</label>
        <SelectMenu id="runtime-preset" value={selected} disabled={busy}
          onChange={event => select(event.target.value)}>
          <option value="">本地默认设置</option>
          {data.profiles.map(item => <option value={item.id} key={item.id}>{item.name}</option>)}
          <option value="new">新建运行方案</option>
        </SelectMenu>
      </div>
      {draft && <form className="settings-fields" onSubmit={event => {
        event.preventDefault();
        void run(async () => {
          const profile = await invoke<Profile>("save_model_runtime_profile", { workspaceId, profileId: saved?.id ?? null, input: draft });
          setData(value => ({ ...value, profiles: [...value.profiles.filter(item => item.id !== profile.id), profile].sort((a, b) => a.name.localeCompare(b.name)) }));
          setSelected(profile.id);
          setDraft(toInput(profile));
          setNotice("已保存运行方案。");
        });
      }}>
        <div className="parameter-fields">
          <div className="field"><label htmlFor="runtime-name">方案名称</label>
            <input id="runtime-name" value={draft.name} maxLength={80} required disabled={busy}
              onChange={event => setDraft({ ...draft, name: event.target.value })} /></div>
          <div className="field"><label htmlFor="runtime-mode">运行模式</label>
            <SelectMenu id="runtime-mode" value={draft.mode} disabled={busy}
              onChange={event => switchMode(event.target.value as RuntimeMode)}>
              {Object.entries(runtimeModes).map(([mode, name]) => <option key={mode} value={mode}>{name}</option>)}
            </SelectMenu></div>
          {roleNames.map(role => {
            const items = choices(role, draft);
            return <div className="field" key={role}><label htmlFor={"runtime-" + role}>{runtimeRoles[role]}</label>
              <SelectMenu id={"runtime-" + role} value={targetKey(draft[role])} disabled={busy} onChange={event => {
                const choice = items.find(([, target]) => targetKey(target) === event.target.value)?.[1];
                if (choice) setDraft({ ...draft, [role]: choice });
              }}>
                {items.map(([label, target]) => <option key={targetKey(target)} value={targetKey(target)}>{label}</option>)}
              </SelectMenu></div>;
          })}
        </div>
        {draft.embedding.kind === "cloud" && <p>在「云端模型」为所选向量模型建立索引。索引就绪前按词语查找。</p>}
        <button className="button primary" disabled={busy || !edited}>保存方案</button>
        {saved && !edited && saved.id !== data.activeId && <button type="button" className="button secondary"
          disabled={busy} onClick={() => void run(async () => {
            await invoke("activate_model_runtime_profile", { workspaceId, profileId: saved.id });
            setData(value => ({ ...value, activeId: saved.id }));
            setNotice("已切换运行方案。");
          })}>启用方案</button>}
        {saved && saved.id !== data.activeId && <button type="button" className="button text-button" disabled={busy}
          onClick={() => setDeleting(true)}>删除方案</button>}
      </form>}
      {!draft && data.activeId && <button type="button" className="button secondary" disabled={busy} onClick={() => void run(async () => {
        await invoke("activate_model_runtime_profile", { workspaceId, profileId: null });
        setData(value => ({ ...value, activeId: null }));
        setNotice("已恢复本地默认设置。");
      })}>恢复本地默认</button>}
      {deleting && saved && <div className="cloud-runtime-confirm">
        <p>删除运行方案「{saved.name}」？</p>
        <button type="button" className="button secondary" disabled={busy} onClick={() => void run(async () => {
          await invoke("delete_model_runtime_profile", { workspaceId, profileId: saved.id });
          setData(value => ({ ...value, profiles: value.profiles.filter(item => item.id !== saved.id) }));
          select("");
          setNotice("已删除运行方案。");
        })}>确认删除</button>
        <button type="button" className="button text-button" onClick={() => setDeleting(false)}>取消</button>
      </div>}
      {busy && <p role="status">正在读取或保存方案…</p>}
      {notice && <p role="status">{notice}</p>}
      {error && <p className="form-error" role="alert">{error}</p>}
    </div>
  </>;
}

function toInput(profile: Profile): ProfileInput {
  return { name: profile.name, mode: profile.mode, generation: profile.generation, embedding: profile.embedding, reranking: profile.reranking };
}

function cloudOptions(role: RuntimeRole, connections: RuntimeConnection[]): [string, Target][] {
  return connections.filter(connection => connection.credentialAvailable !== false && connection.catalogState !== "invalid_credential")
    .flatMap(connection => connection.probes
    .filter(probe => probe.capability === role && probe.state === "verified")
    .map(probe => [cloudConnectionLabel(connection) + " / " + probe.model,
      { kind: "cloud", connectionId: connection.id, model: probe.model }]));
}
