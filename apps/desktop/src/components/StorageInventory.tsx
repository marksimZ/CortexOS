import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { errorText } from "../studio";
import { AppDialog, openDialog } from "./AppDialog";

type Size = {
  logicalBytes: number;
  allocatedBytes: number | null;
  files: number;
  skippedEntries: number;
  readErrors: number;
  limitReached: boolean;
};
type Item = {
  id: string;
  label: string;
  owner: string;
  location: string | null;
  size: Size | null;
  payloadBytes: number | null;
  references: { label: string; count: number }[] | null;
  reason: string;
};
type Inventory = {
  sampledAt: string;
  fileBytes: number;
  partialFileTotal: boolean;
  activeTaskObserved: boolean;
  referenceError: string | null;
  items: Item[];
  reclaimPreview: ReclaimPreview | null;
};
type CacheAssessment = "rebuildable" | "empty" | "unverified";
type ReclaimPreview = {
  recoveryDirectory: string | null;
  recoveryFreeBytes: number | null;
  cacheItems: { item: Item; assessment: CacheAssessment }[];
  approvalId: string | null;
  approvalCaches: string[];
};
type RecoveryStatus = "archived" | "restored" | "blocked" | "cancelled";
type RecoveryRecord = {
  id: string;
  label: string;
  action: "archive" | "restore";
  requestedAt: string;
  backupLocation: string;
  canRestore: boolean;
  outcome: { status: RecoveryStatus; detail: string | null; savedBytes: number | null; at: string } | null;
};
type RecoveryRequest =
  | { kind: "archive"; cache: string; approvalId: string }
  | { kind: "restore" | "cancel"; id: string };

const recoveryStatus: Record<RecoveryStatus, string> = {
  archived: "已移入备份",
  restored: "已恢复",
  blocked: "未完成",
  cancelled: "已撤销",
};

const cacheStatus: Record<CacheAssessment, string> = {
  rebuildable: "可重建，待复查",
  empty: "无文件",
  unverified: "未核实，受保护",
};

const amount = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 1 });
function bytes(value: number) {
  const unit = value >= 1024 ** 3 ? 3 : value >= 1024 ** 2 ? 2 : value >= 1024 ? 1 : 0;
  return `${amount.format(value / 1024 ** unit)} ${["字节", "KiB", "MiB", "GiB"][unit]}`;
}

const partial = (size: Size) => size.skippedEntries > 0 || size.readErrors > 0 || size.limitReached;

function itemSize(item: Item) {
  if (item.size) {
    if (partial(item.size)) return item.size.files ? `至少 ${bytes(item.size.logicalBytes)}` : "未测得";
    return item.size.files ? bytes(item.size.logicalBytes) : "无文件";
  }
  if (item.payloadBytes !== null) return `库内内容 ${bytes(item.payloadBytes)}`;
  return item.location && item.references ? "查看引用" : "占用未测量";
}

function ItemDetails({ item, status, children }: { item: Item; status?: string; children?: ReactNode }) {
  return <li>
    <details className="field-help">
      <summary>{item.label}<span className="storage-size">{status ?? itemSize(item)}</span></summary>
      <p>{item.reason}</p>
      <dl className="data-summary">
        <div><dt>管理方</dt><dd>{item.owner}</dd></div>
        {item.location && <div><dt>位置</dt><dd className="data-path">{item.location}</dd></div>}
        {status && <div><dt>文件大小</dt><dd>{itemSize(item)}</dd></div>}
        {item.size && <>
          <div><dt>已读取文件</dt><dd>{amount.format(item.size.files)} 个</dd></div>
          <div><dt>逐文件磁盘分配量</dt><dd>{item.size.allocatedBytes === null ? "未测得" : `${partial(item.size) ? "至少 " : ""}${bytes(item.size.allocatedBytes)}`}</dd></div>
        </>}
        {item.references?.map(reference => <div key={reference.label}><dt>{reference.label}</dt><dd>{amount.format(reference.count)} 项</dd></div>)}
      </dl>
      {item.payloadBytes !== null && <p>约 {bytes(item.payloadBytes)} 的库内内容已包含在主数据库和待写回记录中。</p>}
      {!!item.size?.skippedEntries && <p>跳过 {item.size.skippedEntries} 项链接或特殊文件。</p>}
      {!!item.size?.readErrors && <p>{item.size.readErrors} 项未能读取。</p>}
      {item.size?.limitReached && <p>已达单次读取上限。</p>}
      {children}
    </details>
  </li>;
}

