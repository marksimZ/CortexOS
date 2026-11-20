export type FrontendError = {
  message: string;
  code?: string;
  details?: string;
  retryable?: boolean;
  persistence?: "saved" | "unsaved" | "partial";
};

const fallback = "操作未完成。请检查当前状态后再试。";

function safeText(value: unknown): string | undefined {
  if (typeof value !== "string" || !value.trim()) return;
  return value
    .replace(/-----BEGIN [^-]*PRIVATE KEY-----[\s\S]*/gi, "[私钥已隐藏]")
    .replace(/\b(?:sk-[A-Za-z0-9_-]{8,}|gh[pousr]_[A-Za-z0-9_]{8,}|github_pat_[A-Za-z0-9_]{8,}|AIza[A-Za-z0-9_-]{12,})\b/g, "[密钥已隐藏]")
    .replace(/\bBearer\s+[^\s,;"']+/gi, "Bearer [已隐藏]")
    .replace(/(["']?\b(?:api[_ -]?key|token|password|authorization)\b["']?\s*[:=]\s*)(?:"[^"\r\n]*"|'[^'\r\n]*'|[^\s,;]+)/gi, "$1[已隐藏]")
    .replace(/https?:\/\/[^\s<>"'，。；]+/gi, "[地址已隐藏]")
    .replace(/(?:[A-Z]:[\\/]|\\\\)[^\r\n"'<>，。；]*/gi, "[路径已隐藏]")
    .replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, "")
    .trim().slice(0, 500) || undefined;
}

export function parseFrontendError(value: unknown): FrontendError {
  if (typeof value === "string") return { message: safeText(value) ?? fallback };
  if (!value || typeof value !== "object") return { message: fallback };
  const error = value as Record<string, unknown>;
  const code = typeof error.code === "number" && Number.isSafeInteger(error.code)
    ? String(error.code)
    : typeof error.code === "string" && /^(?:[a-z][a-z_0-9]{0,63}|\d{3,6})$/i.test(error.code)
      ? error.code : undefined;
  const persistence = error.persistence === "saved" || error.persistence === "unsaved" || error.persistence === "partial"
    ? error.persistence : undefined;
  return {
    message: safeText(error.userMessage ?? error.message) ?? fallback,
    code,
    details: safeText(error.publicDetails),
    retryable: typeof error.retryable === "boolean" ? error.retryable : undefined,
    persistence,
  };
}

export function errorText(value: unknown): string {
  const { message, code, details, retryable, persistence } = parseFrontendError(value);
  return [message, code && `错误代码 ${code}。`, details && details !== message ? details : null,
    retryable === true ? "可以重试。" : retryable === false ? "请先解决此问题。" : null,
    persistence === "saved" ? "服务报告数据已保存。" : persistence === "partial" ? "服务报告部分数据已保存。" : persistence === "unsaved" ? "服务报告数据尚未保存。" : null,
  ].filter(Boolean).join(" ");
}
