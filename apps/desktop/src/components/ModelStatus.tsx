import { CheckCircle2, Unplug, ChevronRight } from "lucide-react";
import type { Studio } from "../studio";
export function ModelStatus({ studio }: { studio: Studio }) {
  const label = studio.checking
    ? "正在检测本机…"
    : studio.health
      ? studio.health.models.length
        ? "本机模型已就绪"
        : "本机暂无模型"
      : "本机模型未启动";
  const tip = studio.health
    ? "本机离线模型服务已就绪，点击前往设置管理本机或云端模型"
    : "本机离线模型服务未启动（点击前往设置；若已配置云端模型可直接对话）";
  return (
    <a
      href="#/settings"
      className={`model-status ${studio.health ? "connected" : ""}`}
      title={tip}
      aria-label={label}
    >
      {studio.health ? (
        <CheckCircle2 size={15} aria-hidden="true" />
      ) : (
        <Unplug size={15} aria-hidden="true" />
      )}
      <span>{label}</span>
      <ChevronRight size={14} aria-hidden="true" />
    </a>
  );
}