function PreviewReport({ inventory, preview, canRequest, onArchive }: {
  inventory: Inventory; preview: ReclaimPreview; canRequest: boolean; onArchive: (item: Item) => void;
}) {
  const candidates = preview.cacheItems.filter(({ assessment }) => assessment === "rebuildable");
  const measuredBytes = candidates.reduce((total, { item }) => total + (item.size?.logicalBytes ?? 0), 0);
  return <section aria-labelledby="reclaim-preview-title" className="storage-preview">
    <h4 id="reclaim-preview-title">回收预览</h4>
    <p>{candidates.length ? `${candidates.length} 项缓存可重建，退出应用后需复查。` : "暂未确认可回收的缓存。"}</p>
    <dl className="data-summary">
      {!!candidates.length && <div><dt>已测缓存文件大小</dt><dd>{bytes(measuredBytes)}</dd></div>}
      <div><dt>读取时间</dt><dd>{new Date(inventory.sampledAt).toLocaleString("zh-CN")}</dd></div>
      <div><dt>备份磁盘可用空间</dt><dd>{preview.recoveryFreeBytes === null ? "未测得" : bytes(preview.recoveryFreeBytes)}</dd></div>
      <div><dt>拟用备份位置</dt><dd className="data-path">{preview.recoveryDirectory ?? "位置未核实"}</dd></div>
    </dl>
    {inventory.activeTaskObserved && <p>生成预览时有任务在运行，需待任务结束后重新核对。</p>}
    <ul className="runtime-usage-list" aria-label="界面缓存检查">
      {preview.cacheItems.map(({ item, assessment }) => <ItemDetails key={item.id} item={item} status={cacheStatus[assessment]}>
        {preview.approvalId && preview.approvalCaches.includes(item.id) && !inventory.activeTaskObserved &&
          <button type="button" className="button secondary" disabled={!canRequest} onClick={() => onArchive(item)}>移到备份…</button>}
      </ItemDetails>)}
    </ul>
    <details className="field-help">
      <summary>查看受保护的数据与引用</summary>
      <ul className="runtime-usage-list" aria-label="受保护的数据">
        {inventory.items.map(item => <ItemDetails key={item.id} item={item} />)}
      </ul>
    </details>
  </section>;
}

function RecoveryHistory({ records, disabled, onRequest }: {
  records: RecoveryRecord[]; disabled: boolean; onRequest: (record: RecoveryRecord, kind: "restore" | "cancel") => void;
}) {
  if (!records.length) return null;
  const pending = records.some(row => !row.outcome);
  return <section className="storage-preview" aria-labelledby="storage-recovery-title">
    <h4 id="storage-recovery-title">缓存操作记录</h4>
    {pending && <p role="status">有一项预约。退出 CortexOS 后重新打开，应用会先复查再处理。</p>}
    <ul className="runtime-usage-list" aria-label="缓存操作记录">
      {records.map(row => <li key={row.id}>
        <details className="field-help" open={!row.outcome || row.outcome.status === "blocked" || undefined}>
          <summary>{row.action === "archive" ? "备份" : "恢复"}{row.label}
            <span className="storage-size">{row.outcome ? recoveryStatus[row.outcome.status] : "待下次启动"}</span>
          </summary>
          <dl className="data-summary">
            <div><dt>预约时间</dt><dd>{new Date(row.requestedAt).toLocaleString("zh-CN")}</dd></div>
            {row.outcome && <div><dt>处理时间</dt><dd>{new Date(row.outcome.at).toLocaleString("zh-CN")}</dd></div>}
            <div><dt>{row.action === "archive" ? "备份位置" : "恢复前缓存的保存位置"}</dt><dd className="data-path">{row.backupLocation}</dd></div>
            {row.outcome?.savedBytes != null && <div><dt>文件大小</dt><dd>{bytes(row.outcome.savedBytes)}</dd></div>}
            {row.outcome?.status === "archived" && <div><dt>释放空间</dt><dd>0 字节（同盘备份）</dd></div>}
          </dl>
          {row.outcome?.detail && <p className="form-error">{row.outcome.detail}</p>}
          {!row.outcome && <button type="button" className="button secondary" disabled={disabled} onClick={() => onRequest(row, "cancel")}>撤销预约</button>}
          {row.canRestore && <button type="button" className="button secondary" disabled={disabled || pending} onClick={() => onRequest(row, "restore")}>恢复缓存…</button>}
        </details>
      </li>)}
    </ul>
  </section>;
}

