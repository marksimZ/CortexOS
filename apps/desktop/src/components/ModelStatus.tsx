import { AlertTriangle, CheckCircle2, ChevronRight, Cloud, Unplug } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { chat } from "../api/chat";
import {
  CLOUD_CONNECTIONS_CHANGED_EVENT,
  useModelUsagePreference,
} from "../model-usage-preference";
import type { RuntimeConnection } from "../runtime-profiles";
import type { Studio } from "../studio";

type CloudState = {
  workspace: string;
  loading: boolean;
  rows: RuntimeConnection[];
  error: string | null;
};

type PillTone = "connected" | "warning" | "error" | "neutral";

function summarizeCloudStatus(
  desktop: boolean,
  workspaceId: string | null,
  cloudState: CloudState,
): { label: string; tone: PillTone; tip: string } {
  if (!desktop) {
    return {
      label: "云端预览模式",
      tone: "neutral",
      tip: "浏览器仅供界面预览；在桌面版中可检查真实云端连接状态",
    };
  }
  if (!workspaceId) {
    return {
      label: "云端待建工作区",
      tone: "neutral",
      tip: "创建或选择工作区后即可查看与配置云端模型连接",
    };
  }
  if (cloudState.loading || cloudState.workspace !== workspaceId) {
    return {
      label: "正在检测云端…",
      tone: "neutral",
      tip: "正在读取当前工作区的云端连接状态",
    };
  }
  if (cloudState.error) {
    return {
      label: "云端读取失败",
      tone: "error",
      tip: cloudState.error,
    };
  }
  const rows = cloudState.rows;
  if (!rows.length) {
    return {
      label: "云端未配置",
      tone: "neutral",
      tip: "当前工作区尚未添加云端模型连接，点击前往「设置 → 云端模型」配置",
    };
  }
  const verified = rows.filter(
    (row) =>
      row.credentialAvailable !== false &&
      (row.catalogState === "verified" ||
        row.probes.some((probe) => probe.state === "verified")),
  );
  if (verified.length > 0) {
    return {
      label: verified.length > 1 ? `云端已就绪 (${verified.length})` : "云端模型已就绪",
      tone: "connected",
      tip: `已验证连接：${verified.map((item) => item.label).join("、")}（点击管理云端模型与隐私权限）`,
    };
  }
  const failed = rows.some(
    (row) =>
      row.credentialAvailable === false ||
      (row.catalogState !== "unconfigured" && row.catalogState !== "verified") ||
      row.probes.some(
        (probe) => probe.state !== "unconfigured" && probe.state !== "verified",
      ),
  );
  if (failed) {
    return {
      label: "云端连接异常",
      tone: "error",
      tip: "部分云端连接密钥未绑定或验证失败，点击前往「设置 → 云端模型」排查",
    };
  }
  return {
    label: "云端待验证",
    tone: "warning",
    tip: `已添加 ${rows.length} 个云端连接，点击前往「设置 → 云端模型」验证连通性`,
  };
}

export function ModelStatus({ studio }: { studio: Studio }) {
  const [usage] = useModelUsagePreference();
  const [refreshTick, setRefreshTick] = useState(0);
  const [cloudState, setCloudState] = useState<CloudState>({
    workspace: "",
    loading: false,
    rows: [],
    error: null,
  });

  const bumpCloudRefresh = useCallback(() => {
    setRefreshTick((n) => n + 1);
  }, []);

  useEffect(() => {
    if (!studio.desktop || !usage.cloud) return;
    const onChanged = (event: Event) => {
      const detail = (event as CustomEvent<{ workspaceId?: string | null }>).detail;
      if (!detail?.workspaceId || detail.workspaceId === studio.workspaceId) {
        bumpCloudRefresh();
      }
    };
    window.addEventListener(CLOUD_CONNECTIONS_CHANGED_EVENT, onChanged);
    return () => {
      window.removeEventListener(CLOUD_CONNECTIONS_CHANGED_EVENT, onChanged);
    };
  }, [studio.desktop, studio.workspaceId, usage.cloud, bumpCloudRefresh]);

  useEffect(() => {
    const ws = studio.workspaceId;
    if (!studio.desktop || !usage.cloud || !ws) {
      return;
    }
    let active = true;
    setCloudState({ workspace: ws, loading: true, rows: [], error: null });
    void chat("list_runtime_cloud_connections", { workspaceId: ws })
      .then((rows) => {
        if (active) {
          setCloudState({ workspace: ws, loading: false, rows, error: null });
        }
      })
      .catch((err) => {
        if (active) {
          setCloudState({
            workspace: ws,
            loading: false,
            rows: [],
            error: err instanceof Error ? err.message : String(err),
          });
        }
      });
    return () => {
      active = false;
    };
  }, [studio.desktop, studio.workspaceId, usage.cloud, refreshTick]);

  const localLabel = studio.checking
    ? "正在检测本机…"
    : studio.health
      ? studio.health.models.length
        ? "本机模型已就绪"
        : "本机暂无模型"
      : "本机模型未启动";
  const localTip = studio.health
    ? "本机离线模型服务已就绪，点击前往设置管理本机或云端模型"
    : "本机离线模型服务未启动（点击前往设置；若已配置云端模型可直接对话）";

  const cloud = summarizeCloudStatus(studio.desktop, studio.workspaceId, cloudState);

  return (
    <div className="model-status-group" role="group" aria-label="模型连接状态">
      {usage.local && (
        <a
          href="#/settings"
          className={`model-status ${studio.health ? "connected" : ""}`}
          title={localTip}
          aria-label={localLabel}
        >
          {studio.health ? (
            <CheckCircle2 size={15} aria-hidden="true" />
          ) : (
            <Unplug size={15} aria-hidden="true" />
          )}
          <span>{localLabel}</span>
          <ChevronRight size={14} aria-hidden="true" />
        </a>
      )}
      {usage.cloud && (
        <a
          href="#/settings?section=cloud"
          className={`model-status cloud-status ${cloud.tone}`}
          title={cloud.tip}
          aria-label={cloud.label}
        >
          {cloud.tone === "connected" ? (
            <CheckCircle2 size={15} aria-hidden="true" />
          ) : cloud.tone === "error" || cloud.tone === "warning" ? (
            <AlertTriangle size={15} aria-hidden="true" />
          ) : (
            <Cloud size={15} aria-hidden="true" />
          )}
          <span>{cloud.label}</span>
          <ChevronRight size={14} aria-hidden="true" />
        </a>
      )}
    </div>
  );
}
