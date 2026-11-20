import type { ReactNode } from "react";

type StatusTone = "neutral" | "attention" | "warning" | "success";

export function StatusLabel({ children, tone = "neutral", className }: {
  children: ReactNode;
  tone?: StatusTone;
  className?: string;
}) {
  return <span className={className ? `status-label ${className}` : "status-label"} data-tone={tone}>{children}</span>;
}