export function StorageInventory() {
  const [inventory, setInventory] = useState<Inventory>();
  const [operation, setOperation] = useState<"inventory" | "preview">();
  const [stopping, setStopping] = useState(false);
  const [stopped, setStopped] = useState(false);
  const [error, setError] = useState("");
  const [records, setRecords] = useState<RecoveryRecord[]>([]);
  const [recoveryError, setRecoveryError] = useState("");
  const [saving, setSaving] = useState(false);
  const [confirmation, setConfirmation] = useState<{ request: RecoveryRequest; label: string; path: string | null }>();
  const dialog = useRef<HTMLDialogElement>(null);
  const cancelConfirmation = useRef<HTMLButtonElement>(null);
  const activeScan = useRef<string | null>(null);

  useEffect(() => {
    let mounted = true;
    void invoke<RecoveryRecord[]>("storage_recovery_history").then(
      rows => { if (mounted) setRecords(rows); },
      failure => { if (mounted) setRecoveryError(errorText(failure)); },
    );
    return () => { mounted = false; };
  }, []);

  useEffect(() => {
    if (confirmation) {
      openDialog(dialog.current);
      cancelConfirmation.current?.focus({ preventScroll: true });
    }
  }, [confirmation]);

  useEffect(() => () => {
    const scanId = activeScan.current;
    activeScan.current = null;
    if (scanId) void invoke("cancel_storage_inventory", { scanId }).catch(() => {});
  }, []);

  async function measure(preview: boolean) {
    if (activeScan.current) return;
    const scanId = crypto.randomUUID();
    activeScan.current = scanId;
    setOperation(preview ? "preview" : "inventory");
    setStopped(false);
    setError("");
    try {
      const result = await invoke<Inventory>("storage_inventory", { scanId, preview });
      if (activeScan.current === scanId) setInventory(result);
    } catch (failure) {
      if (activeScan.current !== scanId) return;
      const message = errorText(failure);
      if (message === "已停止读取存储清单。") setStopped(true);
      else setError(message);
    } finally {
      if (activeScan.current === scanId) {
        activeScan.current = null;
        setOperation(undefined);
        setStopping(false);
      }
    }
  }

  async function stop() {
    const scanId = activeScan.current;
    if (!scanId) return;
    setStopping(true);
    try { await invoke("cancel_storage_inventory", { scanId }); }
    catch (failure) {
      if (activeScan.current !== scanId) return;
      setError(errorText(failure));
      setStopping(false);
    }
  }

  async function requestRecovery(request: RecoveryRequest) {
    if (saving || operation) return;
    setSaving(true);
    setRecoveryError("");
    try {
      setRecords(await invoke<RecoveryRecord[]>("storage_recovery_request", { request }));
      setInventory(previous => previous?.reclaimPreview
        ? { ...previous, reclaimPreview: { ...previous.reclaimPreview, approvalId: null } }
        : previous);
      dialog.current?.close();
    } catch (failure) {
      setRecoveryError(errorText(failure));
    } finally {
      setSaving(false);
    }
  }

  const disabled = !!operation || saving;
  return <section aria-labelledby="storage-inventory-title" className="storage-inventory">
    <div className="cloud-runtime-actions">
      <h3 id="storage-inventory-title">存储清单</h3>
      <button type="button" className="button secondary" disabled={disabled} onClick={() => void measure(false)}>
        {inventory && !inventory.reclaimPreview ? "重新读取" : "读取存储清单"}
      </button>
      <button type="button" className="button secondary" disabled={disabled} onClick={() => void measure(true)}>
        预览可回收内容
      </button>
      {operation && <button type="button" className="button text-button" disabled={stopping} onClick={() => void stop()}>停止读取</button>}
    </div>
    <div role="status">
      {operation ? (stopping ? "正在停止…" : operation === "preview" ? "正在生成回收预览…" : "正在读取本机文件大小与引用…") : stopped ? "已停止本次读取。" : null}
    </div>
    {error && <p className="form-error" role="alert">{error}</p>}
    {inventory?.referenceError && <p role="alert" className="form-error">部分引用未能读取。{inventory.referenceError}</p>}
    {inventory && (inventory.reclaimPreview ? <PreviewReport inventory={inventory} preview={inventory.reclaimPreview}
      canRequest={!disabled && records.every(row => !!row.outcome)}
      onArchive={item => {
        const approvalId = inventory.reclaimPreview?.approvalId;
        if (approvalId) {
          setRecoveryError("");
          setConfirmation({ request: { kind: "archive", cache: item.id, approvalId }, label: item.label, path: item.location });
        }
      }} /> : <>
      <dl className="data-summary">
        <div><dt>已统计的应用文件</dt><dd>{inventory.partialFileTotal && "至少 "}{bytes(inventory.fileBytes)}</dd></div>
        <div><dt>读取时间</dt><dd>{new Date(inventory.sampledAt).toLocaleString("zh-CN")}</dd></div>
      </dl>
      {inventory.activeTaskObserved && <p>读取开始或结束时有任务在运行，文件大小可能变化。</p>}
      <ul className="runtime-usage-list" aria-label="存储项目及引用">
        {inventory.items.map(item => <ItemDetails key={item.id} item={item} />)}
      </ul>
    </>)}
    {!confirmation && recoveryError && <p className="form-error" role="alert">{recoveryError}</p>}
    <RecoveryHistory records={records} disabled={disabled} onRequest={(record, kind) => {
      if (kind === "cancel") {
        void requestRecovery({ kind, id: record.id });
      } else {
        setRecoveryError("");
        setConfirmation({ request: { kind, id: record.id }, label: record.label, path: record.backupLocation });
      }
    }} />
    <AppDialog ref={dialog} className="form-dialog" aria-labelledby="confirm-cache-operation"
      onClose={() => setConfirmation(undefined)} onCancel={event => { if (saving) event.preventDefault(); }}>
      {confirmation && <form onSubmit={event => { event.preventDefault(); void requestRecovery(confirmation.request); }}>
        <h2 id="confirm-cache-operation">{confirmation.request.kind === "archive" ? "确认移到备份" : "确认恢复缓存"}</h2>
        <p><strong>{confirmation.label}</strong></p>
        {confirmation.path && <p className="data-path">{confirmation.path}</p>}
        <p>{confirmation.request.kind === "archive"
          ? "下次打开 CortexOS 时，应用会复查并将此项缓存移到同盘备份。文件保留在备份中，这一步不会释放磁盘空间。"
          : "下次打开 CortexOS 时，应用会把此项备份放回原位置。如果已经生成新缓存，会先把新缓存单独保存。"}
        </p>
        {recoveryError && <p className="form-error" role="alert">{recoveryError}</p>}
        <div className="cloud-runtime-actions">
          <button type="button" className="button secondary" ref={cancelConfirmation} disabled={saving} onClick={() => dialog.current?.close()}>取消</button>
          <button type="submit" className="button primary" disabled={saving}>{saving ? "正在预约…" : "确认预约"}</button>
        </div>
      </form>}
    </AppDialog>
  </section>;
}
